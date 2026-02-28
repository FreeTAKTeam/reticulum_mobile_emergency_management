# R3AKT Client -- CODEX Implementation Plan

**Generated:** 2026-02-27 UTC

------------------------------------------------------------------------

# ROLE

You are implementing a production-grade **R3AKT client** inside the existing
`reticulum_mobile_emergency_management` monorepo.

The implementation must extend the current stack:

- Rust (`crates/reticulum_mobile`)
- UniFFI (Rust -> Kotlin/Swift bindings)
- Capacitor plugin bridge (Android primary, iOS supported)
- Vue 3 + TypeScript frontend (`apps/mobile`)
- Existing TypeScript wrapper (`packages/node-client`)

The phone MUST continue to run Reticulum locally.

Your task is to evolve the current mobile Reticulum node into a full
R3AKT-capable client using **message-contract operations**, local Rust runtime
logic, native bridge extensions, and Vue-facing typed APIs.

------------------------------------------------------------------------

# CONTEXT (MANDATORY)

This plan is adapted from the existing mobile implementation prompt, but the
target is no longer just a generic mobile Reticulum node. The target is a
**client-side R3AKT implementation** with these constraints:

- The OpenAPI file is **not** a REST implementation target.
- `API/ReticulumCommunityHub-OAS.yaml` is only a description of operation
  intent and payload shapes.
- Southbound messages MUST be carried over **Reticulum/LXMF**.
- Northbound data MUST be exposed to Vue like the current app:
  - native plugin methods
  - pushed events
  - TypeScript wrapper
- Only the operations classified as `client` are in scope.
- `server-only` and `unknown` functions are out of scope unless explicitly
  approved later.

Authoritative scope references:

- `docs/plan/APIANnalysis.md`
- `docs/plan/APIANnalysis_clientImplementationSet.md`
- `docs/plans/r3aktClient_implementationPlan.md`

------------------------------------------------------------------------

# REPOSITORY STRUCTURE (MANDATORY)

Preserve the current monorepo and extend it. Do not redesign the repo.

Existing structure:

- `apps/mobile` -> Vue + Capacitor application
- `packages/node-client` -> TypeScript wrapper around the Capacitor plugin
- `crates/reticulum_mobile` -> Rust UniFFI wrapper crate
- `tools/codegen` -> UniFFI generation scripts

Required additions must stay within that structure:

- Add new modules under `crates/reticulum_mobile` for:
  - message contract
  - R3AKT client runtime
  - local persistence
  - sync/event routing
- Extend `packages/node-client` instead of creating a separate client package.
- Extend `apps/mobile` with new stores/views/components using the existing
  Vue 3 + Pinia structure.

Do not add a REST server crate for feature delivery.

------------------------------------------------------------------------

# NON-NEGOTIABLE ARCHITECTURE

## 1. API Model

Treat the OAS as a **message schema reference**, not as endpoint work.

Create and maintain:

- `API/ReticulumCommunityHub-Messages.yaml`

This file must map only `client` operations into:

- `Query` messages for read operations
- `Command` messages for mutating operations
- `Event` messages for streamed/live updates

## 2. Scope Filter

Use `docs/plan/APIANnalysis_clientImplementationSet.md` as the implementation
backlog and sequencing source.

Do not implement:

- any `server-only` operation
- any `unknown` operation

unless the scope is widened in a later task.

## 3. Transport Rules

Southbound:

- internal runtime -> Reticulum/LXMF messages
- inbound LXMF -> decode -> validate -> dispatch to client runtime

Northbound:

- Rust runtime -> native plugin bridge -> `packages/node-client` -> Vue
- no feature REST calls

## 4. Delivery Model

This is an **offline-first client**.

The runtime must:

- retain local state
- survive transient connectivity
- sync incrementally when peers/hub messages are available

------------------------------------------------------------------------

# PHASE 1 -- MESSAGE CONTRACT AND OPERATION FILTER

## 1. Freeze the client-only operation set

Use `docs/plan/APIANnalysis_clientImplementationSet.md` as the allowed feature
surface.

The current client-only scope is grouped in this exact build order:

1. Core Discovery and Session
2. Telemetry and Live Status
3. Messaging and Chat
4. Topics and Distribution
5. Files and Media
6. Map, Markers, and Zones
7. R3AKT Mission Core
8. R3AKT Teams, People, and Skills
9. R3AKT Assets and Assignments
10. Checklists

## 2. Define canonical message envelope

All message families must share a stable envelope:

- `api_version`
- `message_id`
- `correlation_id`
- `kind` (`command | query | result | event | error`)
- `type`
- `issuer`
- `issued_at`
- `payload`

## 3. Define message catalog

For each client operation:

- assign a stable `type`
- define request payload shape
- define result payload shape
- define error mapping
- define emitted events (if any)

Do not expose untyped ad-hoc payloads.

------------------------------------------------------------------------

# PHASE 2 -- RUST CORE EXTENSION (`crates/reticulum_mobile`)

## 1. Keep the existing node model

Do not replace the current `Node` lifecycle foundation.

Retain and extend:

- local Reticulum runtime
- command channel
- event broadcast channel
- node state tracking

## 2. Add R3AKT client runtime modules

Implement internal modules under `crates/reticulum_mobile/src/`:

- `contract/` -> message envelope and typed payloads
- `client_runtime/` -> command/query dispatch + feature orchestration
- `client_storage/` -> local persistence for cached/synced state
- `sync/` -> southbound Reticulum message translation and correlation handling
- `r3akt_domain/` -> mission/checklist/team/asset state management

## 3. Add client runtime object

Implement an internal runtime object that composes with the current node:

