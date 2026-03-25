# Performance Plan: Rust-First, Event-Driven Runtime

## Objective

Move business logic and protocol handling into the Rust runtime so TypeScript is reduced to visualization/state projection. Improve throughput, lower latency, and reduce UI-thread contention across peer discovery, telemetry, event replication, and delivery tracking.

## Current Performance Risks (Static Analysis)

### 1) Repeated JS decode/parse work on hot packet paths
- `apps/mobile/src/stores/eventsStore.ts` decodes incoming packet bytes (`TextDecoder`) and parses legacy payloads in the packet listener path.
- `apps/mobile/src/stores/telemetryStore.ts` performs multiple parsing attempts per packet (`parseCompatibleTelemetryMessage` / dedicated fields / JSON parse fallback) and converts base64 repeatedly.
- `packages/node-client/src/index.ts` decodes base64 payloads to `Uint8Array` for every packet event before dispatch.

**Impact:** CPU and allocation pressure in the JS thread under high message rates.

### 2) Duplicate transformation layers across JS ↔ plugin ↔ Rust
- `missionSync` and telemetry helpers in TS perform MsgPack/base64 packaging, then JNI decodes and Rust reparses.
- Similar base64 encode/decode helpers exist in multiple TS modules (`node-client`, `missionSync`, `telemetryStore`).

**Impact:** avoidable copies and encode/decode churn; harder to optimize globally.

### 3) Polling + interval-driven state refresh in UI stores
- `nodeStore` uses timers for presence and messaging snapshot refresh.
- `telemetryStore` uses recurring intervals for loop and staleness clock updates.
- Rust runtime also runs periodic maintenance loops (ack timeout watcher, stale cleanup).

**Impact:** periodic work continues even when no relevant state changes; duplicated scheduling logic across layers.

### 4) Broad reactive recomputation in Pinia
- `eventsStore` and `telemetryStore` repeatedly sort/filter full maps to compute derived lists.
- `watch` callbacks in stores fan out network calls or synchronization work when route sets change.

**Impact:** O(n log n) recomputation on updates; UI stalls as peer/event counts grow.

### 5) Global EventBus cloning in Rust broadcast path
- `crates/reticulum_mobile/src/event_bus.rs` emits cloned `NodeEvent` to all subscribers while holding a mutex and retaining live senders.

**Impact:** fan-out cost scales with subscriber count and event payload size; lock contention risk under high event volume.

### 6) Business logic still split between TS and Rust
- Mission/event semantics, snapshot choreography, and telemetry format compatibility parsing remain partly in TS stores.
- Rust already has delivery/ack tracking and transport-level context but TS still orchestrates workflow decisions.

**Impact:** cross-layer orchestration overhead, duplicated edge-case handling, and migration blockers for a true event-driven runtime.

---

## Target Architecture

## Principle
- **Rust owns protocol + business logic + state transitions.**
- **TS owns rendering + user intent dispatch only.**

## Event-driven contract
1. TS sends **commands** only (e.g., `CreateEvent`, `PublishTelemetry`, `RequestSnapshot`, `ConnectPeer`).
2. Rust emits **domain events** only (e.g., `EventUpserted`, `TelemetryUpdated`, `PeerReadinessChanged`, `DeliveryStateChanged`).
3. TS stores become projection caches that apply immutable event payloads.

## New boundary
- Replace packet-level callbacks in TS (`onPacket`) with typed domain-event streams from Rust.
- Keep wire encoding/decoding and fallback compatibility parsing in Rust.
- Pass binary payloads across bridge without JS base64 conversion when platform permits; otherwise centralize conversion once at bridge edge.

---

## Migration Plan

## Phase 0 — Baseline and Guardrails (1 sprint)
1. Add runtime metrics in Rust:
   - packet receive -> domain event emit latency
   - command submit -> delivery terminal state latency
   - queue depth for command/event channels
   - serialization/deserialization time buckets
2. Add JS performance marks around store update/render cycles.
3. Define SLOs:
   - p95 packet->UI projection < 150ms on 200-peer simulation
   - no dropped domain events at sustained 50 msg/s

**Deliverables**
- Metrics schema in Rust runtime and bridge-exposed diagnostic endpoint.
- Performance dashboard doc update in `docs/architecture.md`.

