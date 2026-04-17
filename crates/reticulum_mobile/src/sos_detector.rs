use std::collections::VecDeque;

use crate::types::{SosSettingsRecord, SosTriggerSource};

const GRAVITY: f64 = 9.81;
const SHAKE_WINDOW_MS: u64 = 1_000;
const SHAKE_SUSTAIN_MS: u64 = 500;
const TAP_WINDOW_MS: u64 = 1_200;
const TAP_DEBOUNCE_MS: u64 = 150;
const WALKING_STEP_REJECT_MS: u64 = 100;
const POWER_WINDOW_MS: u64 = 2_000;
const COOLDOWN_MS: u64 = 5_000;

#[derive(Debug, Clone)]
pub(crate) struct SosTriggerDetector {
    shake_samples: VecDeque<u64>,
    tap_spikes: VecDeque<u64>,
    power_events: VecDeque<u64>,
    last_tap_at_ms: Option<u64>,
    last_trigger_at_ms: Option<u64>,
}

impl SosTriggerDetector {
    pub(crate) fn new() -> Self {
        Self {
            shake_samples: VecDeque::new(),
            tap_spikes: VecDeque::new(),
            power_events: VecDeque::new(),
            last_tap_at_ms: None,
            last_trigger_at_ms: None,
        }
    }

    pub(crate) fn accelerometer_sample(
        &mut self,
        settings: &SosSettingsRecord,
        x: f64,
        y: f64,
        z: f64,
        at_ms: u64,
    ) -> Option<SosTriggerSource> {
        if self.in_cooldown(at_ms) {
            return None;
        }
        let magnitude = (x.mul_add(x, y.mul_add(y, z * z))).sqrt();
        if settings.trigger_shake && magnitude > settings.shake_sensitivity.max(1.0) * GRAVITY {
            self.shake_samples.push_back(at_ms);
            prune_before(
                &mut self.shake_samples,
                at_ms.saturating_sub(SHAKE_WINDOW_MS),
            );
            if self
                .shake_samples
                .front()
                .is_some_and(|first| at_ms.saturating_sub(*first) >= SHAKE_SUSTAIN_MS)
            {
                return self.mark_trigger(SosTriggerSource::Shake {}, at_ms);
            }
        }

        if settings.trigger_tap_pattern && magnitude > 2.2 * GRAVITY {
            if let Some(last) = self.last_tap_at_ms {
                let delta = at_ms.saturating_sub(last);
                if delta < TAP_DEBOUNCE_MS || delta > WALKING_STEP_REJECT_MS && delta < 260 {
                    return None;
                }
            }
            self.last_tap_at_ms = Some(at_ms);
            self.tap_spikes.push_back(at_ms);
            prune_before(&mut self.tap_spikes, at_ms.saturating_sub(TAP_WINDOW_MS));
            if self.tap_spikes.len() >= 3 {
                return self.mark_trigger(SosTriggerSource::TapPattern {}, at_ms);
            }
        }
        None
    }

    pub(crate) fn screen_event(
        &mut self,
        settings: &SosSettingsRecord,
        at_ms: u64,
    ) -> Option<SosTriggerSource> {
        if !settings.trigger_power_button || self.in_cooldown(at_ms) {
            return None;
        }
        self.power_events.push_back(at_ms);
        prune_before(
            &mut self.power_events,
            at_ms.saturating_sub(POWER_WINDOW_MS),
        );
        if self.power_events.len() >= 3 {
            return self.mark_trigger(SosTriggerSource::PowerButton {}, at_ms);
        }
        None
    }

    fn mark_trigger(&mut self, source: SosTriggerSource, at_ms: u64) -> Option<SosTriggerSource> {
        self.last_trigger_at_ms = Some(at_ms);
        self.shake_samples.clear();
        self.tap_spikes.clear();
        self.power_events.clear();
        Some(source)
    }

    fn in_cooldown(&self, at_ms: u64) -> bool {
        self.last_trigger_at_ms
            .is_some_and(|last| at_ms.saturating_sub(last) < COOLDOWN_MS)
    }
}

fn prune_before(values: &mut VecDeque<u64>, threshold_ms: u64) {
    while values.front().is_some_and(|value| *value < threshold_ms) {
        values.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sos::default_sos_settings;

    #[test]
    fn shake_requires_sustained_window() {
        let mut settings = default_sos_settings();
        settings.enabled = true;
        settings.trigger_shake = true;
        let mut detector = SosTriggerDetector::new();
        assert_eq!(
            detector.accelerometer_sample(&settings, 30.0, 0.0, 0.0, 100),
            None
        );
        assert_eq!(
            detector.accelerometer_sample(&settings, 31.0, 0.0, 0.0, 650),
            Some(SosTriggerSource::Shake {})
        );
    }

    #[test]
    fn power_button_triggers_on_three_events() {
        let mut settings = default_sos_settings();
        settings.enabled = true;
        settings.trigger_power_button = true;
        let mut detector = SosTriggerDetector::new();
        assert_eq!(detector.screen_event(&settings, 0), None);
        assert_eq!(detector.screen_event(&settings, 500), None);
        assert_eq!(
            detector.screen_event(&settings, 900),
            Some(SosTriggerSource::PowerButton {})
        );
    }
}
