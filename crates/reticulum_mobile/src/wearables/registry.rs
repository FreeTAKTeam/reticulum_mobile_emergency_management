use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::events::{WearableSensorType, WearableSensorValue};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WearableDeviceConfigRecord {
    pub device_id: String,
    pub alias: Option<String>,
    pub operator_rns_identity: Option<String>,
    pub sensor_type: WearableSensorType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WearableSettingsRecord {
    pub enabled: bool,
    pub stale_timeout_seconds: u32,
    pub devices: Vec<WearableDeviceConfigRecord>,
}

impl Default for WearableSettingsRecord {
    fn default() -> Self {
        Self {
            enabled: false,
            stale_timeout_seconds: super::ingestion::DEFAULT_WEARABLE_STALE_TIMEOUT_SECONDS,
            devices: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WearableStatusKind {
    Active {},
    Stale {},
    Offline {},
    Unsupported {},
}

impl WearableStatusKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active {} => "Active",
            Self::Stale {} => "Stale",
            Self::Offline {} => "Offline",
            Self::Unsupported {} => "Unsupported",
        }
    }
}

impl Serialize for WearableStatusKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str((*self).as_str())
    }
}

impl<'de> Deserialize<'de> for WearableStatusKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.trim().to_ascii_lowercase().as_str() {
            "active" => Ok(Self::Active {}),
            "stale" => Ok(Self::Stale {}),
            "offline" => Ok(Self::Offline {}),
            "unsupported" => Ok(Self::Unsupported {}),
            other => Err(D::Error::custom(format!(
                "unknown WearableStatusKind: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WearableStatusRecord {
    pub device_id: String,
    pub device_name: Option<String>,
    pub device_model: Option<String>,
    pub operator_rns_identity: Option<String>,
    pub sensor_type: WearableSensorType,
    pub value: WearableSensorValue,
    pub unit: Option<String>,
    pub confidence: f32,
    pub connection_state: Option<String>,
    pub last_seen_timestamp_ms: i64,
    pub stale_after_ms: i64,
    pub status: WearableStatusKind,
}
