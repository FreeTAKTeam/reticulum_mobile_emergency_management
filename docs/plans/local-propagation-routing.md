# Daemon-free local propagation and LoRa/TCP routing

Status: subject to the authoritative [offline-only correction](offline-only-correction.md); approved for implementation; Phase 0 waived by the user. The receiver-confirmed `/get` correction was explicitly approved on 2026-10-24; see [the amendment](local-propagation-relay-correction.md).
Supersedes the earlier unapproved routing draft. Keep that draft for reference and do not merge its incidental cleanup.

## Goal

When a recipient can't be reached and no external propagation node is reachable either, REM keeps **its own outgoing messages** in a durable, bounded local store inside LXMF-rs. It hands them on as soon as delivery becomes possible: either directly to the recipient or through a relay. Whether node A can use a remote relay depends on Reticulum routes, not on whether A has TCP itself. This brings in no daemon, `RpcDaemon`, RPC server, ZeroMQ backend or separate REM queue.

## Grounding (checked at LXMF-rs `7bf5257c`, 2026-10-23)

These facts shape the plan. Re-check them if the base revision moves.

- **Where the propagation code lives.** Propagation storage and its handlers sit in `crates/libs/rns-rpc`: the daemon dispatch under `rpc/daemon/dispatch_legacy_propagation_parts` and the rusqlite storage under `storage/messages_parts`. The inbound `/offer` and peering handling is in `crates/apps/reticulumd/.../inbound_control_propagation_parts`. `lxmf-runtime` has a `propagation_relay` setting but no store. So the extraction source is mainly `rns-rpc`, not `reticulumd`.
- **`/offer` needs a peering key.** `reticulumd` validates one (`validate_peering_key`), and Python `lxmd` does the same. A phone that is not a peer cannot normally run an `/offer` possession check against a stock relay.
- **LoRa polling is off on purpose.** REM's automatic relay polling skips RNode-only links (`spawn_propagation_maintenance_task` → `has_active_relay_transport_interface`). This plan's LoRa polling deliberately reverses that rule, and needs an airtime budget.
- **Size headroom.** `delivery_policy.rs` (439 lines) and `sdk_bridge/propagation_fetch.rs` (401 lines) are close to the 500-line gate. New behaviour goes into new modules.

## Scope

- **Eligible:** messages this phone prepares that already go through LXMF delivery: chat, mission events, telemetry (latest-fix only).
- **Not eligible:**
  - messages with no validated recipient encryption identity (these get a typed `UnresolvedRecipient` error)
  - group fan-out copies beyond each recipient's own transient
  - anything from a third party
- **Before routing:** apply recipient permissions and sharing restrictions before choosing a route. Check them again just before any later transmission (direct, upload or retrieval).

## Routing rules

| Situation | Behaviour |
|---|---|
| Recipient has a usable route | Normal direct delivery. Having no open link does not by itself mean the peer is offline. |
| Recipient unreachable; an external relay is reachable over any interface | Upload to that relay. |
| A has LoRa only; B has LoRa and TCP, with transport enabled | A reaches the relay over the path A → LoRa → B → TCP → relay. B hosts nothing and stores nothing. |
| B loses TCP, disables transport, or can't reach the relay | Once the bounded attempts below fail, A commits the message to its local store. |
| Neither the recipient nor any relay is reachable, including when there are no interfaces | Commit the message to the local store and report **Stored locally**. |
| **A locally stored message's recipient becomes reachable again** (announce, path response or inbound link) | **A pushes the message directly**. Delivery proof retires the local copy. This is the main recovery path. |
| A stored message's relay becomes reachable | Upload it (see Recovery). |
| Recipient comes back but A doesn't notice (asymmetric LoRa reachability) | Phase 3 only: the recipient pulls from A's retrieval service. |

**"Bounded attempts"** reuses `direct_attempt_budget_for_send` and the existing path-request timeout. The plan must give one figure per interface class: how long before "Stored locally" shows, with a target of 60 s or less on TCP and 180 s or less on LoRa. It must not add another retry loop.

