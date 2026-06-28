use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WearableSensorEvent {
    #[serde(rename = "type", alias = "event_type")]
    pub event_type: String,
    pub source: String,
    pub device_id: String,
    pub device_name: Option<String>,
    pub device_model: Option<String>,
    pub timestamp_ms: i64,
    pub sensor_type: WearableSensorType,
    pub value: WearableSensorValue,
    pub unit: Option<String>,
    pub confidence: f32,
    pub connection_state: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WearableSensorType {
    HeartRateBpm {},
    BatteryPercent {},
    StepCount {},
    Location {},
    Unknown {},
}

impl WearableSensorType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HeartRateBpm {} => "heart_rate_bpm",
            Self::BatteryPercent {} => "battery_percent",
            Self::StepCount {} => "step_count",
            Self::Location {} => "location",
            Self::Unknown {} => "unknown",
        }
    }
}

impl Serialize for WearableSensorType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str((*self).as_str())
    }
}

impl<'de> Deserialize<'de> for WearableSensorType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.trim().to_ascii_lowercase().as_str() {
            "heart_rate_bpm" | "heartratebpm" | "heart-rate-bpm" => {
                Ok(Self::HeartRateBpm {})
            }
            "battery_percent" | "batterypercent" | "battery-percent" => {
                Ok(Self::BatteryPercent {})
            }
            "step_count" | "stepcount" | "step-count" => Ok(Self::StepCount {}),
            "location" => Ok(Self::Location {}),
            "unknown" => Ok(Self::Unknown {}),
            other => Err(D::Error::custom(format!(
                "unknown WearableSensorType: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WearableSensorValue {
    Integer(i64),
    Float(f64),
    Text(String),
}
