# Codex task: circles into teams, phase 1 (runtime rule, migration, derived tier)

Design: `docs/plans/circles-into-teams-design.md` (read it first, including the Decisions section).
Repository rules: `AGENTS.md` (read it first). Rust rules in AGENTS.md apply: typed errors with `?`, no `unwrap` in library code, `// SAFETY:` on any `unsafe`.

## Role

You are changing the Rust runtime in `crates/reticulum_mobile` so that trust is team membership, chat is never gated, and telemetry follows the team roster. This phase touches the native runtime and its tests only. The Vue app, the node-client package and the UniFFI contract keep compiling unchanged because the circle tier becomes a value derived by the runtime.

## Scope of phase 1

In scope:

1. Chat admission: remove the Inner-tier and active-team gates.
2. Team flows (events, missions, checklists, EAM, telemetry): remove the Outer filter and the exact-telemetry Inner gate; recipients are the active team roster.
3. Household status (B04) and SOS: unchanged behaviour, with the tier override removed.
4. Link maintenance: maintain links for local team members (and keep today's behaviour for peers with an open conversation).
5. Migration: Inner peers outside any local team join local Yellow once; Outer peers stay out.
6. Derived tier: `circle_tier` on every saved peer read by callers is `inner` when the peer is in at least one local team, else `outer`; writes to the tier are ignored with a deprecation log line.
7. Tests for the policy table below.

Out of scope (later phases): Vue screens, node-client contracts, Block Code `peer_tiers` -> `peer_teams`, UniFFI removal of `CircleTier`, user manual.

## Policy table (the acceptance contract)

| Flow | Required behaviour after phase 1 |
| --- | --- |
| Household status (B04) | sent to every saved peer, regardless of team |
| SOS | unchanged: every saved peer and configured relays |
| Events, missions, checklists, EAM | active team roster (local members + hub roster per hub mode); a saved peer in no team receives nothing |
| Telemetry | active team roster, same routing rules as events; no recipient-side position policy; Connected mode still does not send exact positions through the hub |
| Chat (`send_lxmf`, retry) | any destination: saved or discovered, in a team or not, hub-roster member or not; in Connected mode a hub-roster-only destination is routed through the hub as team traffic is today; power saver still pauses chat |
| Maintained links | members of any local team; a saved peer in no team gets no maintained link unless it has an open conversation |
| Derived `circle_tier` | `inner` iff the peer is in at least one local team |

## Where the current behaviour lives

- `crates/reticulum_mobile/src/node/outbound_policy.rs`: `peer_is_inner_saved`, `peer_is_outer_saved`, `inner_saved_peer_authorizes_destination`, `exact_telemetry_target_is_allowed`.
- `crates/reticulum_mobile/src/node/messaging.rs` lines ~56-65 and ~125-135: chat admission on send and on retry.
- `crates/reticulum_mobile/src/node/lifecycle.rs` around line 339: `send_bytes_with_class` rejects Outer peers for every class except SOS and CommunityStatus.
- `crates/reticulum_mobile/src/node/replication_targets.rs` `routed_destination_hex`: rejects destinations outside the active local team when local teams exist. Chat must bypass this; team flows keep it.
- `crates/reticulum_mobile/src/node/replication_routing.rs` line ~352 and `replication_planning.rs` line ~155: Outer peers filtered from targets, saved-peer target set limited to Inner.
- `crates/reticulum_mobile/src/node/events_plugins_telemetry.rs` line ~61: B04 overrides every tier to Inner. Replace with "all saved peers" directly.
- `crates/reticulum_mobile/src/node/messaging_policy.rs` line ~20, `sos_fanout.rs` line ~171, `events_plugins_telemetry.rs` line ~433: telemetry targets filtered by `exact_telemetry_target_is_allowed`.
- `crates/reticulum_mobile/src/app_state/settings.rs` line ~14: existing one-time local-team initialisation (`local_teams_initialized`). Add the phase-1 migration next to it.
- `crates/reticulum_mobile/src/jni_bridge/parsing.rs` `to_explicit_saved_peer_record`: currently rejects a saved peer without a tier. Accept a missing tier; ignore a provided one.
- `crates/reticulum_mobile/src/runtime/projections.rs` line ~76 and `app_state/block_onboarding.rs` line ~101: tier defaults on newly created saved peers. Keep the field populated (derived) so serialisation is unchanged.
- `crates/reticulum_mobile/src/types/mission_contracts.rs` `SavedPeerRecord.circle_tier`: keep the field this phase.

## Implementation steps

1. **Team membership helper.** In `node/local_team_routing.rs` (or a new `node/team_membership.rs` under the 500-line gate) add `fn saved_peer_is_local_team_member(local_teams: &[LocalTeamRecord], peer: &SavedPeerRecord) -> bool` that matches `destination_hex`, `lxmf_destination_hex` and `identity_hex` against `member_destinations` the same way `peer_is_inner_saved` matches today (normalise with `normalize_hex_32`). Local teams come from `AppSettingsRecord.teams.local_teams`.
2. **Derived tier.** Add `fn apply_derived_circle_tiers(peers: &mut [SavedPeerRecord], local_teams: &[LocalTeamRecord])` and call it in `Node::get_saved_peers` and in every other path that returns saved peers to the bridge (grep `get_saved_peers` in `jni_bridge/`). The app changes a tier by rewriting the whole list through `setSavedPeers` (`apps/mobile/src/stores/nodeActionsController.ts` `setPeerTier`), so in `Node::set_saved_peers` keep each stored peer's tier, ignore the incoming one when it differs, and log one `log::warn!` per call with "circle_tier is derived from team membership and was ignored". The call must still succeed.
3. **Migration.** In `app_state/settings.rs` next to `local_teams_initialized`, add `circle_migration_completed: bool` to `TeamSettingsRecord` (serde default false). On the first `get_app_settings` after upgrade: for every saved peer with stored `circle_tier == inner` not present in any local team, append its `destination_hex` to the local Yellow team (create Yellow if absent); leave Outer peers untouched; set the flag; write settings and peers in one transaction. Add a test that runs the migration twice and proves it is idempotent.
4. **Chat.** In `messaging.rs` delete both `inner_saved_peer_authorizes_destination` checks. In `lifecycle.rs` `send_bytes_with_class` delete the Outer rejection. In `routed_destination_hex` add a `traffic_class` parameter (or a sibling function) so `OutboundTrafficClass::Chat` skips the local-team membership rejection while still mapping a hub-roster-only destination to the configured hub in Connected mode.
5. **Team flows.** Remove the `CircleTier::Inner` filters in `replication_planning.rs` and `replication_routing.rs` and the `retain(!peer_is_outer_saved)` at the end of target building. Confirm with a test that a saved peer in no local team receives no event replication while a saved peer in the active local team does.
6. **Telemetry.** Delete `exact_telemetry_target_is_allowed` and its three call sites; telemetry targets are whatever `build_telemetry_replication_targets` returns. Keep the Connected-mode hub rule (`connected_hub_replication_target` only when the hub roster allows telemetry, exact positions never through the hub) exactly as it is.
7. **B04.** In `events_plugins_telemetry.rs` replace the tier override loop with target building over all saved peers.
8. **Links.** In `lifecycle.rs` link maintenance, replace the Outer exclusion with "not a local team member and no open conversation".
9. **Delete** `peer_is_inner_saved`, `peer_is_outer_saved`, `inner_saved_peer_authorizes_destination`, `exact_telemetry_target_is_allowed` once no caller remains.
10. **Docs.** Update `docs/architecture.md` sections "Inner and Outer circles" and "Local And RCH Multi-Team Routing" to the phase-1 behaviour (keep the section heading until phase 3; add one sentence that the tier is now derived).

## Tests to add or update

Existing tests that assert tier behaviour and must be rewritten to the policy table: `node/tests/outbound_policy.rs`, `node/tests/propagation.rs`, `node/tests/block_onboarding.rs`, `node/tests/eam_upsert_eam_replicates_to_native_peer_proje.rs`, `node/tests/events_delete_event_replicates_to_native_peer_pro.rs`, `node/tests/events_event_route_priority_puts_low_hop_live_pee.rs`, `node/tests/events_event_replication_payload_uses_compact_mec.rs`, `node/tests/checklist_runtime.rs`, `node/tests/checklist_contracts_checklist_create_online_args_match_support.rs`, `node/tests/telemetry_telemetry_replication_payload_stays_under_.rs`, `app_state/tests/peers.rs`, `runtime/tests/peers_routes_direct_delivery_health_blocks_and_restores.rs`, and the JNI contract tests in `jni_bridge.rs` (`assert_eq!(peer.circle_tier, CircleTier::Inner {})` at ~line 113).

New tests (one per policy row, names are suggestions):

- `chat_send_is_admitted_to_unsaved_discovered_peer`
- `chat_send_is_admitted_to_saved_peer_outside_every_team`
- `chat_send_to_hub_roster_member_routes_through_hub_in_connected_mode`
- `power_saver_still_pauses_chat`
- `event_replication_skips_saved_peer_in_no_local_team`
- `telemetry_targets_follow_active_team_without_tier_gate`
- `telemetry_never_sends_exact_position_through_hub` (keeps today's assertion)
- `community_status_reaches_every_saved_peer`
- `derived_circle_tier_reflects_local_team_membership`
- `set_peer_tier_is_ignored_and_logged`
- `inner_peers_outside_teams_migrate_to_yellow_once`

Test helpers live in `crates/reticulum_mobile/src/node/tests/support.rs` and `messaging_delivery.rs` (`prepare_storage_dir`, `sample_app_settings`, `sample_saved_peer`). Tests that call `Node::new()` use a cwd-relative storage dir and must take `test_lock()`; prefer `Node::with_storage_dir(Some(dir))` with `prepare_storage_dir` for new tests.

## Verification (all must pass)

```bash
cargo test --manifest-path crates/reticulum_mobile/Cargo.toml
```

```bash
npm --workspace apps/mobile run typecheck
```

```bash
npm run check:source-size
```

```bash
npm run test:unit
```

```bash
npx playwright test e2e/peers-connect.spec.ts e2e/chat-failure-retry.spec.ts e2e/community-p0.spec.ts --project=chromium
```

Do not raise the 500-line source gate; split files instead. No UniFFI contract change in this phase, so no binding regeneration. If a file you touch is generated, stop and report it.

## Delivery

- Branch from `main` (after PR #292 "household profile never blocks Settings saves" is merged; if it is not, branch from `Corvo/unruffled-albattani-e8d909`).
- One PR titled `feat(teams): derive trust from team membership (circles phase 1)`.
- PR description: what changed, which files, the policy table with each row's test name, the verification commands run, and any deviation from this brief.
- Commit messages in conventional-commit form.

## Definition of done

A device that saves a peer without adding it to a team can chat with that peer, receives its household status, and sends it nothing else. Adding the peer to the active team makes it receive events, checklists, EAM and telemetry. Every saved peer projected to the app reports a `circle_tier` that equals its team membership, and an upgraded device with legacy Inner peers finds them in local Yellow.
