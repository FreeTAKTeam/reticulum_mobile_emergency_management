# Design draft: fold Inner/Outer circles into team membership

Status: draft for review, 2026-10-16. Nothing in this document is implemented.

## Goal

Reduce the peer model to two operator-facing concepts:

- **Household**: the device's own public card (name, counts, badges, status). Unchanged by this design.
- **Team**: the people a device works with. Membership decides *who receives my team traffic*.

The Inner/Outer circle tier disappears as a separate concept. There is no separate trust action: trusting a peer **is** adding it to a team. Chat is never gated. Whether a device sends its location is that device's own telemetry setting, so no recipient-side location policy exists.

## What circles do today

Every saved peer carries `circle_tier: inner | outer`. The runtime reads it in six places:

| Consumer | Behaviour today |
| --- | --- |
| `node/outbound_policy.rs` `peer_is_inner_saved` | Chat send/retry and exact-location telemetry require a saved **Inner** peer. |
| `node/replication_routing.rs:352` and `replication_planning.rs:155` | Events, missions, checklists and EAM fanout drop every saved **Outer** peer, even when it is in the active team. |
| `node/lifecycle.rs:339` | Outer peers are excluded from maintained links. |
| `node/events_plugins_telemetry.rs:61` | Household status (B04) overrides every tier to Inner, so all saved peers receive it. |
| SOS fanout | Ignores the tier. |
| `app_state/block_onboarding.rs` | Block Code import requires a tier for every imported peer; new peers default to Outer. |

Net effect: an **Outer** peer receives household status and SOS and nothing else. It is a saved contact that takes part in no working flow. An **Inner** peer is a saved contact that receives everything the active team receives plus the two private flows (chat, exact GPS).

Teams already scope every outbound set (`replication_targets.rs`, `routed_destination_hex`), and existing saved peers were migrated into local Yellow when teams arrived. So today "Inner" and "member of a local team" overlap almost completely, and the one place they differ (an Outer peer inside a team) is a dead state.

## Proposed model

### Rule 1: trust = "Add to team"

The only trust action is adding a saved peer to a local team. Hub-owned (RCH) memberships are what the hub says they are and are not edited in REM, exactly as today.

| Flow | Today | Proposed |
| --- | --- | --- |
| Household status (B04) | all saved peers | all saved peers (unchanged) |
| SOS | tier ignored | unchanged |
| Events, missions, checklists, EAM | active team roster minus Outer | active team roster (local + hub) |
| Telemetry | saved Inner peer, direct route, not Connected | active team roster, same routing rules as events |
| Chat | saved Inner peer in the active team | **any reachable peer**, saved or discovered, in or out of a team |
| Maintained links | saved Inner peers | members of any local team, plus any peer with an open chat conversation |

A saved peer in no local team is a **contact**: visible, chattable, receives household status and SOS. It does not receive team traffic (events, missions, checklists, EAM, telemetry). This is the Outer behaviour plus chat, under a name that says what it is.

### Rule 2: chat is always available

Chat admission loses both of today's gates: the Inner-tier check in `outbound_policy.rs` and the active-team check in `routed_destination_hex`. Delivery still follows the peer's own route (direct link, then propagation). A peer that REM knows only from the RCH roster is chat-eligible too: in Connected hub mode it is reached through the hub, as team traffic is today; a peer with its own announce or link is reached directly. Power saver keeps pausing ordinary chat; that is a battery rule, not a trust rule.

### Rule 3: telemetry has no recipient-side policy

Whether a device sends its position is decided by that device's own Settings telemetry toggle (and the SOS flow). A device that has telemetry off is simply not sending, which says nothing about what it may receive. So the "exact telemetry only to Inner peers" gate (`exact_telemetry_target_is_allowed`) is removed, and telemetry is routed like every other team flow: to the active team roster. The Connected-mode rule that exact positions are not sent through the hub until RCH provides a recipient policy stays, because it is a hub contract, not a circle rule.

### Rule 4: Block Codes assign teams, not tiers

The signed envelope already excludes tiers. The import request changes from `peer_tiers` (destination -> inner/outer) to `peer_teams` (destination -> list of local team UIDs, possibly empty = contact). The review screen replaces "Circle access" with a team picker; the issuer defaults to the active team, other trusted destinations default to contact.

