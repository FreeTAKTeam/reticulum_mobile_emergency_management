# R3AKT Client Implementation Plan (Message-Contract Architecture)

## Summary
This plan delivers a full R3AKT client and Rust Hub backend without implementing REST as the feature API.
The existing OpenAPI file is treated as a contract source for message intent and payload shapes only.
Southbound transport is Reticulum/LXMF, and northbound integration follows the current emergency mobile app pattern: typed client/plugin methods plus pushed events into Vue stores.
Implementation scope is restricted to the operations classified as `client` in `docs/plan/APIANnalysis.md`.
`docs/R3AKT_client_CODEX_Implementation_plan.md` is the primary execution document for implementation order and delivery details; this file defines the architectural scope, inclusion rules, and phase-1 constraints.

## Scope
- In:
  - Rust backend port for Hub + R3AKT domain behavior.
  - Full functional parity for only the operations classified as `client` in `docs/plan/APIANnalysis.md`.
  - Southbound command/query/result/event exchange over Reticulum/LXMF.
  - Northbound typed plugin/event bridge for Vue (no feature REST calls).
  - Reuse selected Vue domain/store logic from Reticulum Community Hub UI where compatible.
  - Use `docs/R3AKT_client_CODEX_Implementation_plan.md` as the execution-sequencing source during implementation.
- Out:
  - REST endpoint parity as a runtime requirement.
  - Any operations classified as `server-only` in `docs/plan/APIANnalysis.md`.
  - Any operations classified as `unknown` in `docs/plan/APIANnalysis.md` unless explicitly re-approved later.
  - TAK bridge parity in phase 1.
  - Data migration from legacy Python Hub database (fresh-start cutover).

## API Definition Strategy (Non-REST)
- Keep `API/ReticulumCommunityHub-OAS.yaml` as a schema vocabulary and operation reference.
- Add a canonical message catalog: `API/ReticulumCommunityHub-Messages.yaml`.
- Use `docs/plan/APIANnalysis.md` as the allowlist source for implementation.
- Only for OAS operations classified as `client`:
  - `GET` -> `Query` message.
  - `POST/PUT/PATCH/DELETE` -> `Command` message.
  - WS stream definitions -> `Event` message families.
- Do not implement `server-only` or `unknown` operations in phase 1.

## Canonical Envelope
All internal, northbound, and southbound messages must share a common envelope:
- `api_version`
- `message_id`
- `correlation_id` (required on replies and errors)
- `kind` (`command | query | result | event | error`)
- `type` (message type identifier)
- `issuer` (`ui | reticulum | internal`)
- `issued_at`
- `payload`

## Transport Architecture
### Southbound (Backend <-> Mesh)
- Internal command/query/event bus -> Reticulum adapter -> LXMF wire messages.
- Inbound LXMF -> decode/validate -> internal bus -> domain services.

### Northbound (Backend -> Vue App)
- No feature REST endpoints.
- Vue communicates through a typed TS client package and plugin bridge, matching the current emergency app model.
- Store updates are event-driven (status, telemetry, chat, mission/checklist changes, map updates).

## Public Interfaces and Types
### Rust-side interfaces
- `HubCommandBus`
- `HubQueryBus`
- `HubEventBus`
- `ReticulumSouthboundAdapter`
- `NorthboundBridge` (plugin-facing)

### TypeScript client package
- `@r3akt/hub-client`
- Typed methods for command/query operations.
- Typed event subscriptions for system, telemetry, message, and R3AKT domain events.

### Vue integration contract
- Pinia stores consume typed operations such as:
  - `listMissions`, `upsertMission`, `deleteMission`
  - `listChecklists`, `setChecklistTaskStatus`, `setChecklistCell`
  - `listMarkers`, `createMarker`, `updateMarkerPosition`
  - `listZones`, `createZone`, `updateZone`
- Stores subscribe to:
  - `system.status`, `system.event`
  - `telemetry.update`
  - `message.receive`, `message.sent`
  - `mission.change`, `checklist.change`

