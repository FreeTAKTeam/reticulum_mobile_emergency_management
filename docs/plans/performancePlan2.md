# Performance Improvement Plan

## Repository
`FreeTAKTeam/reticulum_mobile_emergency_management`

## Objective

Improve performance and remove architectural blockers by making the mobile application event-driven end to end, with Rust owning runtime, domain behavior, state transitions, persistence, replication, retry, and synchronization. TypeScript must be reduced to presentation, view composition, and user interaction only.

---

## Executive summary

The current application is split across three major areas:

1. a Vue/Capacitor mobile shell,
2. a TypeScript bridge package,
3. a Rust runtime and transport core.

That split is reasonable, but the current ownership is not. Too much domain behavior remains in TypeScript stores. The UI layer still performs replication fanout, delivery tracking, retry behavior, hydration, local persistence, normalization, peer routing decisions, and state reconciliation. This creates unnecessary work on the UI thread, duplicates logic across layers, increases bridge chatter, and makes performance dependent on Pinia/Vue reactivity rather than on the Rust runtime.

The strongest architectural conclusion is this:

**Rust is already strong enough to be the operational core, but the app still behaves as if TypeScript were a co-runtime. That must stop.**

The plan below moves the application toward this target:

- **Rust:** authoritative runtime, event engine, persistence, replication, message/event/EAM state machines, peer graph, sync orchestration, projections
- **TypeScript:** render-only projections, command dispatch, local screen state, formatting for views

---

## Evidence from the current codebase

### 1. TypeScript still owns business and sync behavior

The mobile app depends on Vue, Pinia, MapLibre, Capacitor, and a local `@reticulum/node-client` bridge package, which confirms a three-layer split rather than a thin UI over a native core.  
Source: `apps/mobile/package.json`

The node bridge package is only a TypeScript wrapper around the Capacitor plugin, but it contains a large amount of mapping, event listener registration, base64 conversion, normalization, and in-browser mock/runtime logic. This means the bridge is more than a transport boundary and still performs substantial runtime work.  
Source: `packages/node-client/src/index.ts`

The `nodeStore.ts` file is especially heavy. It contains:

- peer normalization,
- peer ranking,
- propagation candidate selection,
- saved-peer auto-connect queues,
- identity resolution orchestration,
- startup settling,
- local persistence via `localStorage`,
- messaging snapshot refresh cooldown logic,
- hub registration bootstrap,
- runtime logging strategy,
- multiple reactive projections and derived peer route sets.

This is not presentation logic. It is application runtime logic living in the UI store.  
Source: `apps/mobile/src/stores/nodeStore.ts`

The `messagesStore.ts` still owns:

- EAM replication fanout,
- command tracking,
- pending delivery trackers,
- retry of errored messages,
- hydration of peer state,
- local persistence,
- draft replay,
- mission packet parsing,
- command/response handling.

This is domain orchestration and should not be in Pinia.  
Source: `apps/mobile/src/stores/messagesStore.ts`

The `eventsStore.ts` still owns:

- event replication fanout,
- event replay to peers,
- mission sync handling,
- delivery tracker management,
- event persistence,
- peer sync logic,
- replay cooldown policy,
- event upsert/delete behavior.

Again, this is runtime logic, not UI logic.  
Source: `apps/mobile/src/stores/eventsStore.ts`

### 2. Rust already behaves as the real runtime

The Rust `runtime.rs` already owns:

- transport lifecycle,
- announce sending and receiving,
- packet and resource receivers,
- LXMF send policy,
- delivery receipts,
- acknowledgement buffering,
- pending delivery timeout handling,
- peer resolution,
- hub refresh,
- messaging store integration,
- command processing.

This is already the natural home for the rest of the application logic.  
Source: `crates/reticulum_mobile/src/runtime.rs`

The Rust event bus currently broadcasts by cloning each event to every subscriber while holding a mutex over the subscriber list:

```rust
pub fn emit(&self, event: NodeEvent) {
    let Ok(mut guard) = self.subscribers.lock() else {
        return;
    };
    guard.retain(|tx| tx.send(event.clone()).is_ok());
}
```

