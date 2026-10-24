# Goal: Daemon-free local propagation

Implement `../../plans/local-propagation-routing.md` using Krypton Execution.
The user authorized implementation and explicitly waived Phase 0 on 2026-10-23.
Keep LXMF-rs as the propagation owner; do not import the daemon or add a REM queue.
Preserve the earlier unapproved routing worktree unchanged for reference.

## Current scope correction (2026-10-24)

The user rejected changes to online delivery. The [offline-only correction](../../plans/offline-only-correction.md)
is authoritative. Both phones are restored to the exact signed public 1.5.5 APK
(`ce5c8324…`) with original radio frequency and telemetry preferences, preserving app data.

The corrective library branch starts at the tested `7822b608` rather than the
newer transport revision. REM's normal delivery body, Connect/aliases, caller
waits, packet sizing and normal relay Sync/polling are restored. Native custody
starts only after an eligible unreachable-route failure; it does not run a second
send loop. Additional offline recipients do not alter the baseline online target set.

Earlier implementation/checkpoint evidence below is historical and does not
qualify this correction. Final source pins, Android rebuilding and hardware
acceptance remain open. Phase 3 remains deferred.

## Historical task board

- [x] Re-read authoritative plan; create clean REM and pinned LXMF-rs worktrees.
- [x] Phase 0: skipped at the user's explicit instruction; no new measurement claimed.
- [x] Phase 1: extract shared LXMF-rs propagation crate; native storage/recovery contracts and daemon delegation.
- [x] Phase 1 gate: daemon/shared-crate tests and Python interoperability.
- [x] Phase 2 implementation: native REM bridge, one retry owner, status projection, route-aware recovery, bounded retrieval and UI.
- [ ] Phase 2 gate: host/native/UI checks and physical acceptance matrix.
- [ ] Phase 3: only if Phase 2 evidence demonstrates asymmetric need.
- [ ] Independent correctness, maintainability and completion audits.

## Execution decisions

- Existing native send/path timeout bounds remain the starting point; total initial TCP deadline 60 seconds, LoRa deadline 180 seconds.
- Handoff that cannot prove remote possession remains UploadedUnverified and retains the local payload until recipient delivery proof, expiry or recorded eviction.
- Optional retrieval uses the REM-owned `rem.fetch` aspect and `rem.local_propagation_fetch.v1` capability.
- LoRa polling starts from the approved proposed 300-second interval; the user waived Phase 0 measurement. Apply a conservative bounded transfer cap and preserve live hardware acceptance as unverified until exercised.
- No phone installation, queue reset, or release publication is implied by a passing host suite.

## Qualified software checkpoint

- LXMF-rs branch `corvo/local-propagation`, immutable revision `14880b5447430227d898a58a62d413e6f05622e6`, draft PR https://github.com/FreeTAKTeam/LXMF-rs/pull/679. REM CI now pins that pushed revision.
- Shared schema, SQL, validation, retention, full standard `/get` and `/offer` handler bodies, and sender-only custody are implemented. Daemon adapters retain the existing writer, authentication, counters and bounded processing. The approved receiver-confirmed `/get` amendment changes premature completion semantics only.
- One retained native owner holds transfer admission through Resource completion or observed cancellation. Store leases recover conservatively from SQLite finalization failures; tests cover cancellation, disconnect, timeout, storage rejection, restart and stale completion.
- Local custody is the only retry/ACK owner after admission. Direct recovery does not create REM pending ACK timers. Authenticated negative application results stay rejected; late results cannot revive expired or evicted payloads.
- Native telemetry projections keep their custody owner across route changes and restart. Chat history persists before custody admission, so payload retirement cannot erase the only chat body after an interrupted command.
- REM UDL/JNI/Java/TS/chat status integration is implemented. Desktop/mobile Playwright confirms StoredLocally -> Delivered and Evicted/relay-copy notices. Bindings and Android native outputs are generated, ignored build artifacts; no hand-edited generated code is committed.
- Host evidence: 509 REM tests, 41 library/protocol/runtime tests, 680 daemon tests, 238 RPC tests (one existing ignored), 171 app unit tests, 44 node-client tests, typecheck, builds, 500-line gates, embedded no-RPC/ZeroMQ gate, and pinned Python upload/retrieval smokes. The final current-source REM/Android packaging runs are recorded below when finished.

## Open gates

- The user approved the relay correction. Receiver-confirmed `/get` and compiled bounded native retrieval are implemented; host and Android validation pass; current hosted checks and physical acceptance remain separate.
- Oversized unpaginated inventories, unknown sender identities and unproven application bundles remain relay-held. No new persistent signer cache or SOS persistence aggregation is included.
- Physical routing matrix rows 1–6 and new-release qualification remain unperformed for this implementation. Existing two-phone results from REM 1.5.5 do not prove this new implementation.
- Optional Phase 3 remains deferred; no asymmetric field evidence has justified it.
- No phone installation, queue manipulation, new release publication or iOS build was performed.

## Independent reviews