## Implementation Phases
1. Fork and Monorepo Bootstrap
- Create new repo and workspace layout:
  - `crates/hub-contract`
  - `crates/hub-core`
  - `crates/hub-reticulum`
  - `crates/hub-bridge`
  - `packages/hub-client`
  - `apps/mobile`
- Add CI for Rust workspace + TS package + mobile app.
- Align actual implementation order with `docs/R3AKT_client_CODEX_Implementation_plan.md`.

2. Contract Freeze
- Generate operation inventory from OAS, then filter it through `docs/plan/APIANnalysis.md`.
- Map only `client` operations to message types in `ReticulumCommunityHub-Messages.yaml`.
- Define stable naming for all message `type` IDs.
- Freeze envelope fields and versioning policy.

3. Core Message Layer
- Implement envelope, validators, serializers, and compatibility guards in `hub-contract`.
- Implement in-process command/query/event buses in `hub-core`.

4. Basic Domain Port
- Port foundational features to Rust message handlers:
  - status/events/help/examples
  - client directory listing and join/leave flows
  - client-readable topic operations only (`list`, `get`, `subscribe`)
  - file/image metadata and raw content handling
  - chat/message flows
  - markers/zones
  - app info and telemetry/system streams
- Explicitly exclude:
  - subscriber CRUD
  - topic create/update/delete/associate
  - identity moderation
  - config/control/runtime administration

5. R3AKT Domain Port
- Port R3AKT mission/checklist ecosystem:
  - missions, mission changes, logs
  - teams, members, assets, skills
  - task skill requirements, assignments
  - snapshots/events/capabilities

6. Reticulum Southbound Adapter
- Implement command/query/event bridging over LXMF.
- Enforce correlation IDs, retries, timeout/error normalization.

7. Northbound Plugin/Event Bridge
- Implement plugin-facing bridge API consumed by `@r3akt/hub-client`.
- Support request/reply operations and push event subscriptions.

8. Mobile Client Expansion
- Integrate `@r3akt/hub-client` into Vue app.
- Migrate and expand stores/views in stages:
  - connect/status/dashboard
  - missions/checklists
  - map/markers/zones
  - team/assets/assignments
  - chat/files

9. Hardening and Release
- Add observability (structured logs + message metrics + event throughput).
- Run end-to-end parity tests.
- Ship with fresh-start cutover runbook.

## Validation and Testing
### Contract tests
- Every `client`-classified operation has a valid message definition and schema.
- No `server-only` or `unknown` operation is exposed through the phase-1 message catalog.

### Serialization tests
- Envelope and payload round-trip tests for all message families.

### Southbound tests
- Internal -> LXMF mapping and LXMF -> internal mapping.
- Correlation ID propagation and timeout/error paths.

### Domain tests
- CRUD and link/unlink behavior only for entities and actions required by `client`-classified operations.

### Northbound integration tests
- Plugin/client methods return correct typed results.
- Event subscription and delivery behavior into Vue stores.

### End-to-end tests
- Mission lifecycle
- Checklist lifecycle
- Map marker/zone workflows
- Chat and file/image workflows
- Telemetry update propagation

## Risks and Mitigations
- Risk: message-contract drift across layers.
  - Mitigation: single source `ReticulumCommunityHub-Messages.yaml` + generated TS/Rust types.
- Risk: parity gaps during migration.
  - Mitigation: phase gates by operation family with required test pass.
- Risk: event ordering and consistency issues.
  - Mitigation: explicit sequencing metadata and idempotent handlers.

## Assumptions
- Rust backend is mandatory in v1.
- REST is not the feature API.
- Southbound transport is Reticulum/LXMF only.
- Northbound follows the current emergency mobile app plugin/event integration model.
- `docs/plan/APIANnalysis.md` is the authoritative inclusion filter for phase 1.
- `docs/R3AKT_client_CODEX_Implementation_plan.md` is the primary execution plan for Codex/engineering work.
- Phase 1 is fresh-start data cutover (no migration tool in scope).