That is acceptable at low event volume, but it will become a cost center as event payloads grow or subscriber count increases.  
Source: `crates/reticulum_mobile/src/event_bus.rs`

---

## Main performance issues

## Issue 1: UI stores are acting as runtime services

### Why this hurts
Pinia stores and Vue reactivity are being used for orchestration instead of display. That adds:

- avoidable allocations,
- repeated recomputation,
- frequent reactive invalidation,
- timer-based polling and cooldown logic on the UI side,
- larger JS heap pressure,
- more bridge crossings.

### Consequence
As peer count, event volume, and message volume grow, performance will degrade first in TypeScript, not Rust.

### Required change
Move all domain workflows into Rust.

---

## Issue 2: Duplicate state machines across Rust and TypeScript

There are multiple examples where Rust tracks peer, sync, or delivery state while TypeScript derives, enriches, retries, or reinterprets the same lifecycle. That leads to:

- divergence risk,
- duplicate transitions,
- more event handling,
- harder reasoning about correctness,
- extra bridge traffic.

### Required change
Rust becomes the single source of truth for:

- peer state,
- replication state,
- delivery state,
- sync state,
- persisted projections.

TypeScript must not reconstruct these.

---

## Issue 3: Too many high-frequency bridge events

The bridge currently carries many detailed runtime events into JS where they are again normalized and persisted. Even if each event is individually cheap, the cumulative cost grows quickly with:

- announce bursts,
- peer changes,
- message updates,
- delivery updates,
- hydration and replay responses.

### Required change
Export **coarser projections** from Rust rather than raw operational chatter whenever possible.

Examples:

- `PeerListChanged`
- `ConversationListChanged`
- `MessageListChanged(conversationId)`
- `MissionTimelineChanged`
- `EamListChanged`
- `OperationalSummaryChanged`

---

## Issue 4: Local persistence is in TypeScript

The stores persist messages, events, settings, and saved peers via `localStorage`. That is fast to prototype, but it is weak for a mobile operational app.

### Why this hurts
- serialization happens on the JS side,
- hydration happens on the JS side,
- no strong transactional model,
- limited indexing,
- persistence competes with rendering work.

### Required change
Move persistence to Rust. Use a native store such as SQLite or `redb`/`sled` depending on access pattern. SQLite is the better default because it supports indexed projections cleanly.

---

## Issue 5: Repeated normalization and transformation in the bridge

The TypeScript bridge contains many conversion helpers, event mappers, enum conversions, base64 encode/decode helpers, and fallback runtime variants.

### Why this hurts
- repeated CPU work,
- repeated object allocation,
- large bridge module complexity,
- difficult profiling.

### Required change
The bridge should become mostly generated or near-mechanical. It should expose a compact command API and a small set of projection subscriptions.

---

## Issue 6: Event bus cloning in Rust

`EventBus` clones each `NodeEvent` for each subscriber and uses a `Mutex<Vec<Sender<NodeEvent>>>`.

### Why this hurts
At small scale it is fine. At larger scale it can become expensive because:

- large payloads are cloned per subscriber,
- subscriber iteration is serialized by a mutex,
- backpressure strategy is coarse.

### Required change
Replace it with one of these patterns:

1. `tokio::sync::broadcast` for lightweight fanout,
2. `Arc<NodeEvent>` instead of cloning full payloads,
3. domain-specific channels where high-volume streams are separated.

Recommended approach: **`broadcast` + `Arc` payloads for heavy events**.

---

## Target architecture

## Rule of ownership

### Rust owns
- peer discovery and peer graph
- route resolution
- propagation candidate selection
- announce handling
- packet parsing and command decoding
- EAM state machine
- event state machine
- mission sync
- retries
- replay
- hydration
- persistence
- projections
- telemetry aggregation
- message history
- conversations
- delivery tracking
- sync policies
- log throttling and event coalescing

### TypeScript owns
- screen navigation
- view rendering
- map rendering
- user input
- local ephemeral UI state
- formatting for display
- invoking commands against Rust
- subscribing to read-only projections from Rust

---

## Proposed runtime model

Adopt a strict **command/query + evented projection** model.

### Commands from TypeScript to Rust
Examples:

