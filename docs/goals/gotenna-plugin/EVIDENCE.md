# goTenna addressed chat and event-code evidence — 2026-11-04

This supersedes the initial 0.1.0 plugin-inbox model, which the user rejected.
Plugin **0.2.0** uses explicit recipient hashes for chat and canonical MECP event codes.

## Implemented outcome

- REM1 chat frames carry stable ID, reported source hash and recipient LXMF destination hash. Missing recipients/unaddressed Shouts never become REM chat.
- Local admission uses the runtime message import command and existing persistence/cache/projections, so chat is visible in ordinary REM conversations. Mesh source claims have a native-forced unverified badge; no trusted peer identity is manufactured.
- Nonlocal chat uses compiled LXMF addressed to the carried recipient, signed by the real gateway and explicitly attributed. Durable per-gateway claims/results prevent blind retries; unresolved attempts remain unconfirmed, never falsely accepted duplicates.
- Outgoing chat history is queued/unconfirmed before BLE writes, then upgraded only after radio acceptance. A separate pending receipt survives native-history recording failures without retransmitting bytes. No recipient delivery is inferred.
- Local and received REM events enter the feed as canonical MECP codes (for example `MECP/2/P01`). Mesh events use source-scoped stable IDs, gateway identity and unverified goTenna attribution in the native event timeline and replication flow.
- Settings have no inbox or fixed forwarding recipient. Raw receive custody remains internal. Partial assemblies checkpoint before radio queue advancement; completed inbound admissions and sent receipts have separate durable slots. Terminal invalid/not-local payloads cannot permanently block reception.
- Existing position exchange, native grants, default-off public forwarding, source-size limits and unrelated dirty telemetry work were preserved. No UniFFI contract or generated mobile bindings changed for this correction.

## Checks observed

Before commit, the goTenna changes were isolated from the unrelated R3S telemetry edits and checked against base `5c88839` with the committed LXMF pin `ea30c23da03bc12ddd2b08aa9ac0a0763536f6f9` and unchanged Cargo manifests/lockfile. The results below are from that clean snapshot.

- `cargo test --locked --manifest-path crates/reticulum_mobile/Cargo.toml --lib --quiet`: **535 passed, 0 failed, 2 ignored**. Log: `output/gotenna-commit-native-tests.log`.
- Focused native relay tests passed for recipient validation, normal runtime history/cache visibility, forced local source, pending→radio-accepted status upgrade, grant revocation, source-scoped event dedup, local/remote MECP event feed, compiled nonlocal forwarding to the carried recipient, and unresolved forwarding outcomes.
- Real compiled LXMF traffic crossed an isolated local TCP pair in the chat/event/telemetry checks. These are software peers, not physical radio.
- `:gotenna-plugin:testDebugUnitTest`: **11 passed, none skipped** (10 protocol/assembly/vector cases plus typed host rejection handling). Includes the actual native poll fixture, recipient on every chat fragment, Unicode/out-of-order reassembly, conflicting duplicates, canonical event code, original wire/location vectors and restart/expiry of bounded fragment snapshots.
- `:gotenna-plugin:lintDebug` and `:gotenna-plugin:assembleDebug` passed with the local REM debug host signing fingerprint configured.
- `npx playwright test e2e/gotenna-routing.spec.ts --workers=2`: **6 passed**, covering desktop and mobile normal-chat provenance/status, MECP event timeline, absence of plugin inbox and mandatory per-message destination.
- Node-client tests (**46 passed**), mobile typecheck, web build, Rust formatting, source-size (**729 files**) and diff checks passed. The maximum source/class limit remains 500 lines with no added exceptions.
- POST correctness/maintainability review found no remaining blocker or major finding after fixes to native forwarding outcomes, event ID namespaces, checkpoint-before-ACK ordering, lifecycle ownership and durable sent-history receipts.

## Artifacts and qualifications

- `output/gotenna-plugin/rem-gotenna-mesh-0.2.0-debug.apk`
- `output/gotenna-plugin/SHA256SUMS`
- `output/gotenna-plugin/BUILD-INFO.txt`
- [Setup and wire format](../../../plugins/gotenna/README.md)

The listed APK was produced before isolation; the same plugin APK build and lint also passed in the clean snapshot. The APK uses the workstation's debug signer and trusts the local REM debug-host certificate. It requires the matching updated native REM API 1.5 host; an older API 1.5 build lacking these new JSON operations is insufficient. No host APK, release-signed bundle or publication was produced by this correction. Build artifacts are ignored output.

Physical BLE/GATT, Android Binder and preferences I/O fault/restart timing, radio range/delivery, and real cross-device forwarding remain **implemented but unproven**. An Android phone appeared in the device list during final checks; it was not installed to or used for radio tests. No public radio transmission was made. Mesh headers remain plaintext/unverified. Dedup is per gateway; several gateways can independently forward a reception, so this is not network-wide exactly-once delivery.