- POST plan review: partially aligned because bounded LoRa retrieval and physical acceptance remain open.
- Correctness review: fixed duplicate telemetry ownership, permission rechecks, premature terminal-notice acknowledgement, negative-result handling, stale saved-address resolution and duplicate recovery ACK timers.
- Maintainer review: fixed divergent LoRa Link timing, idle/full-history scans, and silent lease-finalization failure. Chat reconciliation now runs only for chat custody; telemetry/mission rows do not scan chat history.
- Verifier: shared/runtime tests, strict Clippy, source-size and embedded dependency checks pass. Full goal completion is not claimed while the open gates remain.

## Final checkpoint evidence

- Current-source full REM suite: 509 passed (70.08 s).
- Android: all three native ABIs built with 16 KiB ELF checks; Kotlin regenerated; Android-only Capacitor sync completed; `testDebugUnitTest` passed, including 56 app tests. The plugin discovery fixture now expects and explicitly locates both new native methods.
- Typecheck, mobile build and all JS unit suites pass (7 source-size, 171 app, 44 node-client tests). Desktop/mobile local-propagation Playwright evidence was inspected earlier in this implementation.
- Final source-size check: 697 first-party files, no ratcheted exceptions. Shared Rust module-size and feature-isolation checks pass. No compiler, source-size or dependency limit was weakened.
- `ktlint` is unavailable, so codegen could not auto-format generated Kotlin; generation and Android compilation succeeded.
- Hosted CI remains separate from these local results; draft PRs are not releases. The relay amendment is approved and implemented; the field matrix remains open.

## Hosted CI follow-up

- On REM `677b14e`, hosted Rust tests, the complete Playwright matrix, all web checks and dependency audits passed. Formatting and strict Clippy found source issues in the new integration; those were fixed without weakening checks.
- Exact REM CI formatter and strict Clippy commands now pass locally, including the JNI macro. The full 509-test suite and stopped-runtime status regression pass after the fixes.
- LXMF workspace Clippy passes on the CI stable toolchain (1.99.0). The existing integration guard against ignored errors passes after explicit channel-close and poisoned-lock diagnostics. Runtime cancellation/timeout/disconnect tests remain green.
- The dependency pin follows LXMF `0d127f90`; hosted checks on both follow-up revisions remain pending. Android native artifact qualification above belongs to REM `677b14e` / LXMF `5d054d87`; final release packaging must rebuild after the relay-policy amendment.

## Embedded API completion audit

- The final audit found that shared `/get` extraction alone did not provide the explicit Phase 1 `retrieve_for(authenticated_identity)` embedded API. That gap is now fixed in LXMF `14880b54` without activating Phase 3.
- The library API derives its destination from the host-authenticated identity, filters expired/terminal rows, checks current permission against the original signed wire, and returns one bounded original recipient ciphertext. It changes no custody/attempt/retry state. The runtime guard retains the shared transfer permit.
- Tests prove recipient isolation, denied sharing, expiry/retirement/stopped filtering, immutable repeat retrieval, larger-budget retry after skipping oversized entries, and shared admission ownership. All 44 library/protocol/runtime tests pass; strict workspace Clippy, source-size, embedded dependency and silent-failure gates pass.
- Independent review found no blocking API issue. Authentication remains the host's native Link responsibility, and any later Phase 3 sender must retain the guard until native completion/cancellation. No endpoint, announce, capability or poller was added.
- Main CI passed for REM `5bbcdb5` and LXMF `0d127f90`; REM's Android plugin/APK/Binder job also passed. The current dependency pin now follows `14880b54`, so its hosted reruns remain separate from that prior evidence.

## Approved relay correction

- Response construction and size exclusion no longer complete recipient custody. Interrupted fetch and larger-cap retry tests pass; authenticated `haves` still retire known recipient entries.
- The native library owns inventory/fetch/haves RequestReceipts with response caps and cleanup after caller cancellation. It verifies the requested transient, recipient and sender before invoking durable import; both Python and Rust confirmation shapes work.
- REM uses that importer for chat and supported operational projections. Partial marker/checklist batches and database failures cannot confirm; corrected retries accept already-saved operations. Malformed mission envelopes cannot masquerade as saved chat.
- The legacy REM fetch/control/crypto compatibility code is removed. Existing response wrapper and legacy-salt compatibility now live in compiled LXMF-rs.
- Source signature lookup falls back to a bounded native path request. Public signing keys are not currently persisted by REM; a missing offline sender identity defers confirmation safely.

## Receiver-confirmed validation

- LXMF dependency pin: `2e04e012b05402195e02e05e8cdcd5accad23fb9`.
- Full REM suite: 499 passed; the lower count reflects removing obsolete compatibility-code tests after moving that implementation into LXMF-rs. Four new receive-proof tests cover partial batches, replay, malformed chat envelopes and database failure.
- Shared/protocol/runtime: 50 passed; core wire tests pass; daemon binary: 681 passed. The separate pinned-Python native-client test passed with actual durable save and relay retirement.
- Both existing pinned-Python daemon smokes pass. Strict Clippy, formatting, 694-file REM source-size gate, LXMF module-size and embedded dependency isolation pass.
- Android native rebuild: all three ABIs and 16 KiB ELF checks pass; Kotlin regenerated; Android-only Capacitor sync, 56 unit tests and debug APK assembly pass. Typecheck, mobile build and all JS unit suites pass.
- New hosted LXMF Verify native Python custody step passed; full hosted workflows remain in progress. Read-only ADB inventory shows only Pixel 7 attached; Poco remains untouched. Physical routing acceptance is not claimed.