A cached propagation announce is a hint, not proof that the relay is reachable. Availability is decided by path state, link results and bounded recovery.

**One owner per message.** Once a message is committed to the local store, REM's outbound retry and ack tracking stop owning it. The store's recovery loop drives it from then on, so the same message is never in two retry loops. Storing a message locally does not start the recipient acknowledgement timeout.

## Phases

Each phase ends at a gate. A later phase doesn't start until the earlier gate has passed.

### Phase 0: spikes (no product code)

1. **Verifying the handoff without peering.** Find out whether the phone can confirm that a relay has the message without being its peer. Options:
   - an `/offer` with a computed peering key, including what that costs on mobile
   - a relay-specific query
   - treating the relay's resource-transfer success as "uploaded, unverified"

   **Gate:** a written decision. If nothing works against stock `lxmd`, retire on **delivery proof or expiry only**. Uploaded copies then stay "Uploaded (unverified)" until one of those happens.
2. **Retrieval destination aspect.** Choose an aspect REM owns, such as `rem.fetch`, instead of `lxmf.propagation`. Otherwise path responses show up as propagation nodes in Sideband or `lxmd` peer lists, and those nodes try to deposit or peer with the phone. Confirm the aspect against the delivery-capability extension, `rem.local_propagation_fetch.v1`.
3. **LoRa airtime budget.** Measure what one retrieval poll and one upload cost on the field RNode settings. **Gate:** the polling interval and the transfer-size cap come from measurements, not from the five-minute guess.

### Phase 1: LXMF-rs `lxmf-propagation` crate

- **Extract, don't duplicate.** Move the following from `rns-rpc` and `reticulumd` into `crates/libs/lxmf-propagation`:
  - the store schema, single-writer database access, validation, deduplication and retention
  - the standard propagation handlers

  Then make `rns-rpc`/`reticulumd` delegate to it. Keep the schema and bounded inventory processing unchanged. Preserve custody semantics except the approved receiver-confirmed `/get` correction: response construction and transfer-size exclusion must not complete receiver custody. Add a feature split so the embedded build pulls in neither the RPC dispatch nor ZeroMQ.
- **Embedded API.** The service is built from the existing transport, identity, storage and lifecycle handles. It provides:
  - `deposit_own(prepared_message)`: there is deliberately no raw-ciphertext-plus-claimed-owner entry point
  - `status()`, `recover()`
  - `retrieve_for(authenticated_identity)` (used in Phase 3)
  - `stop()`, which never deletes data still pending

  Errors are typed: `UnresolvedRecipient`, `PayloadTooLarge`, `StoreFull`, `Expired`.
- **Contract tests shared by the daemon and embedded modes:**
  - storage, deduplication, expiry and eviction
  - authentication
  - restart in the middle of a deposit, an upload, a verification and a retirement
  - "transport completes, then storage rejects": the local copy must be kept
- **Dependency proof.** Check `cargo tree -e features` for the embedded feature set in CI and fail the build if `rns-rpc` dispatch or `zmq` appears.
- **Gate:** the `reticulumd` tests pass through the shared crate, with only the approved `/get` custody expectations corrected, and Python interop passes for relay upload and retrieval.

### Phase 2: REM integration (durable outbox and relay recovery)

- **Bridge.** Wire the service in through `sdk_bridge`, as a new module. Add configuration and a storage-status projection to the UDL, then regenerate the bindings with `tools/codegen`.
- **Recovery triggers.** Add these as events in the existing runtime, with no new polling loop:
  - recipient announce or path learned → direct push
  - relay became reachable → upload
- **Per-message states.** Delivery projections and the UI distinguish these states:
  - `StoredLocally`
  - `Uploading`
  - `UploadedUnverified`
  - `RelayHolds` (verified, if Phase 0 allows)
  - `Delivered`
  - `Acknowledged`
  - `Expired`
  - `Evicted`

  `Expired` and `Evicted` are shown on the message and are never removed silently.