- `StartNode`
- `StopNode`
- `ConnectPeer`
- `DisconnectPeer`
- `UpsertEam`
- `DeleteEam`
- `UpsertEvent`
- `DeleteEvent`
- `RequestTeamSummary`
- `RefreshHubDirectory`
- `SetSettings`
- `SetMapViewport` if needed for telemetry relevance filtering

### Queries from TypeScript to Rust
Examples:

- `GetOperationalSummary`
- `GetPeers`
- `GetPeerDetails`
- `GetConversationList`
- `GetMessages(conversationId, page)`
- `GetEamList(filter, page)`
- `GetEventTimeline(filter, page)`
- `GetSyncStatus`

### Events from Rust to TypeScript
Only coarse-grained UI-safe events:

- `OperationalSummaryChanged`
- `PeersChanged`
- `ConversationsChanged`
- `MessagesChanged`
- `EamChanged`
- `EventsChanged`
- `SyncStatusChanged`
- `ToastEvent`
- `ErrorEvent`

The UI should then issue a query to refresh the affected projection rather than rebuild state from dozens of low-level events.

---

## Refactoring plan

## Phase 1 — Stabilize boundaries

### Goals
- define what belongs to Rust vs TypeScript
- stop adding new logic to Pinia stores
- document the contract

### Actions
1. Create a formal ownership matrix for each feature area.
2. Freeze TypeScript stores to presentation-related changes only.
3. Define Rust-facing command/query/event contracts.
4. Introduce projection DTOs for UI consumption.

### Deliverables
- `docs/architecture/runtime-ownership.md`
- `docs/architecture/native-contract.md`

### Acceptance criteria
- No new retry, hydration, replication, or persistence logic is added in TypeScript.
- All future work references the ownership matrix.

---

## Phase 2 — Move persistence to Rust

### Goals
- eliminate `localStorage` as system-of-record
- reduce JS hydration cost
- enable indexed projections

### Actions
1. Introduce native persistence for:
   - settings
   - saved peers
   - EAM records
   - events
   - conversations
   - messages
   - sync metadata
2. Add migration from current `localStorage` payloads.
3. Expose read models via query APIs.
4. Keep TypeScript copies read-only and projection-based.

### Recommended storage
SQLite with indexed tables and append-friendly event history.

### Acceptance criteria
- On app start, TypeScript no longer loads operational records from `localStorage`.
- Cold start time improves measurably.
- Message/event/EAM history survives restart without JS-side reconstruction.

---

## Phase 3 — Move EAM logic to Rust

### Goals
- remove EAM replication from `messagesStore.ts`
- make Rust the authoritative EAM engine

### Actions
1. Port these behaviors into Rust:
   - EAM normalization
   - EAM persistence
   - EAM upsert/delete command handling
   - fanout
   - retry of errored deliveries
   - draft replay
   - team summary computation
   - inbound packet/result processing
   - delivery tracking
2. Add Rust projections:
   - `EamListProjection`
   - `EamTeamSummaryProjection`
   - `EamSyncStateProjection`
3. Replace TypeScript store code with:
   - command invocation
   - projection subscription
   - query refresh

### Acceptance criteria
- `messagesStore.ts` no longer owns fanout, retry, or mission packet handling.
- EAM behavior continues to work with TypeScript disconnected from business logic.

---

## Phase 4 — Move mission/event logic to Rust

### Goals
- remove event replication and replay from `eventsStore.ts`
- centralize mission/event consistency

### Actions
1. Port into Rust:
   - event normalization
   - event persistence
   - event upsert/delete
   - replay logic
   - peer hydration
   - mission command handling
   - snapshot/list handling
2. Add Rust projections:
   - `MissionTimelineProjection`
   - `EventDetailProjection`
   - `MissionSyncProjection`

### Acceptance criteria
- `eventsStore.ts` becomes a thin adapter.
- Replay cooldown and hydration state are not managed in JS anymore.

---

## Phase 5 — Reduce nodeStore to nodeViewModel

### Goals
- stop using `nodeStore.ts` as an orchestration engine
- keep only presentation-safe derived values in TypeScript

