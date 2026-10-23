const POWER_SAVER_MIN_CADENCE_SECONDS: u32 = 300;
const POWER_SAVER_HYSTERESIS_PERCENT: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutboundAdmission {
    Allow,
    Deny,
}

fn outbound_admission(saver_active: bool, class: OutboundTrafficClass) -> OutboundAdmission {
    if !saver_active {
        return OutboundAdmission::Allow;
    }
    match class {
        OutboundTrafficClass::Sos {}
        | OutboundTrafficClass::Telemetry {}
        | OutboundTrafficClass::CommunityStatus {} => OutboundAdmission::Allow,
        OutboundTrafficClass::Chat {}
        | OutboundTrafficClass::Eam {}
        | OutboundTrafficClass::Event {}
        | OutboundTrafficClass::Checklist {}
        | OutboundTrafficClass::Plugin {}
        | OutboundTrafficClass::Raw {}
        | OutboundTrafficClass::Control {} => OutboundAdmission::Deny,
    }
}

fn ensure_outbound_admitted(
    saver_active: bool,
    class: OutboundTrafficClass,
) -> Result<(), NodeError> {
    match outbound_admission(saver_active, class) {
        OutboundAdmission::Allow => Ok(()),
        OutboundAdmission::Deny => Err(NodeError::InvalidConfig {}),
    }
}

fn next_saver_state(
    current: bool,
    policy: &PowerPolicyRecord,
    battery_percent: u8,
    charging: bool,
) -> bool {
    if !policy.enabled || charging {
        return false;
    }
    if current {
        battery_percent < policy.threshold_percent.saturating_add(POWER_SAVER_HYSTERESIS_PERCENT)
    } else {
        battery_percent <= policy.threshold_percent
    }
}

pub(crate) fn effective_power_cadence_seconds(normal_seconds: u32, saver_active: bool) -> u32 {
    if saver_active {
        normal_seconds.max(POWER_SAVER_MIN_CADENCE_SECONDS)
    } else {
        normal_seconds
    }
}

impl Node {
    pub fn update_battery_state(
        &self,
        battery_percent: u8,
        charging: bool,
    ) -> Result<PowerStateRecord, NodeError> {
        if battery_percent > 100 {
            return Err(NodeError::InvalidConfig {});
        }
        let (state, publish_transition) = {
            let mut inner = self.inner.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let (policy, telemetry_interval_seconds) = inner
                .app_state
                .get_app_settings()?
                .map(|settings| (settings.power, settings.telemetry.publish_interval_seconds))
                .unwrap_or((PowerPolicyRecord::default(), 60));
            let saver_active = next_saver_state(
                inner.power_state.saver_active,
                &policy,
                battery_percent,
                charging,
            );
            let publish_transition = saver_active != inner.power_state.saver_active;
            if publish_transition {
                inner.community_status_sent_in_saver = false;
                let telemetry_cadence = effective_power_cadence_seconds(
                    telemetry_interval_seconds,
                    saver_active,
                );
                inner.next_telemetry_publish_at_ms = Some(
                    now_ms().saturating_add(u64::from(telemetry_cadence).saturating_mul(1_000)),
                );
            }
            inner.power_state = PowerStateRecord {
                battery_percent: Some(battery_percent),
                charging,
                saver_active,
                updated_at_ms: now_ms(),
            };
            inner.power_saver_tx.send_replace(saver_active);
            inner.bus.emit(NodeEvent::PowerStateChanged {
                state: inner.power_state.clone(),
            });
            if !saver_active {
                if let (Some(capability_string), Some(tx)) =
                    (&inner.deferred_announce_capabilities, &inner.cmd_tx)
                {
                    let (resp, _) = cb::bounded(1);
                    match try_dispatch_command(tx, Command::SetAnnounceCapabilities {
                        capability_string: capability_string.clone(), resp,
                    }) {
                        Ok(()) => inner.deferred_announce_capabilities = None,
                        Err(error) => emit_replication_delivery_failure(
                            &inner.bus,
                            format!("deferred announce capabilities enqueue failed reason={error}"),
                            &error,
                        ),
                    }
                }
            }
            (inner.power_state.clone(), publish_transition)
        };
        if publish_transition {
            let _ = self.publish_community_status();
        }
        if !state.saver_active {
            // Every update outside saver, not only the transition, so holds that
            // survived a restart are sent on the first battery update.
            self.release_held_situation_messages();
        }
        Ok(state)
    }

    pub fn get_power_state(&self) -> Result<PowerStateRecord, NodeError> {
        let inner = self.inner.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        Ok(inner.power_state.clone())
    }
}