## Phase 1 — Domain-event API in Rust (1–2 sprints)
1. Add typed runtime events in `crates/reticulum_mobile/src/types.rs` and bridge mapping in `jni_bridge.rs`.
2. Move mission/event parse + command interpretation from TS (`eventsStore`, `missionSync`) into Rust handlers.
3. Move telemetry compatibility parsing and snapshot request/response choreography into Rust.
4. Expose only domain events to TS node-client (remove packet-level requirement for app logic).

**Deliverables**
- New plugin event names and TS typings in `packages/node-client/src/index.ts`.
- TS stores consume domain events only.

## Phase 2 — Command bus + runtime-owned workflows (1 sprint)
1. Introduce command queue in Rust (`runtime.rs`) for user intents.
2. Convert TS calls:
   - `sendBytes` workflow logic -> typed command methods
   - replication fanout decisions -> Rust policy module
3. Move retry, timeout, and backoff policies fully to Rust.

**Deliverables**
- Stable command API at bridge boundary.
- Removal of TS-side fanout/ack orchestration branches.

## Phase 3 — Store simplification in TS (1 sprint)
1. Collapse `eventsStore` and `telemetryStore` logic to projection reducers:
   - apply incoming domain events
   - maintain lightweight indexes for UI
2. Remove JSON/msgpack/base64 transport helpers from TS stores.
3. Keep only formatting/filtering needed for views.

**Deliverables**
- TS stores with no transport/protocol branching.
- Reduced watchers and timers.

## Phase 4 — Throughput optimization and lock reduction (1 sprint)
1. Optimize Rust event fan-out:
   - replace mutex-guarded subscriber vec with lock-light broadcast primitive or segmented channels
   - avoid cloning large payloads when possible (Arc/shared payload sections)
2. Add bounded channels + backpressure behavior for non-critical event classes.
3. Introduce coalescing for high-frequency telemetry updates.

**Deliverables**
- Load-test report with before/after CPU, latency, memory.

---

## Blockers and How to Avoid Them

### Blocker A: Breaking plugin compatibility during API transition
**Mitigation:** dual-path bridge for 1 release cycle (packet events + domain events), feature flag in TS.

### Blocker B: Event schema churn between Rust and TS
**Mitigation:** versioned event envelope (`schemaVersion`), strict decoding with fallback logging.

### Blocker C: JS thread still doing hidden heavy work
**Mitigation:** ban transport parsing in stores; enforce via lint rule + code ownership checklist.

### Blocker D: Runtime lock contention at scale
**Mitigation:** instrument lock hold times; prioritize `event_bus` refactor and channel topology review.

### Blocker E: Migration stalls due to mixed responsibilities
**Mitigation:** per-layer ownership matrix and CI checks that prevent new protocol logic in TS.

---

## Implementation Backlog (Prioritized)

1. **Rust domain-event types and bridge exposure**
   - Files: `crates/reticulum_mobile/src/types.rs`, `jni_bridge.rs`, `runtime.rs`
2. **TS client API update to domain events**
   - File: `packages/node-client/src/index.ts`
3. **Events pipeline migration**
   - Files: `apps/mobile/src/stores/eventsStore.ts`, `apps/mobile/src/utils/missionSync.ts`
4. **Telemetry pipeline migration**
   - File: `apps/mobile/src/stores/telemetryStore.ts`
5. **Remove packet parsing from TS stores**
   - Files: `eventsStore.ts`, `telemetryStore.ts`, `replicationParser.ts`
6. **EventBus / channel performance refactor**
   - File: `crates/reticulum_mobile/src/event_bus.rs`

---

## Acceptance Criteria

- TS stores contain no protocol decode/encode code paths for event/telemetry replication.
- TS receives typed domain events only for replication flows.
- Rust owns retry, timeout, ack tracking, and snapshot orchestration.
- p95 packet->projected UI update improved by at least 40% from baseline.
- CPU time on JS main thread reduced measurably during sustained message intake.

---

## Verification Strategy (Static + Runtime Once Enabled)

1. **Static checks**
   - Ensure no `onPacket`-driven protocol workflows remain in TS stores.
   - Ensure transport helpers are not imported in UI stores.
2. **Runtime perf checks**
   - Synthetic load across 50/100/200 peers.
   - Measure end-to-end latencies and dropped-event counters.
3. **Regression checks**
   - Delivery lifecycle correctness (`Sent`, `Acknowledged`, `TimedOut`, `Failed`).
   - Snapshot sync correctness for events and telemetry.