### Actions
1. Move to Rust:
   - peer ranking
   - propagation selection
   - identity resolution workflows
   - auto-connect queue
   - startup settle logic
   - hub bootstrap orchestration
   - periodic refresh scheduling
2. Replace with projections:
   - `PeerProjection`
   - `ConnectivityProjection`
   - `HubRegistrationProjection`
   - `TransportProjection`

### Acceptance criteria
- `nodeStore.ts` stops using timers for operational behavior.
- Most of its functions become query wrappers or command dispatchers.

---

## Phase 6 — Simplify the Capacitor bridge

### Goals
- minimize bridge overhead
- make the bridge mechanical

### Actions
1. Remove conversion-heavy behavior from `@reticulum/node-client`.
2. Prefer native JSON DTOs or compact typed payloads with stable schema.
3. Eliminate mock/runtime logic from production bridge paths.
4. Consolidate listeners into a smaller number of projection change events.
5. Avoid base64 unless strictly necessary.

### Acceptance criteria
- Bridge package becomes small and predictable.
- Event count across the bridge drops significantly.

---

## Phase 7 — Optimize Rust event distribution

### Goals
- avoid per-subscriber heavy cloning
- prepare for higher event volume

### Actions
1. Replace the current mutexed subscriber list with `tokio::sync::broadcast` or equivalent.
2. Use `Arc<NodeEvent>` or split heavy payloads from light notifications.
3. Separate high-volume transport events from UI-facing projection invalidations.
4. Introduce coalescing for repeated peer-change chatter.

### Acceptance criteria
- Event bus fanout no longer clones large payloads N times in the hot path.
- Burst traffic does not degrade UI responsiveness.

---

## Phase 8 — Add backpressure and batching

### Goals
- keep the runtime stable under bursts
- prevent UI overload

### Actions
1. Batch peer changes over short windows, for example 100–250 ms.
2. Batch delivery updates where the UI only needs final states.
3. Throttle logs crossing into TypeScript.
4. Prefer invalidation events plus query pull over raw push streams.

### Acceptance criteria
- announce bursts and sync bursts do not flood JS.
- UI remains responsive under heavy peer churn.

---

## Concrete technical recommendations

## 1. Introduce native projections

Create read-only projection structs in Rust such as:

- `OperationalSummaryDto`
- `PeerListDto`
- `ConversationListDto`
- `MessagePageDto`
- `EamPageDto`
- `EventTimelineDto`
- `SyncStatusDto`

Each projection should be cheap to query and already sorted/filtered for the UI.

## 2. Replace local timers in TypeScript

Current JS timers for:
- presence ticking,
- startup settling,
- messaging refresh cooldown,
- auto-connect queue draining,
- hydration retry,

should be removed from JS and implemented in Rust schedulers.

## 3. Replace low-level push with invalidation push

Instead of streaming every mutation detail to JS:

- push `PeersChanged(version=123)`
- JS calls `getPeers()`

This is more stable and cheaper.

## 4. Keep map rendering in TypeScript, not map data preparation

MapLibre rendering stays in TypeScript. But all preparation of map-worthy operational objects should happen in Rust:

- filtering stale peers,
- selecting visible telemetry,
- building marker DTOs,
- clustering inputs if required.

## 5. Unify outbound delivery policies in Rust

Rust already has strong send policy logic in `send_lxmf_with_delivery_policy`. Extend this pattern to all operational delivery handling and delete JS-side delivery trackers.

## 6. Add pagination for large lists

Do not expose entire event/message/EAM history as one large array to the UI. Queries should support:

- page size,
- cursor or offset,
- sort mode,
- optional filters.

## 7. Add a real profiling baseline

Before and after each migration phase, measure:

- cold start
- warm start
- peer discovery burst handling
- event fanout latency
- EAM update latency
- memory growth over 15 minutes
- bridge event count per minute
- JS heap size
- dropped frames during map interaction

---

## Suggested performance metrics

## Mobile startup
- cold start to first usable screen
- cold start to node ready
- warm start to node ready

## Runtime throughput
- average and p95 delivery latency for EAM update
- average and p95 event replication latency
- peer hydration completion time

## UI responsiveness
- dropped frames during announce burst
- dropped frames during map pan/zoom
- JS thread blocking time

