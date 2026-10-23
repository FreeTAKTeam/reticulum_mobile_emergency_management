# Telemetry reliability and REM 1.5.5 Implementation Plan

**Intent:** Implement milestone 13 (#272–#276) and prepare a signed Android release candidate.
**Current Behavior:** Telemetry selects live, already linked peers, advances cadence without recipients, discards unsent fixes, and silently drops invalid fields.
**Expected Outcome:** Authorized telemetry can initiate a direct link; the latest pending fix survives readiness changes/restart; operators see local acquisition separately from per-recipient transmission and application acknowledgement.
**Target-Perspective Output:** A receiver stores the first telemetry workflow after startup and the sender shows its correlated application result. Location settings show local acquisition, pending/delivery outcomes, and the latest privacy-safe receive rejection.
**Truth Owner:** Rust Node/AppStateStore and the compiled LXMF-rs runtime. Existing local-team membership, capability, hub exclusion and send admission remain authoritative.
**Contract Boundary:** Native publication snapshot and existing LxmfDelivery updates, exposed through JNI/Capacitor/node-client; Vue only renders projections.
**Cutover:** Replace synchronous best-effort telemetry fan-out with a persisted coalesced pending publication and a native lifecycle worker using the existing queue_bytes_with_class path.
**Displaced Path:** Remove telemetry's active-link prerequisite and cadence-before-target best-effort send loop. Retain warm-link coverage and shared LXMF send/ACK code.
**Value Density:** Five issue fixes share one bounded native publication projection and existing delivery machinery.
**Acceptance Evidence:** Routing matrices, persisted pending/restart tests, telemetry-first two-node persistence and correlated ACK trace, frontend status mapping/E2E, full applicable checks, signed APK/AAB verification, exact-build field matrix.
**Evidence Lane:** Deterministic host/browser tests and signed packaging are distinct from two-phone physical LoRa evidence. Do not mark the milestone field-qualified without the latter.
**Kill Criteria:** No UI retry engine, protocol implementation, arbitrary sleeps, active-link warmup requirement, unbounded position/event queue, or transport receipt promoted to application acceptance.
**Architecture Slice:** node/replication_targets.rs and messaging_policy.rs plan from live plus saved state; node/events_plugins_telemetry.rs admits local fixes; a focused native telemetry publication module owns coalescing; lifecycle.rs owns worker lifetime; runtime delivery and ACK helpers persist correlated outcomes; a typed receive decoder replaces Option parsing in eam_persistence.rs/receive.rs. app_state persists the pending publication. Existing JNI/Java/plugin/node-client adapters expose the projection. SettingsLocationView and telemetryStore render it.
**Plan Review Gate:** PRE reviewed: aligned, no blocker. Recovery/recipient-intent/decoder-gate clarifications accepted.

## Ordered task board

1. **Routing (#272/#273).** Modify native telemetry planning and its callers/tests only. Merge normalized live/saved aliases under existing active-local-team policy; fresh capability evidence wins; permit on-demand direct attempts without an active link; preserve relay/Connected hub rules. Verify focused routing tests and destination matrices.
2. **Publication (#274).** Add a focused persisted native publication state and lifecycle worker; coalesce to this device's newest pending fix per authorized destination/callsign, prune removed recipients, revalidate before dispatch, retain command correlation across retries, and consume existing runtime delivery outcomes. Keep native cadence/power admission. Verify zero targets, late route, superseding fixes, partial fan-out, stale outcomes, restart and timeout.
3. **Receive diagnostics (#276).** Add typed decode outcomes for recognized telemetry and correlated rejected results using existing result encoding; expose only identifiers and reason codes, never coordinate values. Verify table-driven compact/legacy fields, missing/type/value/version/MessagePack failures, persistence failure, marker exclusion and non-telemetry silence.
4. **Projection/UI (#274/#276).** Extend the existing native/client boundary with a publication snapshot; update all real and in-memory adapters and relevant tests. Show acquisition separately from delivery and latest receive failure. Regenerate UniFFI bindings if its contract changes. Verify node-client tests, typecheck, source-size, app tests, production build and focused Playwright/browser output.
5. **Cold-start evidence (#275).** Keep warm-link tests and add telemetry-first/no-connect and before-readiness/restart cases, asserting target, Packet representation, wire/command IDs, persisted receiver fix and explicit app ACK. Record host transport limitations and prepare exact signed-candidate two-phone LoRa matrix.
6. **Release preparation.** Update versions/lockfiles/Android metadata/docs to 1.5.5, run applicable local and hosted checks, create a reviewable PR, build/sign APK+AAB, verify metadata/ABIs/alignment/signatures/checksums, and prepare release notes/manifests. Do not publish or claim physical qualification without its evidence.

Implementation stays in the main agent, sequentially. Read-only explorer, PRE/POST reviewer, correctness, maintainability and verifier roles may inspect disjoint slices. No transport rewrite, LXMF dependency upgrade, unrelated UI redesign, iOS build, or edits to generated bindings by hand.

## Risks and gates

- Exact positions must remain limited to current authorized team/capability recipients; revoke queued work on membership/contact removal.
- Late outcomes must not overwrite a newer fix. Sent, transport delivered, application accepted/rejected and timeout remain separate.
- Crash recovery must preserve stable command identity and idempotent receiver projection; an in-flight send is uncertain until the explicit result arrives.
- Pending recovery is bounded by the current local fix and authorized recipient set, never a historical position backlog.
- Physical phones/RNodes and RCH installed-client evidence may be unavailable. Prepare candidate and evidence matrix, report those gates as unproven, and leave milestone qualification open.

## Reviewed recovery contract

Accepted/rejected rows never replay. Unsent/uncertain rows retain command identity; a fresh fix supersedes older rows and older outcomes cannot overwrite it. Disablement, existing configured telemetry expiry, and revoked membership cancel dispatch. Zero authorized recipients is StoredLocally; authorized saved recipients awaiting a route are QueuedNoRoute. Classify telemetry outside the successful general-metadata gate; send rejection only with sufficient correlation. The user explicitly deferred LoRa testing until the final pre-publication gate.

## Implementation evidence (software lane)

Native routing/outbox, typed receiver diagnostics, JNI projection and UI are implemented. Focused tests include telemetry-first persistence/application ACK, pending restart recovery, coalescing before interface readiness, independent fan-out failure, stale ACK/receipt ordering, typed rejection and position-free diagnostic, and out-of-order receiver idempotence. The native worker uses a weak lifetime reference; final runtime destruction inside async context uses Tokio shutdown_background (https://docs.rs/tokio/1.53.1/tokio/runtime/struct.Runtime.html#method.shutdown_background); normal explicit stop remains synchronous. The first correctness review found stored-route dispatch, concurrent fix reconciliation, out-of-order receiver regression, and plain-chat T noise; all were corrected. Final checks, packaging and final reviews are recorded in the release preparation evidence.

Final source review: correctness and maintainability found no remaining blocker; POST verdict aligned. Software checks passed: 494 Rust tests; full 128 browser tests, followed by 12 telemetry browser cases after the last UI refinement; 7 source-size tests, 171 app unit tests and 40 node-client tests; strict typecheck; source-size; builds; Rust fmt/clippy; npm audit (0 vulnerabilities). cargo-audit reports no vulnerabilities and three upstream unmaintained-package warnings. Vitest was patched to 4.1.12 after the release audit. Android native libraries are being rebuilt and the signed-candidate packaging evidence is kept with the ignored release artifacts. Physical tests remain pending by user instruction.

Current main was merged before candidate packaging, bringing the SOS normalized-identity deduplication fix and its six unit tests. Candidate web assets were rebuilt and Android sync/packaging repeated after that merge. UniFFI Kotlin output was regenerated using the existing workspace bindgen tool because the shell helper skips generation when the CLI is absent; the UDL contract is unchanged. Generated output and signed binaries remain ignored.
