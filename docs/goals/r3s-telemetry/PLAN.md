# R3S/1 origin sensor telemetry implementation

**Intent:** Execute [the R3S/1 specification](../../plans/R3AKT_Sensor_Telemetry_Extension_v1.md) across the shared RCH contract, REM, RCH and the existing TAK adapter.
**Current Behavior (baseline):** Standard positions and marker streams exist; sensor geometry, remote-origin ownership and the plugin 1.4 operations were absent.
**Expected Outcome:** Authorized plugins publish independent origin fixes and up to four sensors; authenticated REM/RCH receivers retain their source identity and timestamps and render coherent overlays without moving the controller.
**Target-Perspective Output:** A synthetic camera on an independently identified moving object produces a north-pointing 20-degree sector at that object on both maps, survives restart, and cannot be attached or republished by an unauthorized plugin or writer.
**Truth Owner:** `r3akt-map-core` owns the checked R3S codec/model, geometry and storage primitives; native REM/RCH own authorization, lifecycle, freshness and publication. UI state only projects those results.
**Contract Boundary:** Provisional Telemeter SID 128 version 1; existing LXMF fields 2/3/13; signed Binder plugin API 1.4; native JSON projections through the current bridge; existing RCH transport and TAK adapter.
**Cutover:** Extend existing Telemeter and plugin dispatch paths. Keep standard sensor 2, scalar `sensor.publish`, T1/collector/ACK behavior and controller observer fixes unchanged.
**Displaced Path:** None: this adds an optional sensor component to the established origin, rather than a second marker, transport, position store or map engine.
**Value Density:** One shared codec and geometry implementation, consumed by both applications; existing authorization and native persistence/publication mechanisms remain authoritative.
**Acceptance Evidence:** T01–T11 deterministic fixtures, real native persistence and transport tests, plugin trust/revocation tests, and rendered REM/RCH map evidence. T12 physical device/radio acceptance remains explicitly separate from software evidence.
**Evidence Lane:** Rust unit/integration tests, JNI/plugin fixture tests, TypeScript client tests and browser evidence.
**Kill Criteria:** No unbound remote-origin claims, controller/drone identity conflation, frontend protocol/domain implementation, silent removal of standard telemetry, credentials in manifests, unbounded pose/queue state, terrain-footprint claim, automatic stream playback or SID-standardization claim.
**Forbidden Moves:** No changes to LXMF-rs protocol, RNS framing/cryptography, unrelated local work, source-size limits, production deployment, or release publication.
**Architecture Slice:** Shared contract → authenticated native origin store → coalesced existing delivery lane → separate native projection → existing MapLibre map. CoT stays in `r3akt-tak-connector`.
**Plan Review Gate:** PRE architecture/trust review before source implementation, then POST alignment/correctness, maintainability and verification gates.

## Binding and freshness decisions

- An independent origin must already have a canonical marker and an authenticated persisted writer binding. A field-3 tuple or manifest cannot claim an arbitrary existing object. Existing legacy objects without a verified writer require explicit native authorization; no trust is inferred from the unvalidated `origin_rch` field.
- Local plugins use the existing plugin-to-marker ownership table. Geometry publication and origin-position publication are separate opt-in capabilities. Network sharing additionally requires the existing LXMF send grant and current recipient policy, rechecked for retries and reconnection.
- Establish remote writer bindings through the authenticated marker creation/authorized mission path; preserve them across restart. Accept relay assertions only from the currently authorized hub under the existing mission/team policy. Revocation or origin deletion stops queued work; source-time tombstones prevent resurrection.
- All source times remain source times. Reject values outside the representable timestamp range and samples more than 30 seconds in the future; older samples may be retained as stale but cannot replace newer state. Equal-time updates require identical content. Bound state to four sensors per origin, 512 encoded extension bytes and the existing outer parser limits.
- Use the configured telemetry stale interval for sensor TTL and a 2-second position/pose skew threshold for moving origins. Suppress misleading precise geometry for stale, unsynchronized, missing-position or near-nadir samples. Metadata and approved non-secret endpoint references travel separately from pose snapshots.
- The shared codec remains Git-pinned in REM after the shared RCH change is committed and pushed. Temporary Cargo path overrides are validation-only and cannot remain in the final integration.

### PRE review resolutions

