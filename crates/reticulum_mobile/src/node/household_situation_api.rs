/// Inputs that `plan_situation` reads besides the stored state.
struct SituationContext<'a> {
    identity_hex: &'a str,
    node_name: &'a str,
    household_tag: Option<&'a str>,
    current_eam: &'a EamProjectionRecord,
    saver_active: bool,
    at_ms: u64,
}

/// Records one situation change stores, and which ones go out now.
#[derive(Debug)]
struct SituationPlan {
    state: StoredHouseholdSituation,
    event: Option<EventProjectionRecord>,
    eam: Option<EamProjectionRecord>,
    send_event: bool,
    send_eam: bool,
    messages: Vec<SituationMessageRecord>,
}

/// Derives the MECP event and EAM update for a situation change. `None` means
/// the operator chose the current situation again, which sends nothing new
/// (SOS still plans, so the SOS runtime can re-broadcast).
fn plan_situation(
    previous: &StoredHouseholdSituation,
    situation: HouseholdSituation,
    spec: &SituationSpec,
    context: &SituationContext<'_>,
) -> Result<Option<SituationPlan>, NodeError> {
    let changed = previous.situation != Some(situation) || previous.detail != spec.detail;
    if !changed && situation != (HouseholdSituation::Sos {}) {
        return Ok(None);
    }
    let sequence = if changed { previous.sequence.saturating_add(1) } else { previous.sequence };
    let event = (changed && spec.publishes_event)
        .then(|| {
            situation_event_for(
                context.identity_hex,
                Some(context.node_name.to_string()),
                situation,
                spec,
                context.household_tag,
                sequence,
                context.at_ms,
            )
        })
        .transpose()?;

    let mut eam = context.current_eam.clone();
    let owned = if changed {
        apply_situation_eam_factors(&mut eam, &previous.applied_eam_factors, spec.factors)
    } else {
        previous.applied_eam_factors.clone()
    };
    let eam_changed = !eam_factors_equal(&eam, context.current_eam);
    if eam_changed {
        eam.updated_at_ms = context.at_ms.max(context.current_eam.updated_at_ms.saturating_add(1));
        eam.overall_status = None;
    }

    let held = |send: bool| {
        if context.saver_active {
            SituationMessageOutcome::HeldByPowerSaver {}
        } else if send {
            SituationMessageOutcome::Queued {}
        } else {
            SituationMessageOutcome::SkippedUnchanged {}
        }
    };
    let mut messages = Vec::new();
    if spec.publishes_event {
        let outcome = if event.is_some() { held(true) } else { SituationMessageOutcome::SkippedUnchanged {} };
        messages.push(situation_message(
            SituationMessageKind::Event {},
            outcome,
            event.as_ref().map(|record| record.uid.clone()).or_else(|| previous.event_uid.clone()),
        ));
    }
    let eam_outcome = if eam_changed { held(true) } else { SituationMessageOutcome::SkippedUnchanged {} };
    messages.push(situation_message(
        SituationMessageKind::Eam {},
        eam_outcome,
        Some(eam.callsign.clone()),
    ));

    let (held_event_uid, held_eam) = if context.saver_active {
        // A newer situation replaces whatever was still held.
        let held_event = if changed {
            event.as_ref().map(|record| record.uid.clone())
        } else {
            previous.held_event_uid.clone()
        };
        (held_event, previous.held_eam || eam_changed)
    } else {
        (None, false)
    };
    let state = StoredHouseholdSituation {
        situation: Some(situation),
        detail: spec.detail.clone(),
        mecp_code: Some(spec.mecp_code.to_string()),
        event_uid: if changed { event.as_ref().map(|record| record.uid.clone()) } else { previous.event_uid.clone() },
        sequence,
        updated_at_ms: if changed { context.at_ms } else { previous.updated_at_ms },
        household_mode: previous.household_mode,
        applied_eam_factors: owned,
        eam_callsign: Some(eam.callsign.clone()),
        held_event_uid,
        held_eam,
    };
    Ok(Some(SituationPlan {
        send_event: !context.saver_active && event.is_some(),
        send_eam: !context.saver_active && (eam_changed || previous.held_eam),
        event,
        eam: eam_changed.then_some(eam),
        state,
        messages,
    }))
}

