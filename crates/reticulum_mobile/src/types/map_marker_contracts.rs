/// A point marker projected from the shared `r3akt-map-core` record.
///
/// `object_destination_hash` is the authoritative identifier shared with RCH.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapMarkerRecord {
    pub object_destination_hash: String,
    pub origin_rch: String,
    pub marker_type: String,
    pub symbol: String,
    pub name: String,
    pub category: String,
    pub lat: f64,
    pub lon: f64,
    pub notes: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

/// One entry of the backend-owned shared symbol catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapMarkerSymbolRecord {
    pub id: String,
    pub set: String,
    pub mdi: Option<String>,
    pub description: String,
    pub tak: Option<String>,
    pub category: String,
    /// Input spellings that normalize to this symbol (for search).
    pub aliases: Vec<String>,
}

/// Fields for a new marker. Validation and normalization are owned by
/// `r3akt-map-core`; the optional key makes creation idempotent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapMarkerCreateRequest {
    pub idempotency_key: Option<String>,
    pub marker_type: String,
    pub symbol: String,
    pub name: Option<String>,
    pub category: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapMarkerCreateResult {
    pub marker: MapMarkerRecord,
    /// `false` when an idempotency key replayed an earlier creation.
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapMarkerRenameRequest {
    pub object_destination_hash: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapMarkerMoveRequest {
    pub object_destination_hash: String,
    pub lat: f64,
    pub lon: f64,
}
