# Runtime Module Boundaries

REM keeps public TypeScript exports, the NodeStore facade, native bridge
signatures, LXMF wire payloads, and persisted records stable while dividing
implementation work by domain. New behavior should extend the owning module
instead of rebuilding protocol behavior in another layer.

## Rust runtime

- `runtime/command_loop.rs` classifies incoming commands and dispatches work.
- `runtime/command_executor.rs` owns bounded normal, control, local, and
  priority execution lanes.
- Delivery, propagation, peer routing, events, telemetry, persistence, and
  tracking live in focused `runtime`, `node`, `app_state`, and
  `runtime_projection` modules.
- SOS, acknowledgements, stop, and lifecycle work use reserved priority
  capacity. Normal queue saturation cannot consume that capacity.
- Potentially slow Reticulum and LXMF work runs outside the central command
  consumer and uses explicit retry budgets or timeouts.

The compiled Rust and LXMF-rs layer remains the only implementation of LXMF
encoding, delivery tracking, and Reticulum transport behavior.

## Native Android boundary

The Java plugin and service retain their existing Capacitor/JNI contract. Their
implementation is split into lifecycle, bridge dispatch, polling, restoration,
notification, conversion, RNode, and plugin API components. Generated UniFFI
bindings and packaged native libraries are not hand-edited.

## TypeScript client boundary

`packages/node-client/src/index.ts` preserves the package import surface.
Contracts and converters are separated by domain, while platform behavior is
owned by:

- `capacitor-client.ts` and `capacitor-projection-client.ts` for native mobile;
- `web-client.ts` for browser builds;
- `mock-client.ts` for deterministic development and tests;
- `in-memory-projection-client.ts` for shared browser/mock projection storage.

Web builds resolve the package root to `web-entry.ts`, so native Capacitor and
mock implementations are not eagerly included in the production web bundle.

## Vue application boundary

`nodeStore.ts` remains the consumer facade. Connection, lifecycle, peer,
announcement, projection, telemetry, settings, logging, and transport behavior
is implemented in focused controllers. Messaging models and projections are
similarly separated from the store facade.

Route views orchestrate focused components and composables. Domain state stays
in Pinia stores and utilities; views do not reproduce wire formats or native
transport rules. Projection invalidations are keyed and coalesced, and obsolete
refreshes do not create unbounded reactive loops.
Large operational collections render through a shared 200-row window. Filter
changes reset the window, selected conversations remain visible, and paging
controls preserve access to the complete collection.

## Enforced invariants

- `npm run check:source-size` rejects any first-party source/test file or class
  above 500 physical lines. Both allowlists are empty.
- Local projection work has a p95 budget of 500 ms at the defined scale matrix.
- Critical telemetry clustering has a 50 ms long-task ceiling in unit tests.
- Large operational lists render at most 200 rows per window.
- Message-thread storage queries use the additive
  `idx_messages_conversation_updated` index, verified by a query-plan test.
- UI actions provide immediate local feedback while network completion remains
  asynchronous.
- Queues are bounded; saturation returns a typed timeout instead of growing
  without limit.
- Priority SOS and acknowledgement work remains available when normal work is
  saturated.

The scale matrix is executable in `packages/node-client/src/scale.test.ts` and
the runtime priority tests under `crates/reticulum_mobile/src/runtime/tests`.