fn sos_is_engaged(status: &SosStatusRecord) -> bool {
    !matches!(status.state, SosState::Idle {})
}

/// The EAM callsign this device reports under, as the EAM screen uses it.
fn own_eam_callsign(settings: &AppSettingsRecord, status: &NodeStatus) -> String {
    let display_name = settings.display_name.trim();
    if display_name.is_empty() { status.name.trim().to_string() } else { display_name.to_string() }
}

fn own_eam(store: &AppStateStore, callsign: &str) -> Result<Option<EamProjectionRecord>, NodeError> {
    Ok(store
        .get_eams()?
        .into_iter()
        .find(|eam| eam.deleted_at_ms.is_none() && eam.callsign.eq_ignore_ascii_case(callsign)))
}

fn local_household_id(inner: &NodeInner) -> Result<Option<String>, NodeError> {
    let Some(mut settings) = inner.app_state.get_app_settings()? else {
        return Ok(None);
    };
    apply_household_defaults(
        &mut settings.community,
        &settings.display_name,
        &local_app_destination_hex(inner),
    );
    Ok(normalize_household_id(&settings.community.household_id).ok())
}

/// The outbound work a committed situation leaves for after the lock is released.
struct SituationSends {
    b04: Option<EventProjectionRecord>,
    event: Option<EventProjectionRecord>,
    eam: Option<EamProjectionRecord>,
}

impl Node {
    pub fn get_household_situation(&self) -> Result<HouseholdSituationRecord, NodeError> {
        let (state, household_id, identity_hex) = {
            let inner = self.lock_inner()?;
            let state = load_household_situation(&inner.app_state)?;
            (state, local_household_id(&inner)?, self.identity_hex_locked(&inner)?)
        };
        Ok(self.situation_record(&state, household_id, &identity_hex, Vec::new()))
    }

    /// Sets Off / This phone reports / Another phone reports. Changing the mode
    /// sends nothing; the next situation uses it.
    pub fn set_household_mode(&self, mode: HouseholdMode) -> Result<HouseholdSituationRecord, NodeError> {
        let (state, household_id, identity_hex) = {
            let inner = self.lock_inner()?;
            let mut state = load_household_situation(&inner.app_state)?;
            state.household_mode = Some(mode);
            commit_situation_state(&inner, &state)?;
            (state, local_household_id(&inner)?, self.identity_hex_locked(&inner)?)
        };
        Ok(self.situation_record(&state, household_id, &identity_hex, Vec::new()))
    }