- **Docs and tests.** Update `docs/architecture.md`. Add Playwright coverage for the "Stored locally" → "Delivered" transition and for the eviction notice, using the web mock.
- **Gate:** Rust, bridge, typecheck, source-size and the e2e checks pass. In the field, the two-phone matrix passes rows 1–6 of the routing table.

### Phase 3 (optional): pull retrieval from the originating phone

Do this only if Phase 2 field data shows real asymmetric cases, where the recipient can reach A but A never learns the recipient's path.

- **The service is retrieval-only.** It rejects every deposit, offer and peering request. An authenticated recipient can fetch only messages addressed to their own identity.
- **Discovery.**
  - The service is advertised only through the `rem.local_propagation_fetch.v1` capability.
  - The service destination is derived from the verified contact identity.
  - It is never promoted into the relay candidate list, including when it shows up through path responses.
- **Interop.** This path is REM-to-REM only. Python and Sideband clients won't discover it. Their recipients still get the message through direct push or a relay.

## Storage policy

- **Capacity:** 26,214,400 bytes (25 MiB) of encrypted payload. Database, journal and status overhead each have their own bound and a test.
- **Retention:** 7 days from first local admission. Retries, duplicates and uploads don't extend it. Shorter telemetry expiry still applies.
- **Overflow order:**
  1. expired entries
  2. superseded telemetry
  3. the oldest telemetry
  4. the oldest other entries

  This order keeps a telemetry flood from evicting chat or mission events. If one recipient holds 50% or more of the store, that recipient's entries are evicted first. Evictions and expiries are recorded per message. A single payload that can't fit in an empty store is rejected with `PayloadTooLarge`.
- **Telemetry:** keeps latest-fix coalescing. Data already uploaded can't be pulled back by a local expiry, and the UI says so.
- **Atomic retirement:** confirmation, payload deletion, quota accounting and the status change are committed in one transaction. A bounded handoff record is kept for diagnostics.

## Recovery and scheduling

- **Uploads.** An upload keeps the transient ID unchanged. Upload and verification progress is saved to disk, so a restart resumes the work instead of starting it again. That rules out upload loops.
- **One transfer at a time.** Only one propagation transfer runs at once across upload, retrieval and verification. It uses the existing bounded backoff, and power saver suspends everything except event-triggered direct pushes.
- **TCP polling:** keeps the existing 30 s `AUTO_PROPAGATION_SYNC_INTERVAL`.
- **LoRa polling** (Phase 2 onward, replacing the current RNode-only skip):
  - runs on discovery or reconnection events, plus a periodic poll at the interval Phase 0.3 sets (proposed: 5 min) while a usable route exists
  - is capped per transfer at the size Phase 0.3 sets

## Verification matrix (before any release)

| # | Scenario | Must show |
|---|---|---|
| 1 | Direct route available | Normal delivery and no local store write. |
| 2 | Recipient down; relay reachable over TCP | The message is uploaded and then retrieved. |
| 3 | A on LoRa only, through B over TCP to the relay | The upload succeeds, and B's database holds no propagation rows. |
| 4 | B loses TCP or turns off forwarding during row 3 | A stores the message locally within the bound, then recovers once B is restored. |
| 5 | No interfaces | Shows "Stored locally". Once the recipient is back, the message is pushed directly and the local copy retired on proof. |
| 6 | Reboot during each persisted state | No loss, no duplicate upload, no upload loop. |
| 7 | Third party deposits to A, or A to another REM phone | Rejected. Neither phone becomes a relay candidate. |
| 8 | Python `lxmd` relay | Upload interop works, and the retirement behaviour matches the Phase 0.1 decision. |
| 9 | Store fill, expiry and eviction | Eviction follows the priority order and is visible to the user. Single-payload rejection works. |

