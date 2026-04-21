// Shared LXMF field IDs used by REM mobile.
//
// `FIELD_COMMANDS` remains `0x09` in this workspace for:
// - RCH-compatible mission command envelopes
// - SOS command envelopes
// - telemetry snapshot requests
//
// The parser boundary between those payload families is defined by their inner
// envelope keys, not by different numeric field IDs.
pub(crate) const FIELD_COMMANDS: i64 = 0x09;
pub(crate) const FIELD_RESULTS: i64 = 0x0A;
pub(crate) const FIELD_EVENT: i64 = 0x0D;