    /// Stores the chosen situation and every message it implies in one
    /// transaction, then queues B04, the MECP event and the EAM through their
    /// existing paths without waiting for delivery.
    pub fn set_situation(
        &self,
        situation: HouseholdSituation,
        detail: Option<String>,
    ) -> Result<HouseholdSituationRecord, NodeError> {
        let spec = situation_spec(situation, detail.as_deref())?;
        let mut messages = Vec::new();
        // SOS side effects first, so a wrong cancel PIN leaves the situation unchanged.
        if situation == (HouseholdSituation::Sos {}) {
            self.trigger_sos(SosTriggerSource::Manual {})?;
            messages.push(situation_message(SituationMessageKind::Sos {}, SituationMessageOutcome::Queued {}, Some("B01".to_string())));
        } else if situation == (HouseholdSituation::AllOk {}) && sos_is_engaged(&self.get_sos_status()?) {
            self.deactivate_sos(detail)?;
            messages.push(situation_message(SituationMessageKind::Sos {}, SituationMessageOutcome::Queued {}, Some("B03".to_string())));
        }
        let (state, household_id, identity_hex, sends) = {
            let mut inner = self.lock_inner()?;
            let previous = load_household_situation(&inner.app_state)?;
            let mut settings = inner.app_state.get_app_settings()?.ok_or(NodeError::InvalidConfig {})?;
            apply_household_defaults(&mut settings.community, &settings.display_name, &local_app_destination_hex(&inner));
            let household_id = normalize_household_id(&settings.community.household_id).ok();
            let mode = previous.mode();
            let status = inner.status.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?.clone();
            let callsign = own_eam_callsign(&settings, &status);
            let stored_eam = own_eam(&inner.app_state, &callsign)?;
            let current_eam = stored_eam.clone().unwrap_or_else(|| blank_eam(&callsign, 0));
            let context = SituationContext {
                identity_hex: &status.identity_hex,
                node_name: &status.name,
                household_tag: household_id.as_deref().filter(|_| mode != (HouseholdMode::Off {})),
                current_eam: &current_eam,
                saver_active: inner.power_state.saver_active,
                at_ms: now_ms(),
            };
            let Some(plan) = plan_situation(&previous, situation, &spec, &context)? else {
                let unchanged = SituationMessageOutcome::SkippedUnchanged {};
                for kind in [SituationMessageKind::CommunityStatus {}, SituationMessageKind::Event {}, SituationMessageKind::Eam {}] {
                    messages.push(situation_message(kind, unchanged, None));
                }
                drop(inner);
                return Ok(self.situation_record(&previous, household_id, &status.identity_hex, messages));
            };
            let (b04_outcome, b04) = match mode {
                HouseholdMode::Off {} => (SituationMessageOutcome::SkippedHouseholdOff {}, None),
                HouseholdMode::AnotherPhoneReports {} => (SituationMessageOutcome::SkippedAnotherPhoneReports {}, None),
                HouseholdMode::ThisPhoneReports {} => {
                    settings.community.status = spec.b04;
                    match prepare_community_status_event(&inner, &settings.community) {
                        Ok((event, true)) => (SituationMessageOutcome::Queued {}, Some(event)),
                        Ok((_, false)) => (SituationMessageOutcome::SkippedUnchanged {}, None),
                        Err(error) => {
                            inner.bus.emit(NodeEvent::Error {
                                code: "SituationB04Failed".to_string(),
                                message: format!("household B04 was not built reason={error}"),
                            });
                            (SituationMessageOutcome::Failed {}, None)
                        }
                    }
                }
            };
            messages.insert(0, situation_message(SituationMessageKind::CommunityStatus {}, b04_outcome, Some(spec.b04.as_str().to_string())));
            messages.extend(plan.messages.iter().cloned());

            let hub_directory_snapshot = inner.hub_directory_snapshot.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?.clone();
            let eam = plan.eam.as_ref().map(|eam| populate_eam_defaults(&status, eam, hub_directory_snapshot.as_ref()));
            let events = plan.event.iter().chain(b04.iter()).cloned().collect::<Vec<_>>();
            let state_json = household_situation_json(&plan.state)?;
            let invalidations = inner.app_state.commit_household_situation(&HouseholdSituationWrite {
                state_json: &state_json,
                settings: (mode == (HouseholdMode::ThisPhoneReports {})).then_some(&settings),
                eam: eam.as_ref(),
                events: &events,
            })?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }
            if b04.is_some() && inner.power_state.saver_active && inner.cmd_tx.is_some() {
                inner.community_status_sent_in_saver = true;
            }
            let sends = SituationSends {
                b04,
                event: plan.event.filter(|_| plan.send_event),
                eam: if plan.send_eam { eam.or(stored_eam) } else { None },
            };
            (plan.state, household_id, status.identity_hex, sends)
        };
        self.queue_situation_sends(sends);
        Ok(self.situation_record(&state, household_id, &identity_hex, messages))
    }

    /// Sends the situation event and EAM that power saver held, once saver ends.
    fn release_held_situation_messages(&self) {
        let result = self.lock_inner().and_then(|inner| {
            if inner.cmd_tx.is_none() || inner.power_state.saver_active {
                return Ok(None);
            }
            take_held_situation_sends(&inner)
        });
        match result {
            Ok(Some(sends)) => self.queue_situation_sends(sends),
            Ok(None) => {}
            Err(error) => self.emit_situation_error(&format!(
                "held situation messages were not released reason={error}"
            )),
        }
    }

    fn queue_situation_sends(&self, sends: SituationSends) {
        let mut failures = Vec::new();
        if let Some(b04) = sends.b04 {
            if let Err(error) = self.replicate_event_with_delivery(&b04, None, OutboundTrafficClass::CommunityStatus {}, false) {
                failures.push(format!("b04 reason={error}"));
            }
        }
        if let Some(event) = sends.event {
            if let Err(error) = self.replicate_event_with_delivery(&event, None, OutboundTrafficClass::Event {}, false) {
                failures.push(format!("event uid={} reason={error}", event.uid));
            }
        }
        if let Some(eam) = sends.eam {
            if let Err(error) = self.replicate_eam(&eam, false) {
                failures.push(format!("eam callsign={} reason={error}", eam.callsign));
            }
        }
        for failure in failures {
            self.emit_situation_error(&format!("situation replication was not queued {failure}"));
        }
    }

    fn situation_record(
        &self,
        state: &StoredHouseholdSituation,
        household_id: Option<String>,
        identity_hex: &str,
        messages: Vec<SituationMessageRecord>,
    ) -> HouseholdSituationRecord {
        let duplicate_reporter = match (state.mode(), household_id.as_deref()) {
            (HouseholdMode::ThisPhoneReports {}, Some(id)) => self
                .get_community_statuses()
                .ok()
                .and_then(|statuses| duplicate_household_reporter(&statuses, id, identity_hex)),
            _ => None,
        };
        let household_id = household_id.filter(|_| state.mode() != (HouseholdMode::Off {}));
        household_situation_record(state, household_id, duplicate_reporter, messages)
    }

    fn emit_situation_error(&self, message: &str) {
        if let Ok(inner) = self.inner.lock() {
            inner.bus.emit(NodeEvent::Error {
                code: "SituationFailed".to_string(),
                message: message.to_string(),
            });
        }
    }

    fn lock_inner(&self) -> Result<std::sync::MutexGuard<'_, NodeInner>, NodeError> {
        self.inner.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })
    }

    fn identity_hex_locked(&self, inner: &NodeInner) -> Result<String, NodeError> {
        let status = inner.status.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        Ok(status.identity_hex.clone())
    }
}

/// Clears the held markers and returns the stored records they point to.
fn take_held_situation_sends(inner: &NodeInner) -> Result<Option<SituationSends>, NodeError> {
    let mut state = load_household_situation(&inner.app_state)?;
    if !state.is_held() {
        return Ok(None);
    }
    let event = match state.held_event_uid.as_deref() {
        Some(uid) => inner.app_state.get_events()?.into_iter().find(|event| event.uid == uid),
        None => None,
    };
    let eam = match state.eam_callsign.as_deref() {
        Some(callsign) if state.held_eam => own_eam(&inner.app_state, callsign)?,
        _ => None,
    };
    state.held_event_uid = None;
    state.held_eam = false;
    commit_situation_state(inner, &state)?;
    Ok(Some(SituationSends { b04: None, event, eam }))
}

fn commit_situation_state(inner: &NodeInner, state: &StoredHouseholdSituation) -> Result<(), NodeError> {
    let state_json = household_situation_json(state)?;
    let invalidations = inner.app_state.commit_household_situation(&HouseholdSituationWrite {
        state_json: &state_json,
        settings: None,
        eam: None,
        events: &[],
    })?;
    for invalidation in invalidations {
        emit_projection_invalidation(&inner.bus, invalidation);
    }
    Ok(())
}