- Bootstrap remote bindings only when a new marker is created through an authenticated source or authorized hub assertion; pass the authenticated sender into the REM marker receive path. Existing unbound markers cannot be claimed by a pose/manifest. Tombstoned identifiers cannot be reused.
- Self origins have no marker requirement. A geometry-granted API 1.4 plugin can register a self sensor, but its plugin ID and trusted publisher fingerprint own that sensor; another plugin cannot change/remove it. Disabling/uninstalling/re-signing a plugin invalidates its pending network work.
- Add separate operator-granted `sensorGeometryPublish`, `originTelemetryPublish`, `sensorGeometryShare`, and `sensorVideoShare` capabilities. Existing LXMF send and recipient/privacy checks still apply to network disclosure. A plugin-supplied flag cannot approve its own endpoint.
- Add typed field-13 events: `mission.registry.origin.telemetry.upsert` carries `{originId, position}` with nullable altitude/datum and exact fix milliseconds; `mission.registry.sensor_manifest.upsert` carries `{originId, manifest, updatedAtMs}`; `mission.registry.sensor.remove` carries `{originId, sensorId, deletedAtMs}`; `mission.registry.origin.remove` carries `{originId, deletedAtMs}`. Validate source identity and origin association before applying any event.
- The origin-fix event is canonical even when sensor 2 is present. Verify matching coordinates/source seconds after standard wire rounding. Omit sensor 2 if any required standard location slot is unknown; do not invent zero altitude, speed, course or accuracy. Sensor-only packets retain the previous fix.
- Sensor removal uses trusted host time strictly greater than the last accepted sample. Persist tombstones; snapshot omission is not deletion. Origin deletion invalidates writer binding and pending traffic and cannot be undone by delayed marker replicas.
- Native projections recompute TTL, moving-origin skew and geometry using saved telemetry settings on reads. Idle UI refresh requests keep displays current without moving these policies into JavaScript.

## Ordered task board

Implementation stays in the main agent. Read-only exploration and review may run independently.

| Task | Owner / input | Allowed files | Output and evidence | Dependency / parallel safety |
|---|---|---|---|---|
| 1. Shared contract | Main; R3S/1 sections 3–5/7 | RCH `crates/r3akt-map-core/src/{sensor*,telemetry_encode.rs,lib.rs}` and matching tests/fixtures | Checked model, deterministic SID codec, moving-origin stream, bounded geodesic projection and monotonic persisted state; T02–T05/T07–T10 | First; sequential |
| 2. REM native integration | Main; shared contract | REM `crates/reticulum_mobile/src/{app_state*,node*,runtime*,telemeter.rs,types*,jni_bridge*,reticulum_mobile.udl}` | Authorized origin binding, coalesced field-2/3 send/receive, deletion, retry privacy and JSON projection; native T04/T06–T11 | 1; sequential |
| 3. Plugin 1.4 and client | Main; native operations | REM `packages/android-plugin-sdk`, Android plugin host/fixture/tests, `packages/node-client`, capability settings and plugin docs | Explicit new grants, signed Binder dispatch, typed bridge/client operations, fixtures and compatible old plugins | 2; sequential |
| 4. RCH and CoT integration | Main; shared/native contract | RCH transport, core/server broker/collector/API, TAK connector and tests | Separate authenticated object samples, durable original-byte forwarding, native projections, typed manifest and CoT adapters; T01/T04–T06/T10–T11 | 1–2; sequential |
| 5. Maps and evidence | Main; native projections | REM focused map component/store/client and e2e; RCH focused map overlay/store/types/tests | Same-map bounded overlays, per-origin/sensor visibility, origin details and lifecycle; rendered T06–T10 | 2–4; sequential |
| 6. Integrate and review | Main plus read-only review roles | Exact changed files, REM shared Git pin/lock, execution/spec status and architecture/plugin docs | Reproducible dependency integration, required checks, POST/correctness/maintainer/verifier results and honest acceptance limits | 1–5; sequential |

All tasks forbid unrelated dependency-baseline edits, generated hand edits, alternate telemetry stacks, source-size bypasses and other repositories' unrelated changes.

## Progress

- [x] Read source specification and mapped existing owners with a read-only explorer.
- [x] PRE review and material findings resolved in the explicit contract amendments above.
- [x] Shared contract.
- [x] REM native integration.
- [x] Plugin API and client.
- [x] RCH and CoT integration.
- [x] Both map projections and software acceptance evidence.
- [x] Reproducible shared pin, final reviews and verification.

## Integration decisions and release boundary

The implemented field-13 contract includes authorized `mission.registry.sensor_origin.assert` and optional `mission.registry.sensor.expiry` events documented in the specification. Bounds are 1 MiB outer fields, 1,024 ordinary object tuples, 16 sensor-bearing origins, 2,304 sidecar events, four live sensors and 512 original extension bytes. Collector replies materialize complete accepted state and send one origin per R3S packet.

Static metadata uses delivery confirmation, current route digests and explicit retry ownership. Connected-mode R3S hub admission requires current plugin/share/team authorization and exact native fields; existing ordinary controller position restrictions remain in place. Failed native map refreshes suspend cached coverage. CoT sensor imports use only the source-time position update, and export revisions include static metadata/expiry.

See [EVIDENCE.md](EVIDENCE.md) for the published shared pin, commands, synthetic renders and review outcomes. The pinned Sideband class was executed to verify repacking loss of SID 128 with ordinary positions preserved. Software implementation does not close SID allocation, physical T12, live Sideband routing, high-rate RCH radio/cadence qualification, or the daemon's post-admission retry cancellation gate. No release or deployment is authorized or claimed.
