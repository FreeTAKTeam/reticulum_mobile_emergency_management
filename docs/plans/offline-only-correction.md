# Offline-only scope correction

The user's 2026-10-24 correction governs the implementation: local propagation
changes only delivery to unreachable peers. Preserve the software tested in
release 1.5.5 for all other traffic.

## Protected baseline

- REM: `807041681430404f06dbac9b257fa5a192bed7e9`.
- LXMF-rs: `7822b608f2cd2debd8170895bc580d01dda5356e` from the release manifest.
- Preserve normal direct/regular propagation selection, retries, packet sizing,
  native connection handling, peer aliases, caller deadlines and normal Sync.
- The earlier feature branches and private field evidence remain available.
  Both phones have been restored to the tested signed release, without clearing data.

## Correction tasks

1. Restore protected behavior and the tested dependency baseline. Keep the shared
   daemon-free custody crate additive; do not take unrelated transport updates.
2. Run the existing delivery flow first for routed traffic. The [S8 timeout
   amendment](local-propagation-routing.md#s8-timeout-correction-2026-10-24)
   adds prompt admission for known unreachable peers and persistent verified keys.
   With no active interface, admit eligible offline traffic directly rather than
   running futile send attempts against a cached Link. A successful send returns its original
   report and never writes custody. No-open-Link alone is not an offline decision.
3. At an unavailable-route failure only, validate recipient identity and current
   sharing permission, then deposit once. Preserve any failed attempt's signed
   wire through a compiled native capture API. Do not run another direct/relay
   attempt loop in the fallback. Busy, validation, permission, storage and internal
   failures are not evidence of an offline recipient.
4. Keep recovery conditional on an actual stored message. It uses compiled
   Reticulum/LXMF APIs, retains ownership through cleanup, and cannot change
   normal connection, polling or Sync policy. Existing online fanout targets stay
   unchanged; any additional offline recipients are handled separately.
   For stored messages only, an unconfirmed direct attempt may use a visible relay
   on its next scheduled attempt; fresh recipient events still prefer direct recovery.
   Publish expiry notices even in power saver, without starting network transfers.
5. Verify baseline preservation, online success with zero custody writes, explicit
   offline admission, immutable recovery and single retry ownership. Then rebuild
   an isolated candidate before repeating the two-phone chat test.

Main agent owns edits. Explorer and independent review roles are read-only.
Affected layers are the native delivery boundary, the additive compiled custody
API, its tests, dependency pins and documentation. Existing custody status UI and
bridge contracts remain additive. No new daemon, protocol encoder, global
timeout, radio profile, queue deletion, or release publication is part of this
correction.

Earlier host test results do not qualify the corrected implementation. Hardware
acceptance and release remain open until the scoped candidate is tested.
