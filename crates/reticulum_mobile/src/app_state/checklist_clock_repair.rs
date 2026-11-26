// One-time recovery for checklists poisoned with far-future version timestamps
// before 1.6.1 added ingress validation (#229, #336).
const CHECKLIST_CLOCK_REPAIR_KEY: &str = "checklist_clock_repair_v1";
// 2026-11-05T00:00:00Z (1.6.1). A device clock earlier than this is not trusted to judge "future".
const CHECKLIST_CLOCK_REPAIR_MIN_NOW_SECONDS: i64 = 1_793_836_800;
const CHECKLIST_CLOCK_REPAIR_EPOCH_ANCHOR: &str = "1970-01-01T00:00:00Z";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChecklistClockRepairOutcome {
    Repaired {
        checklist_uid: String,
        anchor: String,
        fields: Vec<String>,
    },
    Skipped {
        checklist_uid: String,
        reason: String,
    },
}

fn checklist_repair_seconds(timestamp: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(timestamp.trim())
        .ok()
        .map(|parsed| parsed.timestamp())
}

/// Every version-like timestamp the merge/ingress rules compare. Due dates (`due_dtg`)
/// and `start_time` are schedule data and are deliberately excluded.
fn checklist_version_timestamps(checklist: &mut ChecklistRecord) -> Vec<(String, &mut String)> {
    let mut fields = Vec::new();
    for (name, value) in [
        ("created_at", &mut checklist.created_at),
        ("updated_at", &mut checklist.updated_at),
        ("uploaded_at", &mut checklist.uploaded_at),
        ("deleted_at", &mut checklist.deleted_at),
    ] {
        if let Some(value) = value.as_mut() {
            fields.push((name.to_string(), value));
        }
    }
    for task in &mut checklist.tasks {
        let task_uid = task.task_uid.clone();
        for (name, value) in [
            ("updated_at", &mut task.updated_at),
            ("deleted_at", &mut task.deleted_at),
            ("completed_at", &mut task.completed_at),
        ] {
            if let Some(value) = value.as_mut() {
                fields.push((format!("tasks[{task_uid}].{name}"), value));
            }
        }
        for cell in &mut task.cells {
            if let Some(value) = cell.updated_at.as_mut() {
                fields.push((
                    format!("tasks[{task_uid}].cells[{}].updated_at", cell.column_uid),
                    value,
                ));
            }
        }
    }
    for publication in &mut checklist.feed_publications {
        if let Some(value) = publication.published_at.as_mut() {
            fields.push((
                format!("feed_publications[{}].published_at", publication.publication_uid),
                value,
            ));
        }
    }
    fields
}

/// Clamps future version timestamps to the newest trustworthy timestamp already in the
/// record. The anchor comes from the record itself, not the local clock, so peers holding
/// the same poisoned copy repair it to the same value and keep last-writer-wins ordering.
fn repair_checklist_clock(
    checklist: &mut ChecklistRecord,
    now_seconds: i64,
) -> Option<ChecklistClockRepairOutcome> {
    let limit = now_seconds.saturating_add(crate::runtime::CHECKLIST_MAX_CLOCK_SKEW_SECONDS);
    let checklist_uid = checklist.uid.clone();
    let fields = checklist_version_timestamps(checklist);
    let mut anchor: Option<(i64, String)> = None;
    let mut future = Vec::new();
    let mut unparseable = Vec::new();
    for (name, value) in fields {
        match checklist_repair_seconds(value) {
            None => unparseable.push(name),
            Some(seconds) if seconds > limit => future.push((name, value)),
            Some(seconds) => {
                if anchor.as_ref().is_none_or(|(best, _)| seconds > *best) {
                    anchor = Some((seconds, value.clone()));
                }
            }
        }
    }
    if future.is_empty() {
        return None;
    }
    if !unparseable.is_empty() {
        return Some(ChecklistClockRepairOutcome::Skipped {
            checklist_uid,
            reason: format!(
                "future version timestamps alongside unparseable ones: {}",
                unparseable.join(", ")
            ),
        });
    }
    let anchor = anchor.map_or_else(|| CHECKLIST_CLOCK_REPAIR_EPOCH_ANCHOR.to_string(), |(_, value)| value);
    let mut repaired = Vec::with_capacity(future.len());
    for (name, value) in future {
        value.clone_from(&anchor);
        repaired.push(name);
    }
    Some(ChecklistClockRepairOutcome::Repaired {
        checklist_uid,
        anchor,
        fields: repaired,
    })
}

impl AppStateStore {
    /// Runs once per database. Originals are kept in `checklist_clock_repairs` as the backup.
    pub(crate) fn repair_future_checklist_clocks(
        &self,
        now_seconds: i64,
    ) -> Result<Vec<ChecklistClockRepairOutcome>, NodeError> {
        let io = |error| crate::error_context::contextual_node_error(NodeError::IoError {}, error);
        if now_seconds < CHECKLIST_CLOCK_REPAIR_MIN_NOW_SECONDS {
            return Ok(Vec::new());
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(io)?;
        let done: Option<String> = transaction
            .query_row(
                "SELECT value FROM metadata WHERE key = ?1",
                params![CHECKLIST_CLOCK_REPAIR_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(io)?;
        if done.is_some() {
            return Ok(Vec::new());
        }
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS checklist_clock_repairs (
                    uid TEXT PRIMARY KEY,
                    repaired_at_ms INTEGER NOT NULL,
                    anchor TEXT NOT NULL,
                    fields_json TEXT NOT NULL,
                    original_json TEXT NOT NULL
                );",
            )
            .map_err(io)?;
        let rows: Vec<(String, String)> = {
            let mut statement = transaction
                .prepare("SELECT uid, json FROM checklists ORDER BY uid")
                .map_err(io)?;
            let rows = statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(io)?
                .collect::<Result<_, _>>()
                .map_err(io)?;
            rows
        };
        let mut outcomes = Vec::new();
        for (uid, original_json) in rows {
            let Ok(mut checklist) = serde_json::from_str::<ChecklistRecord>(&original_json) else {
                outcomes.push(ChecklistClockRepairOutcome::Skipped {
                    checklist_uid: uid,
                    reason: "stored checklist JSON could not be decoded".to_string(),
                });
                continue;
            };
            let Some(outcome) = repair_checklist_clock(&mut checklist, now_seconds) else {
                continue;
            };
            if let ChecklistClockRepairOutcome::Repaired { anchor, fields, .. } = &outcome {
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO checklist_clock_repairs
                            (uid, repaired_at_ms, anchor, fields_json, original_json)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            uid,
                            crate::numeric::u64_to_i64_saturating(now_ms()),
                            anchor,
                            serialize_json(fields)?,
                            original_json
                        ],
                    )
                    .map_err(io)?;
                self.write_checklist_tx(&transaction, &checklist)?;
                self.bump_checklist_projection_revisions_tx(
                    &transaction,
                    uid.as_str(),
                    "checklist-clock-repaired",
                )?;
            }
            outcomes.push(outcome);
        }
        transaction
            .execute(
                "INSERT INTO metadata (key, value) VALUES (?1, '1')",
                params![CHECKLIST_CLOCK_REPAIR_KEY],
            )
            .map_err(io)?;
        transaction.commit().map_err(io)?;
        Ok(outcomes)
    }
}