- tracks hub session/join state
- executes typed client commands and queries
- manages message correlation and timeouts
- emits domain events for Vue consumption
- updates local cache/storage

## 4. Required event families

Extend the existing event system with client-facing events such as:

- `HubSessionChanged`
- `ClientDirectoryUpdated`
- `SystemStatusUpdated`
- `TelemetrySnapshotReceived`
- `TelemetryUpdated`
- `HubMessageReceived`
- `HubMessageSent`
- `TopicListUpdated`
- `FileListUpdated`
- `ImageListUpdated`
- `MarkerUpdated`
- `ZoneUpdated`
- `MissionUpdated`
- `MissionChangeReceived`
- `ChecklistUpdated`
- `TeamUpdated`
- `AssetUpdated`
- `AssignmentUpdated`
- `SyncError`

All events must be:

- thread-safe
- serializable across FFI
- safe against panic leakage

## 5. Persistence requirements

Persist client-side state needed for offline-first use:

- hub session metadata
- client directory cache
- topic subscriptions
- cached telemetry snapshots
- cached file/image metadata
- markers/zones
- mission/checklist/team/asset state

Use stable, explicit schemas. Do not rely on implicit JSON blobs when a typed
representation is reasonable.

------------------------------------------------------------------------

# PHASE 3 -- UNIFFI AND NATIVE BRIDGE EXTENSION

## 1. Extend the UniFFI public interface

Extend the existing UniFFI surface to expose:

- grouped client query methods
- grouped client command methods
- event subscription for the expanded event model

Do not create a second unrelated FFI surface. Extend the current one.

## 2. Preserve native integration model

Keep:

- Android polling/event emission model
- iOS polling/event emission model
- Base64 encoding for binary payloads on the JS boundary where needed

## 3. Safety requirements

- no `unwrap()` across FFI boundaries
- no backtraces exposed over FFI
- structured `NodeError` / client-operation error types only

------------------------------------------------------------------------

# PHASE 4 -- TYPESCRIPT WRAPPER (`packages/node-client`)

## 1. Extend, do not replace

Expand `packages/node-client` so it remains the single TS entrypoint for the
mobile app.

## 2. Add grouped client APIs

Expose typed feature groups aligned to the client implementation set:

- core/session
- telemetry
- messaging
- topics
- files/media
- map
- mission core
- teams/skills
- assets/assignments
- checklists

Methods must be explicit and typed. Avoid a single generic
`sendMessage(type, payload)` JS API for Vue.

## 3. Preserve current dev ergonomics

Keep mock/browser behavior for web development where possible.

The mock mode must simulate:

- status/session changes
- message receive/send events
- telemetry updates
- R3AKT mission/checklist state changes

------------------------------------------------------------------------

# PHASE 5 -- VUE APP EXPANSION (`apps/mobile`)

## 1. Preserve current frontend conventions

Use:

- Vue 3
- Composition API
- Pinia
- route-level views plus store-driven state

## 2. Build in feature-area order

Implement UI/state work in the exact order defined by
`docs/plan/APIANnalysis_clientImplementationSet.md`:

1. Core Discovery and Session
2. Telemetry and Live Status
3. Messaging and Chat
4. Topics and Distribution
5. Files and Media
6. Map, Markers, and Zones
7. R3AKT Mission Core
8. R3AKT Teams, People, and Skills
9. R3AKT Assets and Assignments
10. Checklists

Do not start checklist UX before mission/team/asset foundations are working.

## 3. State model rules

- Keep stores typed and feature-scoped.
- Use event-driven updates from the plugin wrapper.
- Persist only client-relevant state locally.
- Keep route views thin; place domain logic in stores/composables.

------------------------------------------------------------------------

# PHASE 6 -- TESTING

## Rust

Add tests for:

- message envelope serialization/deserialization
- command/query correlation handling
- Reticulum message translation
- local cache updates after inbound events
- client runtime state transitions
- no deadlocks in expanded event channels

## Native

Verify:

- Android still starts/stops the node correctly
- Android still emits all plugin events
- iOS start/stop/resume remains stable

## TypeScript

Add tests for:

- typed wrapper method behavior
- event emitter behavior
- mock mode parity for new client features

## Vue

Verify:

- stores consume new APIs correctly
- views render without runtime errors
- feature groups unlock in planned order

------------------------------------------------------------------------

# SECURITY REQUIREMENTS

- Never log secrets, tokens, or private identity material.
- Do not trust inbound payloads; validate all southbound messages.
- No unvalidated file paths.
- No arbitrary deserialization into untyped runtime state.
- Enforce scope boundaries; do not expose `server-only` features by accident.

------------------------------------------------------------------------

# SUCCESS CRITERIA

Implementation is complete when:

1. The mobile app still runs a local Reticulum node.
2. The app can execute all operations in
   `docs/plan/APIANnalysis_clientImplementationSet.md`.
3. No operation from the `server-only` or `unknown` sets is implemented in the
   phase-1 message catalog or native bridge.
4. Vue can render and react to the expanded event model without REST feature
   dependencies.
5. Core flows work end-to-end:
   - join/leave
   - status + telemetry
   - send/receive messages
   - topic subscription
   - file/image retrieval and deletion
   - marker/zone workflows
   - mission/team/asset/checklist workflows

------------------------------------------------------------------------

# IMPLEMENTATION ORDER (STRICT)

1. Freeze the client-only operation set
2. Define the message catalog and envelope
3. Extend Rust core runtime
4. Add southbound Reticulum translation
5. Extend UniFFI and native bridge
6. Extend `packages/node-client`
7. Implement Vue features in grouped feature order
8. Add tests at each layer
9. Validate that no out-of-scope operations leaked in

Do not skip order.

------------------------------------------------------------------------

END OF PLAN
