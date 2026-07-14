use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

macro_rules! string_enum {
    ($vis:vis enum $name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        $vis enum $name {
            $(
                $variant {},
            )+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(
                        Self::$variant {} => $value,
                    )+
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str((*self).as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                match value.trim().to_ascii_uppercase().as_str() {
                    $(
                        $value => Ok(Self::$variant {}),
                    )+
                    other => Err(D::Error::custom(format!(
                        "unknown {}: {other}",
                        stringify!($name)
                    ))),
                }
            }
        }
    };
}

include!("types/core_contracts.rs");
include!("types/runtime_contracts.rs");
include!("types/messaging_contracts.rs");
include!("types/mission_contracts.rs");
include!("types/plugin_contracts.rs");
include!("types/node_events.rs");

#[cfg(test)]
mod tests {
    include!("types/tests/contracts.rs");
}
