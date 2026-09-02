impl AppStateStore {
    pub(crate) fn import_block_onboarding_atomic(
        &self,
        inspection: &BlockOnboardingInspection,
        request: &BlockOnboardingImportRequest,
    ) -> Result<BlockOnboardingImportResult, NodeError> {
        self.import_block_onboarding_transaction(inspection, request, false)
    }

    #[cfg(test)]
    pub(crate) fn import_block_onboarding_with_injected_transaction_failure(
        &self,
        inspection: &BlockOnboardingInspection,
        request: &BlockOnboardingImportRequest,
    ) -> Result<BlockOnboardingImportResult, NodeError> {
        self.import_block_onboarding_transaction(inspection, request, true)
    }

    fn import_block_onboarding_transaction(
        &self,
        inspection: &BlockOnboardingInspection,
        request: &BlockOnboardingImportRequest,
        fail_after_settings_write: bool,
    ) -> Result<BlockOnboardingImportResult, NodeError> {
        let mut expected = inspection.trusted_destination_hashes.clone();
        expected.push(inspection.issuer_app_destination_hex.clone());
        expected.sort();
        expected.dedup();
        let mut provided = request
            .peer_tiers
            .iter()
            .map(|entry| entry.destination_hex.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();
        provided.sort();
        if provided.windows(2).any(|pair| pair[0] == pair[1]) || provided != expected {
            return Err(NodeError::InvalidConfig {});
        }

        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        let raw: String = transaction
            .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| row.get(0))
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InvalidConfig {}, error)
            })?;
        let mut settings: AppSettingsRecord = deserialize_json(&raw)?;
        settings.tcp_clients = inspection.network.tcp_clients.clone();
        settings.broadcast = inspection.network.broadcast;
        settings.hub.mode = inspection.network.hub_mode;
        settings.hub.identity_hash = inspection
            .network
            .hub_identity_hash
            .clone()
            .unwrap_or_default();
        settings.hub.api_base_url = inspection
            .network
            .hub_api_base_url
            .clone()
            .unwrap_or_default();
        settings.hub.refresh_interval_seconds = inspection.network.hub_refresh_interval_seconds;
        if let Some(radio) = &inspection.network.radio {
            settings.rnode.region = radio.region.clone();
            settings.rnode.profile = radio.profile.clone();
            settings.rnode.frequency_hz = radio.frequency_hz;
        }
        settings.community = request.community.clone();
        settings.community.preferred_map_layer = inspection.preferred_map_layer;
        self.write_app_settings_tx(&transaction, &settings)?;
        if fail_after_settings_write {
            return Err(NodeError::IoError {});
        }

        for tier in &request.peer_tiers {
            let destination = tier.destination_hex.trim().to_ascii_lowercase();
            let existing: Option<String> = transaction
                .query_row(
                    "SELECT json FROM saved_peers WHERE destination_hex = ?1",
                    params![destination],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|error| {
                    crate::error_context::contextual_node_error(NodeError::IoError {}, error)
                })?;
            let mut peer = existing
                .as_deref()
                .map(deserialize_json::<SavedPeerRecord>)
                .transpose()?
                .unwrap_or(SavedPeerRecord {
                    destination_hex: destination.clone(),
                    label: None,
                    saved_at_ms: now_ms(),
                    identity_hex: None,
                    lxmf_destination_hex: None,
                    app_data: None,
                    display_name: None,
                    last_route_seen_at_ms: None,
                    last_hops: None,
                    circle_tier: CircleTier::Outer {},
                });
            peer.circle_tier = tier.circle_tier;
            peer.saved_at_ms = now_ms();
            self.write_saved_peer_tx(&transaction, &peer)?;
        }
        self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::AppSettings {},
            None,
            Some("block-onboarding-import".to_string()),
        )?;
        self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::SavedPeers {},
            None,
            Some("block-onboarding-import".to_string()),
        )?;
        transaction.commit().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        Ok(BlockOnboardingImportResult {
            imported_peer_count: crate::numeric::usize_to_u32_saturating(expected.len()),
            settings_updated: true,
        })
    }
}