## Resource usage
- native memory
- JS heap size
- event bus queue depth
- bridge events per minute
- disk write volume

---

## Prioritized blockers

## Blocker 1
**TypeScript stores own domain behavior.**  
This is the most important issue. It prevents a true event-driven architecture and makes performance scale poorly.

## Blocker 2
**Persistence is not native.**  
This keeps hydration and indexing on the JS side.

## Blocker 3
**Bridge event granularity is too fine.**  
The UI is consuming operational chatter rather than projections.

## Blocker 4
**Rust event bus cloning strategy will become expensive under load.**

---

## Short-term action list

1. Stop new business logic in `nodeStore.ts`, `messagesStore.ts`, and `eventsStore.ts`.
2. Add native persistence for EAM and events first.
3. Move EAM fanout/retry/replay into Rust.
4. Move event replay/hydration into Rust.
5. Replace UI-side projections with Rust query APIs.
6. Replace raw event streams with projection invalidation events.
7. Refactor Rust event bus to reduce clone-heavy fanout.

---

## Recommended implementation order

### Sprint 1
- define native contracts
- add projection DTOs
- add Rust persistence abstraction
- add migration path from `localStorage`

### Sprint 2
- move EAM engine to Rust
- expose EAM queries and summary projection
- simplify `messagesStore.ts`

### Sprint 3
- move event engine to Rust
- expose event timeline query
- simplify `eventsStore.ts`

### Sprint 4
- move peer orchestration and hub bootstrap to Rust
- simplify `nodeStore.ts`

### Sprint 5
- optimize event bus and bridge
- add batching, coalescing, profiling dashboards

---

## Current Cutover Progress

The current branch has already landed part of the runtime cutover and should be evaluated from that newer baseline:

- mobile now has a native SQLite-backed app-state store in Rust
- mobile stores query native projections for settings, saved peers, EAMs, events, telemetry, and conversations/messages
- a single global legacy-import coordinator is replacing the earlier per-slice import flow
- `nodeStore` no longer acts as a general raw packet/delivery/message relay for other stores
- UI-only preferences were split away from native runtime settings

The remaining performance work should therefore prioritize:

1. proving long-session native ownership of every operational lifecycle under soak
2. finishing telemetry cutover so TS keeps only permission UX and local GPS handoff
3. reducing remaining bridge chatter to projection invalidation plus explicit queries
4. replacing any remaining raw transport event dependencies in mobile app logic

## Tooling Note

UniFFI binding generation should no longer assume a globally installed `uniffi-bindgen` executable. The supported repo-local path is:

- `powershell -ExecutionPolicy Bypass -File .\\tools\\codegen\\generate-uniffi-bindings.ps1 -Language kotlin`
- `powershell -ExecutionPolicy Bypass -File .\\tools\\codegen\\generate-uniffi-bindings.ps1 -Language swift`

The codegen script now falls back to the workspace runner in `tools/uniffi-bindgen` when no PATH-level `uniffi-bindgen` is available.

---

## What TypeScript should look like after refactoring

A healthy TypeScript layer should mostly contain:

- view models,
- route/page composition,
- command calls like `await native.upsertEam(dto)`,
- subscriptions like `native.on("EamChanged", ...)`,
- queries like `await native.getEams(filter)`.

It should **not** contain:

- retry logic,
- replication fanout,
- packet parsing,
- delivery trackers,
- persistence rules,
- peer graph logic,
- synchronization strategies,
- replay engines.

---

## Final recommendation

Do not optimize the current layering incrementally by adding more caching or throttling in TypeScript. That would improve symptoms while preserving the wrong ownership model.

The correct path is to **finish the native runtime transition**:

- Rust becomes the sole operational engine.
- TypeScript becomes a read-only presentation client with command dispatch.

This will improve:

- performance,
- correctness,
- observability,
- offline resilience,
- maintainability,
- future scalability.

---

## Confidence

**0.92**

The confidence is high because the repository already contains a substantial Rust runtime and the TypeScript stores clearly show duplicated orchestration responsibilities. The only missing piece is measured profiling, so the exact order of the worst hotspots may change once instrumented, but the architectural conclusion is stable.
