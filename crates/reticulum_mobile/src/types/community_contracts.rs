macro_rules! community_string_enum {
    ($name:ident, $default:ident, { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant {},)+
        }

        impl $name {
            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant {} => $value,)+
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::$default {}
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
                match value.trim().to_ascii_lowercase().as_str() {
                    $($value => Ok(Self::$variant {}),)+
                    other => Err(D::Error::custom(format!(
                        "unknown {}: {other}",
                        stringify!($name)
                    ))),
                }
            }
        }
    };
}

community_string_enum!(CircleTier, Inner, {
    Inner => "inner",
    Outer => "outer"
});

community_string_enum!(HouseholdStatus, AllHome, {
    AllHome => "all_home",
    OneMissing => "one_missing",
    Evacuated => "evacuated",
    NeedsHelp => "needs_help"
});

community_string_enum!(PreferredMapLayer, Base, {
    Base => "base",
    Satellite => "satellite"
});

community_string_enum!(OutboundTrafficClass, Chat, {
    Sos => "sos",
    Telemetry => "telemetry",
    CommunityStatus => "community_status",
    Chat => "chat",
    Eam => "eam",
    Event => "event",
    Checklist => "checklist",
    Plugin => "plugin",
    Raw => "raw",
    Control => "control"
});

impl OutboundTrafficClass {
    #[must_use]
    pub fn allowed_in_power_saver(self) -> bool {
        matches!(self, Self::Sos {} | Self::Telemetry {} | Self::CommunityStatus {})
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CommunitySettingsRecord {
    #[serde(default)]
    pub household_id: String,
    #[serde(default)]
    pub household_name: String,
    #[serde(default)]
    pub adults: u8,
    #[serde(default)]
    pub children: u8,
    #[serde(default)]
    pub pets: u8,
    #[serde(default)]
    pub role_badges: Vec<String>,
    #[serde(default)]
    pub status: HouseholdStatus,
    #[serde(default)]
    pub preferred_map_layer: PreferredMapLayer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommunityStatusProjectionRecord {
    pub household_id: String,
    pub household_name: String,
    pub adults: u8,
    pub children: u8,
    pub pets: u8,
    pub role_badges: Vec<String>,
    pub status: HouseholdStatus,
    pub saver_active: bool,
    pub updated_at_ms: u64,
    pub source_identity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PowerPolicyRecord {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_power_threshold_percent")]
    pub threshold_percent: u8,
}

impl Default for PowerPolicyRecord {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_percent: default_power_threshold_percent(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PowerStateRecord {
    pub battery_percent: Option<u8>,
    #[serde(default)]
    pub charging: bool,
    #[serde(default)]
    pub saver_active: bool,
    #[serde(default)]
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlockRadioSettings {
    pub frequency_hz: u64,
    pub profile: String,
    pub region: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlockNetworkSettings {
    #[serde(default)]
    pub broadcast: bool,
    #[serde(default)]
    pub hub_api_base_url: Option<String>,
    #[serde(default)]
    pub hub_identity_hash: Option<String>,
    #[serde(default)]
    pub hub_mode: HubMode,
    #[serde(default = "default_hub_refresh_interval_seconds")]
    pub hub_refresh_interval_seconds: u32,
    #[serde(default)]
    pub radio: Option<BlockRadioSettings>,
    #[serde(default)]
    pub tcp_clients: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockOnboardingDraft {
    pub network: BlockNetworkSettings,
    #[serde(default)]
    pub trusted_destination_hashes: Vec<String>,
    #[serde(default)]
    pub preferred_map_layer: PreferredMapLayer,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedBlockOnboardingEnvelope {
    pub encoded_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockOnboardingInspection {
    pub issuer_public_identity_hex: String,
    pub issuer_app_destination_hex: String,
    pub issuer_lxmf_destination_hex: String,
    pub signer_fingerprint: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub network: BlockNetworkSettings,
    pub trusted_destination_hashes: Vec<String>,
    pub preferred_map_layer: PreferredMapLayer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockPeerTierRecord {
    pub destination_hex: String,
    pub circle_tier: CircleTier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockOnboardingImportRequest {
    pub encoded_text: String,
    pub confirmed_signer_fingerprint: String,
    pub community: CommunitySettingsRecord,
    pub peer_tiers: Vec<BlockPeerTierRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockOnboardingImportResult {
    pub imported_peer_count: u32,
    pub settings_updated: bool,
}

fn default_power_threshold_percent() -> u8 {
    20
}

fn default_hub_refresh_interval_seconds() -> u32 {
    3600
}
