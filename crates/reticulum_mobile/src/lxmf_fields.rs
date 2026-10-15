// Shared LXMF field IDs used by REM mobile.
//
// `FIELD_COMMANDS` remains `0x09` in this workspace for:
// - RCH-compatible mission command envelopes
// - SOS command envelopes
// - telemetry snapshot requests
//
// The parser boundary between those payload families is defined by their inner
// envelope keys, not by different numeric field IDs.
pub(crate) use r3akt_mission_wire::{FIELD_COMMANDS, FIELD_EVENT, FIELD_RESULTS};
