# P0 Community Foundation Implementation Plan

**Intent:** Complete the four open P0 product issues (#235-#238) as one coherent community-safety foundation: household identity, explicit privacy circles, signed block onboarding, and automatic battery-aware operation.
**Current Behavior:** REM presents color teams and individual callsigns, shares telemetry with the active team without a per-peer privacy tier, exports an unsigned local-team QR payload, and has no Android battery policy connected to Reticulum scheduling or chat availability.
**Expected Outcome:** A household can describe itself and publish one-tap MECP status, classify peers as Inner or Outer Circle, onboard a neighbor from a signed Block Code, and automatically enter a visible low-power mode that throttles mesh activity and disables ordinary chat while preserving SOS and position publication.
**Target-Perspective Output:** On Android, a neighborhood organizer can create a signed Block Code; a new user can scan it, review the signer and imported network data, name their household, choose role badges and the organizer's trust tier, then see household summaries, circle badges, status, and battery-saver badges in peers/dashboard. An Outer Circle peer never appears in the exact-position destination set and cannot be selected for outgoing chat.
**Truth Owner:** Rust `AppStateStore` owns persisted settings, community projections, saved-peer privacy tiers, Block Code validation/commit, and queued-message traffic classes. The Rust runtime owns destination policy, saver admission, retry policy, and cadence. Android's long-lived `ReticulumNodeService` owns battery observation. Vue/Pinia owns form state and presentation only.
**Contract Boundary:** Typed community, power, traffic-class, and Block Code records cross Rust/JNI/Capacitor/node-client boundaries. A versioned compact REM MECP event extension carries household status/profile without changing EAM semantics.
**Cutover:** New saves default to Outer Circle; existing saved peers without a tier deserialize as Inner Circle to preserve deployed behavior. New QR exports use signed `rem.block-onboarding` records; legacy `rem.local-team` JSON/QR remains import-only compatibility. Existing canonical team UIDs remain the wire/routing seam while product wording moves to Circles.
**Displaced Path:** Unsigned local-team QR is no longer the default export; unclassified peers no longer receive exact telemetry or new chat; the fixed announce timer is replaced by a runtime-adjustable power-aware scheduler.
**Value Density:** Each slice directly satisfies one or more acceptance criteria and reuses native settings, saved-peer, event projection, announce, telemetry, and QR owners already present.
**Acceptance Evidence:** Focused unit/Rust tests prove normalization, signed-envelope fixed vectors and tamper rejection, transactional import, privacy filtering, power traffic-class enforcement, retry behavior, merge/staleness rules, and cadence policy. Playwright exercises household setup/status, tier changes, signer-fingerprint confirmation, Block Code review/import, and battery UI. Android JVM/instrumentation coverage proves service-to-JNI battery mapping; a debug Android run or explicit environment report captures live battery-to-power-mode behavior.
**Evidence Lane:** Automated host tests first, Android unit/build second, browser flows third, physical-device camera/battery/mesh behavior when hardware is available.
**Kill Criteria:** No second household store, no EAM masquerading as MECP household status, no TypeScript signing/canonicalization authority, no private Reticulum identity in QR, no UI-only telemetry or saver filter, no exact GPS routed through a Hub lacking final-recipient policy, no duplicate LXMF implementation, and no legacy unsigned QR export button after cutover.
**Architecture Slice:** Native persisted community settings and peer policies feed runtime routing and app projections; a service-owned Android battery signal drives Rust policy and Pinia presentation; MECP remains the status transport; a signed Block Code is a configuration envelope only.
**Plan Review Gate:** Requires an aligned PRE review before execution.

## Exact contracts and policy

### Community status MECP extension

- Code: add `B04 Household/community status` to the existing MECP codebook. The event body is `MECP/2/B04 #HH_<household-id> REMCS1:<base64url(canonical-json)>`.
- Canonical JSON is produced and validated by Rust with lexicographically ordered keys and no insignificant whitespace. Version 1 fields are: `v` (`1`), `h` (16 lowercase hex household id), `n` (trimmed household name, 1-64 UTF-8 characters), `a`/`c`/`p` (adults/children/pets, integers 0-20), `r` (0-5 unique normalized role strings, each 1-24 characters), `s` (`all_home`, `one_missing`, `evacuated`, or `needs_help`), `b` (power-saver boolean), and `u` (sender update time in epoch milliseconds).
- Native publish creates `EventProjectionRecord` with `command_type = "event.create"`, `mission_uid = "rem-community"`, `uid = "rem-community-status-v1:<source_identity>"`, topic `rem.community-status.v1`, and content as above. Source identity is taken from the signed LXMF envelope/runtime, never from the payload.
- The native community projection merge key is source identity. A newer `u` replaces the record; an equal or older `u` is ignored. Future timestamps beyond five minutes and records older than seven days are rejected for community projection. UI marks records stale after 24 hours and hides status/composition after seven days while retaining the saved-peer/callsign fallback. Malformed or unknown-version content remains visible as a generic event but cannot update the community projection.
- The record intentionally contains no GPS coordinates, medical notes, free-form chat, or private identity material. It is published at initial profile completion, every profile/status/power-saver change, and is replayed to late joiners through the existing persisted event-replication snapshot.

### Circle privacy

- `SavedPeerRecord.circle_tier` is `inner` or `outer`. Missing legacy values normalize to `inner`; every explicit new save/import requires a tier and defaults to `outer` in the UI.
- Exact telemetry is eligible only for directly addressed saved peers whose tier is `inner`. Connected-Hub exact telemetry is fail-closed because the Hub does not expose final-recipient privacy policy. Outer peers receive only the GPS-free community-status event.
- New chat and retry require an Inner saved peer. Native checks are authoritative; UI checks explain the denial and prevent futile selection.

### Signed Block Code

- Rust owns typed `BlockNetworkSettings`, `BlockRadioSettings`, `BlockOnboardingDraft`, `SignedBlockOnboardingEnvelope`, `BlockOnboardingInspection`, and `BlockOnboardingImportRequest/Result` contracts. Envelope kind/version are `rem.block-onboarding`/`1`; the complete encoded QR text is capped at the largest representable `REMBC1:` URL-safe-unpadded-Base64 length below the 2,000-byte product ceiling (1,999 UTF-8 bytes), with 32 trusted destinations, 32-byte hashes, and expiry no more than seven days after issue.
- `BlockNetworkSettings` is an allowlist, not `NodeConfig`: `tcp_clients`, `broadcast`, `hub_mode`, `hub_identity_hash`, `hub_api_base_url`, `hub_refresh_interval_seconds`, and optional `BlockRadioSettings { region, profile, frequency_hz }`. It intentionally cannot represent private identity/ratchets, `name`, `storage_dir`, `hub_api_key`, announce capabilities, RNode enabled/connection mode/peripheral id/display name, Bluetooth/USB identifiers, pairing data, filesystem paths, or hardware-local secrets. Imports preserve local-only fields and never overwrite them.
- Value validation is fail-closed: `tcp_clients` is 0-8 unique canonical `host:port` or `[IPv6]:port` endpoints with a 253-byte host and ports 1-65535, and rejects schemes, user-info, paths, queries, and fragments. `hub_api_base_url` is an absolute HTTP(S) URL with a host and optional path of at most 128 bytes, but no user-info, query, or fragment. Hub hashes use the existing exact-hex validator. Radio region is one of `US915/EU868/AU915/AS923/IN865/KR920/RU864`, profile is one of `REM-MF-URBAN-v1/REM-LF-RURAL-v1/REM-LM-EXTREME-v1`, and frequency must pass the existing native region/profile/`LoraConfig::validate_rnode` checks (including the current 137 MHz-3 GHz absolute bound). Malformed, out-of-range, and credential-smuggling vectors reject before review and again at commit.
- The signed content contains only issue/expiry time, issuer public identity, issuer app/LXMF destination hashes, the allowlisted network settings, trusted destination hashes, and preferred map layer (`base` or `satellite`). The private identity seed/key is structurally absent.
- `Node::create_block_onboarding_code` obtains the active persisted identity, canonicalizes typed content in Rust, and signs the canonical bytes. `inspect_block_onboarding_code` decodes, bounds-checks, binds public identity to advertised destinations, verifies signature/expiry, and returns a safe review DTO plus fingerprint. `import_block_onboarding_code` repeats those checks at commit time and writes supported settings plus issuer/trusted peers in one SQLite transaction; any failure rolls back all writes.
- TypeScript may render/scan the opaque encoded envelope and collect user choices only. It must not canonicalize, sign, verify, or independently write imported settings/peers. Legacy `rem.local-team` stays a clearly labeled import-only compatibility path.
- `BlockOnboardingImportRequest.peer_tiers` is a complete destination-to-tier map. The reviewed tier applies to the issuer; every other imported trusted destination is explicitly `outer` unless the review UI changes it. Missing/extra tier keys reject before the transaction.
- A checked-in non-secret fixed vector is consumed across Rust, node-client, and mobile tests. A single-byte mutation, identity/destination mismatch, oversize payload, expiry, excluded network field, incomplete tier map, and transaction failure all reject. The maximum representable 1,999-byte fixture below the 2,000-byte product ceiling must render with the existing `qrcode` library at error-correction level M and decode through the Android-compatible ZXing path.

### Native power policy

- `ReticulumNodeService` registers an Android `ACTION_BATTERY_CHANGED` receiver for the lifetime of the foreground service, derives percent/charging through `BatteryManager`, and calls a dedicated JNI `updateBatteryState(percent, charging)` path directly. Rust reads the persisted 10/20/30 threshold, enters saver when not charging and `percent <= threshold`, and exits when charging or `percent >= threshold + 3`. Pinia subscribes to the resulting native `PowerStateChanged` event; it never drives the policy.
- The runtime scheduler is watch-driven so transitions reset the next deadline. Saver cadence is `max(normal_announce_seconds, 300)` and `max(normal_telemetry_seconds, 300)`, so saver never increases traffic. Normal configured cadence resumes on exit.
- Every native outbound entry point assigns an `OutboundTrafficClass`: `sos`, `telemetry`, `community_status`, `chat`, `eam`, `event`, `checklist`, `plugin`, `raw`, or `control`. Saver allows SOS, direct Inner-only telemetry, one community-status transition, required delivery cancellation, and local-only operations. It denies chat, EAM, generic event, checklist replication, plugin sends, raw bytes, manual announce, peer-identity requests, propagation sync, and unknown/untyped sends.
- The class is persisted with each queued `MessageRecord`; retry reuses that stored class and never reclassifies from mutable content. Legacy queued messages without a class migrate to `chat`. All public `Node` send paths (`send_lxmf`, `retry_lxmf`, SOS/telemetry, event/EAM, checklist replication, plugin host, broadcast/raw, announce/sync/control commands) are inventoried in a table-driven admission test; `send_bytes` becomes private to typed wrappers or requires an explicit class.

Outbound API inventory and saver decision:

| File | Existing/new entry point | Assigned class | Saver behavior |
| --- | --- | --- | --- |
| `node/messaging.rs` | `send_lxmf` | chat | deny unless future typed SOS/location wrapper bypasses chat |
| `node/messaging.rs` | `retry_lxmf` | persisted original class | reapply stored decision; legacy missing class is chat/deny |
| `node/messaging.rs` | `cancel_lxmf` | control | allow cancellation only |
| `node/messaging.rs` | `announce_now`, `request_peer_identity`, `request_lxmf_sync` | control | deny manual network work |
| `node/messaging.rs` | `set_announce_capabilities` immediate announce | control | persist/coalesce the change and defer transmission to the next allowed cadence |
| `node/lifecycle.rs`, `runtime/background_announces.rs` | `start`, `restart` startup announce bursts | control | suppress 0/10/30-second bursts in saver and schedule one announce at the saver cadence |
| `node/lifecycle.rs` | `connect_peer` | control | deny; `disconnect_peer` remains allowed |
| `node/lifecycle.rs` | `send_bytes`, `broadcast_bytes` | raw | deny and require explicit typed class for internal callers |
| `node/sos.rs` | `trigger_sos`, `deactivate_sos`, SOS telemetry fanout | sos | allow |
| `node/events_plugins_telemetry.rs` | `record_local_telemetry_fix` replication | telemetry | allow only through Inner direct target policy and five-minute cadence |
| `node/community.rs` | new `publish_community_status` | community_status | allow initial saver transition once; coalesce later updates until normal mode |
| `node/events_plugins_telemetry.rs` | `upsert_event`, `delete_event` | event | allow local persistence, deny generic replication |
| `node/eam.rs` | `upsert_eam`, `delete_eam` | eam | allow local persistence, deny replication; `delete_local_eam` is local-only |
| `node/checklist_queries.rs`, `checklist_mutations.rs`, `checklist_task_edits.rs`, `checklist_task_status.rs` | `create_online_checklist`, `create_checklist_from_template`, `upload/update/delete/join_checklist`, row/cell/status mutations | checklist | allow local mutation, deny replication |
| `node/events_plugins_telemetry.rs` | `send_plugin_lxmf` | plugin | deny; plugin discovery/config/sensor persistence and `publish_plugin_event` stay local-only |
| `node/team.rs` | `refresh_hub_directory`, network effects of `set_active_team` | control/eam | deny refresh/replication while permitting local team selection |
| `runtime/background_announces.rs` and telemetry scheduler | periodic announce/telemetry | control/telemetry | permit only at the computed saver cadence |
| `runtime/propagation.rs` | autonomous propagation-node sync and delivery jobs | control/persisted message class | pause autonomous sync; queued delivery consults the stored class and only allowed SOS/telemetry/community work proceeds |

The admission test asserts that every public method above either reaches the typed policy with the listed class or is proven local-only. A compile-visible wrapper or exhaustive test registry fails when a new outbound path is added without classification.

## Architecture map and ownership

Shared contract files, changed once before feature work:

- Rust: existing `crates/reticulum_mobile/src/types/{core_contracts,messaging_contracts,mission_contracts,runtime_contracts}.rs`, new `types/community_contracts.rs`, `crates/reticulum_mobile/src/types.rs`, `crates/reticulum_mobile/src/reticulum_mobile.udl`, and `crates/reticulum_mobile/src/lib.rs`.
- JNI: existing `crates/reticulum_mobile/src/jni_bridge/{json_records,domain_inputs,core_inputs,conversions,parsing,state_api,delivery_api,messaging_api,sos_telemetry_api,mission_plugin_api,lifecycle_api,wire_events}.rs` and new `jni_bridge/community_power_onboarding_api.rs`.
- Node client contracts/converters: `packages/node-client/src/{contracts-domain,contracts-core,contracts-client,contracts,converters,message-converters,projection-converters,runtime-converters,index}.ts`.
- Mobile: `apps/mobile/src/types/domain.ts`, `apps/mobile/src/stores/{nodeSettingsModel,nodeProjectionController,nodeActionsController}.ts`.

Native behavior files:

- Persistence/community/QR: existing `crates/reticulum_mobile/src/app_state/{settings,peers,persistence,storage,mission,messaging}.rs`; new `app_state/{community,block_onboarding}.rs`; new `node/{community,block_onboarding,outbound_policy}.rs`; and their named `tests/community.rs`, `tests/block_onboarding.rs`, and `tests/outbound_policy.rs` modules.
- Runtime/privacy/power: existing `crates/reticulum_mobile/src/node/{lifecycle,messaging,replication_targets,eam,events_plugins_telemetry,sos,team,checklist_mutations,checklist_queries,checklist_task_edits,checklist_task_status}.rs`, `crates/reticulum_mobile/src/runtime/{background_announces,propagation,interface_config}.rs`, and new `runtime/power_policy.rs`. Task 2 extracts the radio region/profile/frequency validation currently assembled in Android-gated `interface_config.rs` into a platform-neutral native helper reused by both runtime configuration and Block Code inspection/commit; rules are not duplicated.
- Android: existing `apps/mobile/android/app/src/main/java/network/reticulum/emergency/{ReticulumBridge,ReticulumBridgeServiceApi,ReticulumNodeService,ReticulumNodePlugin,ReticulumNodePluginBase,ReticulumNodeTransportPluginApi,ReticulumNodeAppDataPluginApi,ServiceEventCoordinator}.java`; new `BatteryPowerCoordinator.java`; new `app/src/test/java/network/reticulum/emergency/{BatteryPowerCoordinatorTest,BlockOnboardingQrCompatibilityTest}.java`; and checked-in non-secret `app/src/test/resources/block-onboarding-max-v1.txt` plus its level-M PNG fixture. The QR compatibility test decodes the PNG through ZXing and asserts byte-for-byte equality with the maximum 1,999-byte text.
- Node-client implementations owned by Task 3: `packages/node-client/src/{capacitor-plugin,capacitor-client,capacitor-projection-client,web-client,mock-client,in-memory-projection-client}.ts` and `capacitor-client.test.ts`. Web/mock clients expose deterministic unavailable/read-only behavior and may consume the checked-in fixed vector, but never implement signing.

Mobile behavior/UI files:

- New `apps/mobile/src/utils/communityStatus.ts`; new `apps/mobile/src/components/{CommunityStatusPicker,CommunityPeerSummary,PowerSaverBadge,BlockOnboardingReview}.vue`; new `apps/mobile/src/components/settings/{SettingsCommunityPanel,SettingsBlockOnboardingPanel}.vue`; and matching `*.test.mjs` or Playwright specs.
- Existing `apps/mobile/src/utils/{mecp,localTeamExchange}.ts`, `apps/mobile/src/stores/{eventsStore,telemetryStore}.ts`, `apps/mobile/src/composables/{useSetupWizard,useTeamDirectory}.ts`, `apps/mobile/src/views/{SetupWizardView,SettingsView,ManageTeamsView,PeersDiscoveryView,DashboardView,InboxView}.vue` and their existing companion CSS files, `apps/mobile/src/components/{TeamQrExchange,PeersTeamRoster}.vue`, `apps/mobile/src/components/settings/{SettingsPeerManagementPanel,SettingsTeamsPanel,SettingsTelemetryPanel}.vue`, and `apps/mobile/src/services/telemetryLocationPlugin.ts`.
- New tests are named `apps/mobile/src/utils/communityStatus.test.mjs`, `apps/mobile/src/utils/blockOnboardingView.test.mjs`, and `e2e/community-p0.spec.ts`; existing `e2e/{settings,setup-wizard,dashboard,peers-connect,chat-failure-retry,telemetry}.spec.ts` is modified only where its existing flow is directly affected.
- `localTeamExchange.ts` remains only for legacy import; new Block Code utilities treat the signed envelope as opaque.

Files to avoid: generated Android build output, `target/`, `node_modules/`, copied UniFFI bindings, release artifacts, and LXMF protocol reimplementations outside the compiled Rust library. If the UDL changes, regenerate through `tools/codegen` and audit generated output intentionally.

Read path: native records/events -> node-client converters -> Pinia projection -> views. Android service battery signal -> JNI -> Rust power state -> native event -> Pinia.

Write path: UI actions -> typed native APIs. Community publish, Block Code create/inspect/import, circle changes, and battery policy never use a parallel TypeScript authority.

## Task 1: Establish all shared contracts and migrations

Allowed scope: the shared contract files listed above, `AppStateStore` migrations/defaults, converter fixtures, UDL/codegen, and focused tests. No feature UI.

Expected output:

- Household name/id/composition/status/role badges and preferred map layer.
- `inner`/`outer` saved-peer tier with legacy-Inner/new-Outer semantics.
- Power preferences/state, `OutboundTrafficClass`, persisted message class, and Block Code typed boundary records.
- Existing/legacy/malformed/boundary fixtures round-trip across Rust, JNI JSON, node-client, and mobile types. A compile-time/serialization fixture proves every excluded `NodeConfig` and hardware-local field is absent from `BlockNetworkSettings`.

Verification: `npm run test:node-client`; `npm run test:app-unit`; `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml app_state`; supported UniFFI generation plus generated-artifact audit when UDL changes.

Parallel: no; this is the single owner for shared boundary shape.

## Task 2: Implement Rust-authoritative community, privacy, QR, and power behavior

Allowed scope: native behavior files listed above and focused Rust tests. No Android lifecycle or feature UI.

Expected output:

- Typed publish/parse/project/replay for the exact B04/REMCS1 contract, including merge, malformed, future, and stale behavior.
- Inner-only direct telemetry; Hub fail-closed; native Inner chat/send/retry guards.
- Rust canonical Block Code create/inspect/revalidate-and-transactionally-import APIs using the persisted Reticulum identity.
- Table-driven outbound classification/admission, class persistence/retry reuse, and watch-driven announce/telemetry cadence.

Verification: focused Rust contract, signature vector, SQLite rollback, projection, target-set, admission-matrix, retry, and scheduler tests; then `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml`.

Acceptance evidence: fixed valid/tampered QR vectors, an Inner/Outer/Hub target fixture, persisted/replayed community record, and admission results for every public outbound API.

Parallel: no; these policies share `Node` and persistence authority.

## Task 3: Connect Android service battery observation and native plugin APIs

Allowed scope: Android and JNI files listed above, the named node-client implementation/fallback files, and Android/node-client unit or instrumentation tests; no Vue feature UI.

Expected output:

- Foreground-service receiver continues observing battery with the WebView paused or absent and directly calls JNI/Rust.
- Native power events reach the Capacitor plugin; Block Code create/inspect/import bridge methods call the Rust owners without Java/TypeScript crypto.
- Capacitor clients expose the typed native methods/events; web/mock/in-memory clients fail clearly for create/scan/import or expose only the fixed inspected fixture without a signing authority.
- Service teardown unregisters the receiver and no lifecycle leak remains.

Verification: Android JVM tests for percent/charging mapping and receiver lifecycle; `BlockOnboardingQrCompatibilityTest` for the maximum 1,999-byte level-M fixture below the 2,000-byte product ceiling; instrumentation or debug evidence for service -> JNI -> Rust -> event; `./gradlew testDebugUnitTest assembleDebug`.

Acceptance evidence: a simulated Android battery transition through the real coordinator and a native Block Code bridge round trip. Real camera/hardware behavior may be labeled implemented but unproven only after build/test evidence passes.

Parallel: no; follows Task 2's native APIs.

## Task 4: Integrate all four P0 user experiences sequentially

Allowed scope: mobile utilities/stores/components/views and focused unit/Playwright tests. Shared store/view files have one sequential owner.

Expected output:

- Setup/settings edit household name, adults, children, pets, up to five role badges, map layer, power toggle, and 10/20/30 threshold.
- Dashboard offers exactly `All Home`, `1 Missing`, `Evacuated`, and `Needs Help`; peers/dashboard show household composition, status freshness, tier, and saver badge.
- Per-peer tier is editable; Outer/unclassified peers are unavailable for exact location and outgoing/retried chat, with native errors reflected clearly.
- Settings creates only signed Block Codes. Scan/review shows signer fingerprint and imported fields, collects name/household/roles/issuer tier, assigns every additional trusted destination `outer` by default (with an explicit per-peer review override), sends the complete tier map to the single native transaction, and rejects missing/extra keys. Legacy QR is import-only and labeled.
- Power mode visibly disables ordinary send/retry controls while preserving SOS/location affordances allowed by native policy.

Verification: `npm run test:app-unit`; `npm run test:node-client`; `npm --workspace apps/mobile run typecheck`; focused Playwright household/dashboard/peers/inbox/Block-Code/power cases using the Rust fixed vector through a test adapter, never a TypeScript signer.

Acceptance evidence: browser screenshots/flows for every acceptance item and explicit web-unavailable messaging for hardware-only QR signing/scanning where applicable.

Parallel: no; the same Pinia and route surfaces compose all features.

## Task 5: Cross-layer integration, documentation, and final proof

Allowed scope: integration fixes, generated bindings through supported codegen, `docs/architecture.md`, user-facing docs, and tests.

Expected output: all P0 flows compose without duplicate truth paths; source-size limits remain satisfied; community replay handles late join; profile and saver transitions publish exactly once; generated artifacts are updated only through supported codegen.

Verification:

- `npm run check:source-size`
- `npm run test:unit`
- `npm --workspace apps/mobile run typecheck`
- `npm run node-client:build`
- `npm run web:build`
- `npm run mobile:build`
- `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml`
- `npm run test:e2e` or documented focused specs if the full environment is unavailable
- From `apps/mobile`: `npx cap sync android`
- From `apps/mobile/android`: `./gradlew testDebugUnitTest assembleDebug`
- `git diff --check`, generated/build-artifact audit, and final clean-scope review

Acceptance evidence: results map directly to issues #235-#238. Physical camera, live battery, and multi-device mesh claims are reported separately as proven or implemented but unproven.

Parallel: no; this is the integration gate.