Remote possession is something the relay asserts after authentication. It doesn't guarantee that the relay will keep the message or survive a crash.

The user's 2026-10-24 scope correction supersedes the original `7bf5257c` implementation base. Preserve the tested LXMF-rs `7822b608f2cd2debd8170895bc580d01dda5356e` behavior and add only offline custody. Pin the qualified additive revision. Normal online LoRa polling, Sync and send deadlines remain those of release 1.5.5.

## S8 timeout correction (2026-10-24)

**Intent:** A previously verified, saved/team recipient without a usable route must enter encrypted local custody promptly, including after a sender restart.
**Current behavior:** Only the identity hash survives restart. The public encryption key is lost; an unreachable send spends the normal path-resolution deadline before failing, without a durable outgoing message.
**Expected outcome:** Persist native-verified public recipient keys in the compiled library. A known eligible recipient with neither a native path nor an active delivery Link bypasses futile online attempts and uses existing encrypted custody only if its live recipient key is missing or no baseline-eligible relay has a native route/Link. A live recipient key plus a usable normal relay preserves baseline propagation. Missing never-learned keys return `UnresolvedRecipient` promptly.
**Target-Perspective Output:** The message shows Stored locally rather than timing out, then the same message is delivered when TCP restores reachability.
**Truth Owner:** Native verified identities, compiled LXMF preparation and the existing single SQLite custody owner.
**Contract Boundary:** REM supplies the destination and existing permission decision. The library reads verified native keys, validates their delivery-address binding on write/read, and prepares the existing signed/encrypted representation. No raw caller key admission.
**Cutover / Displaced Path:** Only the known unreachable preflight replaces futile legacy path attempts. All routed online sends retain the tested baseline policy. Cached keys never restore or invent a route.
**Value Density:** One bounded public-key cache and one offline preflight; no new queue, protocol, daemon, dependencies, settings or background timer.
**Evidence Lane / Acceptance Evidence:** Regression learns a verified identity, restarts with empty native maps, stores a chat within two seconds with encrypted bytes, and recovers the same message after a real transport reconnection. Existing online no-custody and unknown-key tests remain decisive. On Pixel, learn S8 over TCP, remove Pixel's TCP reachability, send once, restore TCP and inspect the same message; recipient confirmation remains separate from sender status.
**Kill Criteria:** Do not inject keys into global transport maps, restore path tables, weaken recipient validation, preserve plaintext pending intents, alter online timing, or claim the already failed message was stored retroactively.
**Architecture Slice:** Extract existing daemon identity schema/read/upsert SQL into `lxmf-propagation`; daemon delegates unchanged. Embedded cache uses the existing serialized connection and at most 4,096 entries, evicting oldest key metadata only. `lxmf-runtime` remembers only keys obtained from its native transport and resolves custody preparation from live keys/cache without a path wait. REM announce handling feeds that API; existing saved-peer matching and native route/Link checks select the early offline path.
**Plan Review Gate:** Requires PRE review before execution.

Tasks run sequentially in the main agent:

1. Shared identity persistence and native offline resolver. Allowed: `lxmf-propagation`, `lxmf-runtime` propagation modules/tests and existing `rns-rpc` identity storage delegation. Verify cache restart/binding/bound, prompt unknown-key rejection without transport-map insertion, and the existing daemon identity restart test. Cache failures must leave ordinary online delivery working. Avoid normal delivery and transport code.
2. REM verified announce hook and known unreachable preflight, with the narrow restart regression. Allowed: local propagation send, announce hook, closest native tests and dependency pin. Verify the three decisive offline/online tests and preservation checks. Avoid frontend, generated contracts, radio settings and normal send policy.
3. Review the bounded diff, build and install the production-style MVP on Pixel, then repeat the unreachable-to-TCP chat test. Preserve phone data and report recipient confirmation accurately; do not touch Poco. Publish evidence and update the existing draft PR, without releasing an unqualified artifact.