## Migration

1. One-shot, in Rust, on first open after upgrade: every saved peer with `circle_tier = inner` that is in no local team is added to local Yellow. Outer peers stay out of all teams and become contacts.
2. `circle_tier` stays in storage for one release but becomes **derived** on read: `inner` if the peer is in any local team, else `outer`. Writes that set a tier are ignored (bridge logs a deprecation warning). This keeps the node-client contract and the UI compiling while screens are converted.
3. Next release: remove `circle_tier` from `SavedPeerRecord`, the JNI inputs (`to_explicit_saved_peer_record` currently rejects a missing tier), the node-client contracts and `types/domain.ts`.

## Runtime changes by file

- `node/outbound_policy.rs`: delete `peer_is_inner_saved`, `peer_is_outer_saved`, `inner_saved_peer_authorizes_destination` and `exact_telemetry_target_is_allowed`; the Outer rejection in `send_bytes_with_class` (`node/lifecycle.rs`) goes with them.
- `node/replication_targets.rs` `routed_destination_hex`: chat (`OutboundTrafficClass::Chat`) skips the active-team membership rejection; team flows keep it.
- `node/replication_routing.rs`, `replication_planning.rs`: drop the Outer filter; the active-team roster already excludes contacts.
- `node/messaging_policy.rs`, `sos_fanout.rs`, `events_plugins_telemetry.rs`: telemetry targets are the active team roster without the Inner filter.
- `node/lifecycle.rs`: maintain links for local team members and open conversations.
- `node/events_plugins_telemetry.rs`: remove the tier override for B04; target all saved peers directly.
- `app_state/block_onboarding.rs`, `jni_bridge/community_power_onboarding_api.rs`: `peer_tiers` -> `peer_teams`.
- `app_state` migration + derived tier projection; UDL and generated bindings regenerated.
- `reticulum_mobile.udl`: `CircleTier` kept until the last phase.

## UI changes

- Peers page: the Inner/Outer select on each saved peer becomes team chips plus "Add to team". A peer in no team shows "Contact".
- `CommunityPeerSummary.vue`: tier badge becomes team colour dots.
- `circleChatPolicy.ts`: `innerCirclePeerOptions` becomes "all reachable peers"; `chatPolicyReason` keeps only the power-saver message.
- Chat: the "requires a saved Inner Circle peer" barrier disappears; a discovered peer can be messaged without saving it first.
- Manage Teams: unchanged apart from copy.
- Block Code review: team picker instead of "Circle access".
- Setup wizard and Settings: no change (household only).
- User manual: replace the Peers "Inner/Outer" paragraphs and the glossary; one sentence under Teams about the location switch.

## What is lost, and the mitigation

| Lost | Mitigation |
| --- | --- |
| Per-peer refusal to receive someone's chat | None by design: chat is always available. Blocking a sender is a separate feature if ever wanted. |
| Per-peer GPS opt-out | Not a recipient decision: a device that does not want to share location turns its own telemetry off. Team membership only decides who is on the roster. |
| "Trusted but not in a team" | Does not exist today in a useful form: an Inner peer outside the active team receives nothing but B04 and SOS. |
| Explicit tier choice during Block import | Replaced by an explicit team choice, which is the decision operators actually make. |

## Decisions (2026-10-16)

1. **Location sharing is always on for the roster.** There is no per-team or per-peer location switch; the sender's own telemetry toggle is the only control.
2. **RCH members are chat-eligible.** A peer known only from the hub roster can be messaged; in Connected mode the message is routed through the hub as team traffic is today, otherwise directly.
3. **`CircleTier` stays in the UniFFI and node-client contracts for one release as a derived value** (assumption, not yet confirmed): it avoids a coordinated breaking change while the screens are converted, and phase 3 removes it.

## Phasing

1. Runtime rule + migration + derived tier, with tests for the policy table above (chat ungated, telemetry on the team roster, contacts receive B04 and SOS only). No UI change yet; existing screens keep working through the derived tier.
2. Peers page, peer summary, chat policy text, Block Code review, manual.
3. Remove `circle_tier` from contracts and bindings.

Each phase ships on its own PR and keeps `cargo test`, typecheck, size gate, unit and the peers/community/settings Playwright specs green.
