use super::*;
use crate::types::AnnounceClass;

use crate::mission_sync::parse_mission_sync_metadata;
use crate::types::{
    BlockPeerTierRecord, ChecklistTaskRecord, CircleTier, EamSourceRecord, HubSettingsRecord,
    MessageDirection, MessageMethod, MessageState, SyncPhase, TelemetrySettingsRecord,
};
use crate::HubMode;
use rmpv::Value as MsgPackValue;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex as AsyncMutex, Notify};

static TEST_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

const TEST_TIMEOUT: Duration = Duration::from_secs(30);
