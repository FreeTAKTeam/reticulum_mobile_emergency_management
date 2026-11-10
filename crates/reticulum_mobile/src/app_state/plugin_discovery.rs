impl AppStateStore {
    pub fn sync_discovered_plugins(
        &self,
        discovered: &[DiscoveredPluginRecord],
    ) -> Result<ProjectionInvalidation, NodeError> {
        let existing = self
            .list_plugins()?
            .into_iter()
            .map(|plugin| (plugin.discovered.plugin_id.clone(), plugin))
            .collect::<HashMap<_, _>>();
        let trusted_fingerprints = self
            .list_trusted_plugin_publishers()?
            .into_iter()
            .map(|publisher| normalize_fingerprint(publisher.fingerprint.as_str()))
            .collect::<std::collections::HashSet<_>>();
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        let mut seen = std::collections::HashSet::new();
        let updated_at_ms = now_ms();

        for item in discovered {
            let mut item = item.clone();
            normalize_discovered_plugin(&mut item)?;
            if !seen.insert(item.plugin_id.clone()) {
                return Err(NodeError::InvalidConfig {});
            }
            let signer_set = std::iter::once(item.publisher_fingerprint.clone())
                .chain(item.publisher_history.iter().cloned())
                .map(|value| normalize_fingerprint(value.as_str()))
                .collect::<std::collections::HashSet<_>>();
            let trusted = signer_set
                .iter()
                .any(|fingerprint| trusted_fingerprints.contains(fingerprint));
            let previous = existing.get(item.plugin_id.as_str());
            let same_signer = previous.is_some_and(|plugin| {
                signer_set.contains(&normalize_fingerprint(
                    plugin.discovered.publisher_fingerprint.as_str(),
                ))
            });
            let mut granted = if same_signer {
                previous
                    .map(|plugin| plugin.granted_capabilities.clone())
                    .unwrap_or_default()
            } else {
                PluginCapabilityRecord::default()
            };
            intersect_capabilities(&mut granted, &item.declared_capabilities);
            let compatible = item.api_major == 1 && item.api_minor <= 5;
            let enabled = previous.is_some_and(|plugin| plugin.enabled)
                && same_signer
                && trusted
                && compatible;
            let state = if !compatible {
                "Incompatible"
            } else if !trusted {
                "Untrusted"
            } else if !enabled {
                "Disabled"
            } else {
                "Stopped"
            };
            let record = InstalledPluginRecord {
                discovered: item,
                state: state.to_string(),
                trusted,
                enabled,
                granted_capabilities: granted,
                diagnostic: None,
                updated_at_ms,
            };
            write_plugin_tx(&transaction, &record)?;
            if previous.is_some() && !same_signer {
                // A new signer must not inherit edit rights over markers the
                // old one created; the shared markers themselves stay.
                transaction
                    .execute(
                        "DELETE FROM plugin_map_markers WHERE plugin_id = ?1",
                        params![record.discovered.plugin_id],
                    )
                    .map_err(|error| {
                        crate::error_context::contextual_node_error(NodeError::IoError {}, error)
                    })?;
            }
        }

        for (plugin_id, previous) in existing {
            if seen.contains(plugin_id.as_str()) {
                continue;
            }
            let mut missing = previous;
            missing.state = "Missing".to_string();
            missing.diagnostic = Some("Plugin package is not installed".to_string());
            missing.updated_at_ms = updated_at_ms;
            write_plugin_tx(&transaction, &missing)?;
        }

        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Plugins {},
            None,
            Some("plugin-discovery-synchronized".to_string()),
        )?;
        transaction.commit().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        Ok(invalidation)
    }

}
