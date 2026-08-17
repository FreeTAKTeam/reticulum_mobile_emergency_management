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
- Direct and Auto SOS sends use the reserved recovery lane. Propagation-only
  SOS fanout uses the propagation lane so unreachable relay recipients cannot
  consume direct emergency capacity.
- RNode interface creation publishes `connecting`; only a validated startup
  probe with an online radio promotes the interface to `connected`. Failed or
  powered-off paired radios cannot make LoRa readiness green.
- REM Rust owns Bluetooth bearer selection and retry. Android performs one
  generation-scoped BLE GATT or Classic RFCOMM attempt, while LXMF-rs owns the
  shared KISS/RNode protocol session and radio validation.
- Potentially slow Reticulum and LXMF work runs outside the central command
  consumer and uses explicit retry budgets or timeouts.
- `error_context.rs` retains the causal value at category-preserving error
  conversions. It logs the owning module and makes synchronous context
  available to the native last-error envelope without changing `NodeError`.
- `numeric.rs` owns checked or deliberately saturating conversions at database,
  timestamp, counter, coordinate, timeout, and size boundaries.

The compiled Rust and LXMF-rs layer remains the only implementation of LXMF
encoding, delivery tracking, and Reticulum transport behavior.

## Native Android boundary

The Java plugin and service retain their existing Capacitor/JNI contract. Their
implementation is split into lifecycle, bridge dispatch, polling, restoration,
notification, conversion, RNode, and plugin API components. Generated UniFFI
bindings and packaged native libraries are not hand-edited.

All 85 Java exports use the local `jni-boundary-macro` attribute. A contained
panic returns `1` for integer calls or null for object calls and records a
non-retryable `InternalError`; no Rust panic crosses into the JVM.

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
All Capacitor plugin calls are proxied through the shared error classifier.
Consumers receive `ReticulumNodeError` with stable code, operation,
retryability, and cause fields while existing promise signatures remain
unchanged.

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
- Production Rust denies panic, unwrap, expect, unreachable, and first-party
  unsafe implementation code. Required JNI/UniFFI export attributes are the
  narrowly documented exception.

The scale matrix is executable in `packages/node-client/src/scale.test.ts` and
the runtime priority tests under `crates/reticulum_mobile/src/runtime/tests`.
