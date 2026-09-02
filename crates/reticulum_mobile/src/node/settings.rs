impl Node {
    pub fn get_app_settings(&self) -> Result<Option<AppSettingsRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        inner.app_state.get_app_settings()
    }

    pub fn set_app_settings(&self, mut settings: AppSettingsRecord) -> Result<(), NodeError> {
        normalize_team_settings(&mut settings.teams)?;
        let community_is_unconfigured = settings.community.household_id.trim().is_empty()
            && settings.community.household_name.trim().is_empty()
            && settings.community.adults == 0
            && settings.community.children == 0
            && settings.community.pets == 0
            && settings.community.role_badges.is_empty();
        if !community_is_unconfigured {
            settings.community = normalize_community_settings(&settings.community)?;
        }
        if !matches!(settings.power.threshold_percent, 10 | 20 | 30) {
            return Err(NodeError::InvalidConfig {});
        }
        let observed_battery = {
            let inner = self.inner.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let invalidation = inner.app_state.set_app_settings(&settings)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let mut snapshot = inner.hub_directory_snapshot.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let mut next_snapshot = snapshot
                .clone()
                .unwrap_or_else(|| HubDirectorySnapshot::yellow_only(crate::runtime::now_ms()));
            apply_local_team_settings(&mut next_snapshot, &settings.teams);
            *snapshot = Some(next_snapshot.clone());
            drop(snapshot);
            inner
                .bus
                .emit(NodeEvent::HubDirectoryUpdated { snapshot: next_snapshot });
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("settings-updated".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);
            inner
                .power_state
                .battery_percent
                .map(|percent| (percent, inner.power_state.charging))
        };
        if let Some((percent, charging)) = observed_battery {
            self.update_battery_state(percent, charging)?;
        }
        let _ = self.publish_community_status();
        Ok(())
    }
}
