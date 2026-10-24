# Goal: Daemon-free local propagation

Implement `../../plans/local-propagation-routing.md` using Krypton Execution.
The user authorized implementation and explicitly waived Phase 0 on 2026-10-23.
Keep LXMF-rs as the propagation owner; do not import the daemon or add a REM queue.
Preserve the earlier unapproved routing worktree unchanged for reference.

## Task board

- [x] Re-read authoritative plan; create clean REM and pinned LXMF-rs worktrees.
- [x] Phase 0: skipped at the user's explicit instruction; no new measurement claimed.
- [x] Phase 1: extract shared LXMF-rs propagation crate; native storage/recovery contracts and daemon delegation.
- [x] Phase 1 gate: daemon/shared-crate tests and Python interoperability.
- [ ] Phase 2: native REM bridge, one retry owner, status projection, route-aware recovery and UI.
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

- LXMF-rs branch `corvo/local-propagation`, immutable revision `5d054d8750fafbd98f394225f083a9d356b65ca7`, draft PR https://github.com/FreeTAKTeam/LXMF-rs/pull/679. REM CI now pins that pushed revision.
- Shared schema, SQL, validation, retention, full standard `/get` and `/offer` handler bodies, and sender-only custody are implemented. Daemon adapters retain the existing writer, authentication, counters, bounded processing and custody behavior.
- One retained native owner holds transfer admission through Resource completion or observed cancellation. Store leases recover conservatively from SQLite finalization failures; tests cover cancellation, disconnect, timeout, storage rejection, restart and stale completion.
- Local custody is the only retry/ACK owner after admission. Direct recovery does not create REM pending ACK timers. Authenticated negative application results stay rejected; late results cannot revive expired or evicted payloads.
- Native telemetry projections keep their custody owner across route changes and restart. Chat history persists before custody admission, so payload retirement cannot erase the only chat body after an interrupted command.
- REM UDL/JNI/Java/TS/chat status integration is implemented. Desktop/mobile Playwright confirms StoredLocally -> Delivered and Evicted/relay-copy notices. Bindings and Android native outputs are generated, ignored build artifacts; no hand-edited generated code is committed.
- Host evidence: 509 REM tests, 41 library/protocol/runtime tests, 680 daemon tests, 238 RPC tests (one existing ignored), 171 app unit tests, 44 node-client tests, typecheck, builds, 500-line gates, embedded no-RPC/ZeroMQ gate, and pinned Python upload/retrieval smokes. The final current-source REM/Android packaging runs are recorded below when finished.

## Open gates

- Automatic bounded LoRa retrieval is not implemented: capped `/get` is refused before any network request. The unchanged Rust relay prematurely suppresses wanted messages on response construction or transfer-size exclusion; a client cannot safely undo that state after cancellation or radio failure.
- The smallest proposed plan amendment is `../../plans/local-propagation-relay-correction.md`: retain wanted messages until authenticated receiver `haves` confirmation. It changes the approved preserve-custody-semantics constraint and is not applied in this checkpoint.
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
- Hosted CI remains separate from these local results; draft PRs are not releases. The relay amendment and field matrix remain open.
