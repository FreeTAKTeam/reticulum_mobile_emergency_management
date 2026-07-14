fn checklist_template_columns() -> Vec<ChecklistColumnRecord> {
    vec![
        ChecklistColumnRecord {
            column_uid: "col-due-relative-dtg".to_string(),
            column_name: "CompletedDTG".to_string(),
            display_order: 0,
            column_type: ChecklistColumnType::RelativeTime {},
            column_editable: false,
            background_color: None,
            text_color: None,
            is_removable: false,
            system_key: Some(crate::types::ChecklistSystemColumnKey::DueRelativeDtg {}),
        },
        ChecklistColumnRecord {
            column_uid: "col-item".to_string(),
            column_name: "Item".to_string(),
            display_order: 1,
            column_type: ChecklistColumnType::ShortString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: false,
            system_key: None,
        },
        ChecklistColumnRecord {
            column_uid: "col-description".to_string(),
            column_name: "Description".to_string(),
            display_order: 2,
            column_type: ChecklistColumnType::LongString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: false,
            system_key: None,
        },
        ChecklistColumnRecord {
            column_uid: "col-category".to_string(),
            column_name: "Category".to_string(),
            display_order: 3,
            column_type: ChecklistColumnType::ShortString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: false,
            system_key: None,
        },
        ChecklistColumnRecord {
            column_uid: "col-quantity".to_string(),
            column_name: "Quantity".to_string(),
            display_order: 4,
            column_type: ChecklistColumnType::Integer {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: false,
            system_key: None,
        },
    ]
}

fn checklist_template_from_rows(
    uid: &str,
    name: &str,
    description: &str,
    rows: &[(&str, &str, &str, u32)],
) -> ChecklistTemplateRecord {
    let timestamp = current_timestamp_rfc3339();
    let tasks = rows
        .iter()
        .enumerate()
        .map(|(index, (item, description, category, quantity))| {
            let task_uid = format!("{uid}-task-{}", index + 1);
            ChecklistTaskRecord {
                task_uid: task_uid.clone(),
                number: (index + 1) as u32,
                user_status: ChecklistUserTaskStatus::Pending {},
                task_status: ChecklistTaskStatus::Pending {},
                is_late: false,
                updated_at: None,
                deleted_at: None,
                custom_status: None,
                due_relative_minutes: Some(
                    (index as u32 + 1) * DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES,
                ),
                due_dtg: None,
                notes: Some((*description).to_string()),
                row_background_color: None,
                line_break_enabled: false,
                completed_at: None,
                completed_by_team_member_rns_identity: None,
                legacy_value: Some((*item).to_string()),
                cells: vec![
                    ChecklistCellRecord {
                        cell_uid: format!("{task_uid}:col-item"),
                        task_uid: task_uid.clone(),
                        column_uid: "col-item".to_string(),
                        value: Some((*item).to_string()),
                        updated_at: None,
                        updated_by_team_member_rns_identity: None,
                    },
                    ChecklistCellRecord {
                        cell_uid: format!("{task_uid}:col-description"),
                        task_uid: task_uid.clone(),
                        column_uid: "col-description".to_string(),
                        value: Some((*description).to_string()),
                        updated_at: None,
                        updated_by_team_member_rns_identity: None,
                    },
                    ChecklistCellRecord {
                        cell_uid: format!("{task_uid}:col-category"),
                        task_uid: task_uid.clone(),
                        column_uid: "col-category".to_string(),
                        value: Some((*category).to_string()),
                        updated_at: None,
                        updated_by_team_member_rns_identity: None,
                    },
                    ChecklistCellRecord {
                        cell_uid: format!("{task_uid}:col-quantity"),
                        task_uid,
                        column_uid: "col-quantity".to_string(),
                        value: Some(quantity.to_string()),
                        updated_at: None,
                        updated_by_team_member_rns_identity: None,
                    },
                ],
            }
        })
        .collect();
    let mut template = ChecklistTemplateRecord {
        uid: uid.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        version: 1,
        origin_type: ChecklistOriginType::RchTemplate {},
        created_at: Some(timestamp.clone()),
        updated_at: Some(timestamp),
        source_filename: None,
        columns: checklist_template_columns(),
        tasks,
    };
    normalize_checklist_template(&mut template);
    template
}

fn default_checklist_templates() -> Vec<ChecklistTemplateRecord> {
    vec![
        checklist_template_from_rows(
            "tmpl-24-hour-survival-pack",
            "24 Hour Survival Pack",
            "Personal 24-hour emergency loadout for rapid deployment and sustainment.",
            &[
                ("Mini Bic or waterproof lighter", "Reliable ignition source", "Fire & Light", 1),
                ("Compact headlamp", "Hands-free light source", "Fire & Light", 1),
                ("1L Nalgene bottle or canteen", "Durable water container", "Water", 1),
                ("Water purification tabs", "Lightweight and effective", "Water", 10),
                ("MRE or freeze-dried meal", "Primary field ration", "Food", 1),
                ("Energy bars", "High-calorie snack", "Food", 4),
                ("Multitool", "Repair and utility tool", "Tools & Utility", 1),
                ("550 Paracord", "Shelter and lashing", "Tools & Utility", 1),
                ("Emergency mylar blanket", "Warmth and signaling", "Clothing / Warmth", 1),
                ("IFAK", "Trauma bandage, gloves, tourniquet", "Medical / Hygiene", 1),
                ("Compass", "Reliable navigation tool", "Navigation / Communication", 1),
                ("Printed map", "Area-specific map", "Navigation / Communication", 1),
            ],
        ),
        checklist_template_from_rows(
            "tmpl-72-hour-home-preparedness",
            "72 Hour Home Preparedness",
            "Household emergency readiness checklist for shelter-in-place and temporary disruption.",
            &[
                ("Stored drinking water", "Three-day reserve for household use", "Water", 12),
                ("Shelf-stable meals", "Ready-to-eat or low-prep food", "Food", 9),
                ("Manual can opener", "Access canned food during outage", "Food", 1),
                ("Flashlights", "Area lighting during power loss", "Power / Lighting", 2),
                ("Battery bank", "Phone and radio charging backup", "Power / Lighting", 1),
                ("AA/AAA batteries", "Spare cells for lights and radios", "Power / Lighting", 12),
                ("First aid kit", "Household medical supplies", "Medical", 1),
                ("Prescription refill copy", "Medication continuity reference", "Medical", 1),
                ("Hygiene kit", "Soap, wipes, sanitation bags", "Hygiene", 1),
                ("Printed contact sheet", "Emergency contacts and rally info", "Communications", 1),
                ("Weather or crank radio", "Situation updates during outage", "Communications", 1),
                ("Copies of IDs and insurance", "Critical documents protected", "Documents", 1),
            ],
        ),
        checklist_template_from_rows(
            "tmpl-vehicle-emergency-preparedness",
            "Vehicle Emergency Preparedness",
            "Vehicle-based emergency kit for evacuation, roadside survival, and communications continuity.",
            &[
                ("Vehicle first aid kit", "Trauma and minor wound support", "Medical", 1),
                ("Jumper cables", "Battery recovery", "Vehicle Recovery", 1),
                ("Tire inflator or sealant", "Flat tire contingency", "Vehicle Recovery", 1),
                ("Tow strap", "Recovery from mud or ditch", "Vehicle Recovery", 1),
                ("Reflective triangles", "Roadside visibility", "Safety", 3),
                ("High-visibility vest", "Roadside operator visibility", "Safety", 1),
                ("Blanket", "Cold-weather warmth", "Shelter / Warmth", 2),
                ("Stored water bottles", "Occupant hydration", "Water", 6),
                ("Shelf-stable snacks", "Travel sustainment", "Food", 6),
                ("Paper map", "Navigation backup if devices fail", "Navigation", 1),
                ("12V phone charger", "Device power continuity", "Communications", 1),
                ("Handheld radio", "Backup local comms", "Communications", 1),
            ],
        ),
    ]
}
