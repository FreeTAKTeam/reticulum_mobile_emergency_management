# R3AKT Sensor Geometry Telemetry Extension (R3S/1)

**Status:** Proposed implementation specification, 31 October 2026. Not implemented or field-qualified.

**Target:** REM Android/Rust, shared `r3akt-map-core`, RCH reception and map, and external R3AKT drone/camera applications.

**Decision:** Reuse ATAK/MITRE sensor geometry semantics and existing LXMF/Sideband Telemeter transport. Attach sensor state to a stable *origin object*; do not create a detached, independently positioned FOV marker, and do not transmit video over LoRa.

## 1. Source findings and boundaries

### 1.1 Verified ATAK-CIV behavior

Reviewed source:

- [MITRE CoT Sensor Schema v0.6](https://github.com/TAK-Product-Center/atak-civ/blob/main/takcot/mitre/CoT%20Sensor%20Schema%20%20(PUBLIC%20RELEASE).xsd): source semantics and validation ranges.
- [SensorDetailHandler.java](https://github.com/TAK-Product-Center/atak-civ/blob/main/atak/ATAK/app/src/main/java/com/atakmap/android/cot/detail/SensorDetailHandler.java): reads/writes the `<detail><sensor .../></detail>` element, stores attributes on the parent marker, creates a FOV overlay whose UID is `<marker-uid>-fov`, follows marker movement/visibility and removes the FOV with the marker.
- [SensorFOV.java](https://github.com/TAK-Product-Center/atak-civ/blob/main/atak/ATAK/app/src/main/java/com/atakmap/android/maps/SensorFOV.java): sector overlay about azimuth ± FOV/2, arrow and optional range lines and labels.
- [VideoDetailHandler.java](https://github.com/TAK-Product-Center/atak-civ/blob/main/atak/ATAK/app/src/main/java/com/atakmap/android/video/cot/VideoDetailHandler.java): a separate `<__video>` detail associates stream information with the parent marker.
- [Video schema](https://github.com/TAK-Product-Center/atak-civ/blob/main/takcot/xsd/details/__video.xsd).

The MITRE `sensor` type has **all attributes optional**. Geometry may be received without enough data to draw a sector. It is independent of parent/platform heading: `azimuth=0` means true north. The `north` field means **where a north arrow points on the image**, not the direction the sensor is looking.

The `range` attribute is the distance in metres to the object occupying the **center pixel**, along the optical path (slant distance). ATAK's handler projects it onto a horizontal map extent using `groundRange = range * cos(elevation)`, and reverses the calculation on export. ATAK's 2D sector is **not** a terrain-intersected camera footprint.

| CoT attribute | Meaning | MITRE v0.6 constraint | R3AKT interpretation |
|---|---|---|---|
| `azimuth` | Sensor boresight clockwise from true north | 0 ≤ x < 360° | `azimuth_deg` |
| `elevation` | Boresight above/below horizon; positive up | −90 ≤ x ≤ +90° | `elevation_deg` |
| `roll` | Sensor image roll, positive right-side down | −180 < x ≤ +180° | `roll_deg` |
| `fov` | Full horizontal angle, edge to edge | 0 ≤ x < 360° | `horizontal_fov_deg`; 0 is geometrically empty |
| `vfov` | Full vertical angle, edge to edge | 0 ≤ x < 360° | `vertical_fov_deg` |
| `range` | Center-ray slant distance to target | x ≥ 0 m | `slant_range_m`; if merely a chosen display radius, flag separately |
| `north` | Direction of north arrow in sensor image | 0 ≤ x < 360° | `image_north_deg`; **not** azimuth |
| `type` | Sensor class, e.g. `r-e` | Free string | `sensor_type` metadata |
| `model` | Sensor model label | Free string | `model` metadata |
| `version` | Sensor sub-schema version | Decimal | `0.6` on CoT export; independent of R3S version |

**Uploaded sample:** parent `type="a-f-A-M-H-Q"`, origin `(38.889, -77.01399)`, `azimuth=0`, horizontal FOV `20°`, vertical FOV `60°`, elevation `0°`, slant `range=500 m`, `north=227°`, separate RTMP video link. Therefore the sector points north and spans 20°, independent of `north=227`. The sample's `hae=9999999` must be treated as unknown/sentinel, **not** as a real 9,999,999 m altitude.

### 1.2 Verified R3AKT/REM transport baseline

- [REM telemetry interoperability](https://github.com/FreeTAKTeam/reticulum_mobile_emergency_management/blob/main/docs/telemetry-interoperability.md): ordinary positions use LXMF `FIELD_TELEMETRY` **0x02** containing a binary MessagePack Telemeter map; RCH/REM share `r3akt-map-core` for checked codecs. REM native positions use m/s internally; Sideband wire speed is km/h (Columba 2.2.6 is a documented incompatible special case).
- [Shared telemetry encoder](https://github.com/FreeTAKTeam/Reticulum-Community-Hub/blob/main/crates/r3akt-map-core/src/telemetry_encode.rs): sensor 1 time; sensor 2 7-element location array; optional battery (4), information (15).
- [RCH telemetry parser](https://github.com/FreeTAKTeam/Reticulum-Community-Hub/blob/main/crates/r3akt-map-core/src/telemetry.rs): unknown numeric sensor IDs are retained in parsed data. Neither the existing standard location nor its units should change.
- [Shared marker wire](https://github.com/FreeTAKTeam/Reticulum-Community-Hub/blob/main/crates/r3akt-map-core/src/marker_wire.rs): LXMF `FIELD_TELEMETRY_STREAM` **0x03** represents an independently identified object with `[binary_object_hash, sample_time_s, packed_telemeter_bytes, appearance]`; `FIELD_EVENT` **0x0D** holds a full R3AKT marker replica.
- [REM plugin API](https://github.com/FreeTAKTeam/reticulum_mobile_emergency_management/blob/main/docs/plugins/android-plugin-system.md): signed Binder plugins, explicit grants, existing `sensor.publish`, map marker operations and owner-specific map access. **Current `sensor.publish` stores local scalar plugin readings; it does not publish this new geometry network protocol.**
- [Sideband reference `sense.py`](https://github.com/markqvist/Sideband/blob/ef572b58e88ff4e4326d51778336922a53b2a03f/sbapp/sideband/sense.py): known sensor IDs 0x01–0x1A (with gaps), `0xFF` custom. Unknown IDs are not decoded by Sideband's normal `Telemeter.from_packed` implementation; some repacking paths can discard them.

## 2. Required behavior

**R3S-01.** A sensor is a **component of an origin**, never a replacement for the origin. Origin may be (a) the authenticated REM device, (b) a fixed or mobile object with a canonical R3AKT `object_destination_hash`, or (c) a drone/camera behind a gateway. `callsign` is presentation only, not object identity. A relay's LXMF source is not automatically the origin.

**R3S-02.** Multiple sensors can attach to the same origin, identified by stable `sensor_id` within that origin. The initial release must support at least four per origin; examples: `eo-main`, `thermal`, `front`, `rear`.

**R3S-03.** Sensor orientation is absolute in geodetic WGS-84 coordinates. The producer computes true-north azimuth and elevation from platform attitude, gimbal angles and mount calibration. The REM receiver must **not add drone course/yaw again**. A fixed camera reports the same geometry without platform motion.

**R3S-04.** Each origin's current position and sensors are projected together on the map. A position update moves the FOV; a sensor update changes its aim/shape. Removing or hiding the origin removes/hides its overlays. Stale sensors are not represented as live coverage.

**R3S-05.** Sensor telemetry must remain optional. A standard Sideband/Columba/Pyxis position continues to decode and display without the extension, and a malformed sensor extension cannot invalidate otherwise valid location telemetry.

**R3S-06.** Stream endpoint metadata is separate from FOV samples. Video packets are sent over a suitable IP/video transport, not over the LoRa/LXMF telemetry path. Video playback is initiated explicitly by the user and is not an acceptance requirement for geometry telemetry.

## 3. Canonical application model

The following names are the application-facing schema (not LXMF field names). JSON uses camelCase in the plugin/UI API; Rust types use snake_case with Serde conversion.

```json
{
  "originId": "<canonical R3AKT object hash or authenticated self destination>",
  "originKind": "uav",
  "position": {
    "lat": 38.889,
    "lon": -77.01399,
    "altM": null,
    "altitudeDatum": "unknown",
    "fixAtMs": 1793440000000
  },
  "sensors": [
    {
      "sensorId": "eo-main",
      "sensorType": "r-e",
      "model": "Drone Camera",
      "azimuthDeg": 0,
      "elevationDeg": 0,
      "rollDeg": 0,
      "horizontalFovDeg": 20,
      "verticalFovDeg": 60,
      "slantRangeM": 500,
      "rangeKind": "measured-target",
      "imageNorthDeg": 227,
      "sampleAtMs": 1793440000000
    }
  ]
}
```

`position` is optional if a previously known origin position exists; no position must be fabricated. `altM` is nullable, and altitude datum may be `hae` (height above WGS84 ellipsoid), `msl` (mean sea level), `agl` (height above ground), or `unknown`. Conversion is explicit and never inferred from a 0 or sentinel value. Position age is based on `fixAtMs`.

Each `SensorState` has **required** `sensorId` (1–32 printable ASCII characters) and `sampleAtMs` (positive Unix UTC milliseconds). All angular and range values are **nullable** if unknown; drawing a sector requires valid azimuth, horizontal FOV > 0 and slant range > 0. `rangeKind` is `measured-target` (optically observed center target), `display-limit` (configured visibility radius, not measured), or `unknown` (range missing). A display limit must never be reported as observed target distance. Text model/type strings are limited to 64 bytes UTF-8 each.

Optional metadata stored with the origin/sensor manifest: a human label, `sensorType`, `model`, camera mount identifier, calibrated intrinsic parameters, and a **non-secret**, explicitly approved video descriptor (`protocol`, endpoint reference, `sensorId`). The manifest is low-frequency and independent of the sensor pose stream. Do **not** attach credentials or signed stream tokens to telemetry.

## 4. LXMF/Telemeter transport (proposed R3S/1)

### 4.1 New numeric Telemeter sensor

**Proposed provisional SID: `0x80` (128), `R3AKT_SENSOR_GEOMETRY`.** It is currently absent from the reviewed Sideband sensor ID table. It is **not globally allocated or standardized**: confirm no collision in target clients and coordinate the allocation before freezing a cross-project release. Unknown-SID forwarding and collector preservation are mandatory tests. Do not repurpose standard sensor 2 or existing `0xFF` custom sensor.

The value of numeric sensor 128 is this MessagePack **array**:

```
[1, [
  [sensorId, azCd, elCd, rollCd, hFovCd, vFovCd,
   rangeCmOrNil, imageNorthCdOrNil, sampleAtMs, rangeKind],
  ...
]]
```

- Element 0 = integer version `1`. Unsupported versions are ignored with a diagnostic, leaving standard location valid.
- Element 1 = 1–4 sensor readings, each **exactly 10 elements** in the order above. Sensor IDs are unique inside the array.
- `Cd` = **centidegrees** (integer degrees × 100), nullable where data are unavailable. `rangeCmOrNil` is a nonnegative integer in **centimetres**, or MessagePack `nil`.
- `sampleAtMs` = UTC Unix milliseconds of the actual gimbal/camera sample, independent from Telemeter sensor 1 time.
- `rangeKind` = integer `0` unknown (requires nil range), `1` measured-target, `2` display-limit (requires a present range).
- No free-form JSON/string map is nested in this fast path. Unknown optional data must go into versioned metadata, not into an unbounded pose record.

Wire example (diagrammatic MessagePack values, **not JSON encoded into LXMF**):

```
FIELD_TELEMETRY 0x02 = bin(msgpack({
  1: 1793440000,
  2: <standard Sideband 7-element location sensor>,
  128: [1, [["eo-main", 0, 0, 0, 2000, 6000,
             50000, 22700, 1793440000000, 1]]]
}))
```

An R3AKT sender may omit `2` when position is unknown, but must not substitute a fabricated coordinate, altitude, or time. A receiving client must decode unknown sensors without breaking its standard Telemeter position path.

### 4.2 Two origin cases

**Case A: sensor on the sender's own REM device.** Add `128` to standard Telemeter in LXMF `FIELD_TELEMETRY` (`0x02`). The authenticated sending LXMF destination is the origin identity. A separate `originId` inside sensor 128 is unnecessary. Do not change the existing REM T1/collector/ACK contract.

**Case B: independent origin (drone, remote camera, gateway object).** Reuse the established **object** envelope in LXMF `FIELD_TELEMETRY_STREAM` (`0x03`):

```
FIELD_TELEMETRY_STREAM 0x03 = [
  [origin_hash_bytes, sample_epoch_seconds, packed_telemeter,
   [icon_name, foreground_rgb_bytes_3, background_rgb_bytes_3]]
]
```

The packed Telemeter includes `128` and, when known, standard location sensor `2` for **that object**. `origin_hash_bytes` is the existing canonical R3AKT object identity, *not* the relay's source address and not an arbitrary label. An origin manifest/update should be independently replicated through `FIELD_EVENT` (`0x0D`), with an explicit event type such as `mission.registry.sensor_manifest.upsert`; implement it as a new typed event, not by adding unvalidated fields to current marker events. Do not send a legacy T1 upsert for the remote object, because T1 is associated with sender positions in REM.

The current `marker_wire::packed_telemeter` always encodes stationary-marker altitude, speed and bearing as zero. **Do not reuse that helper unchanged for a moving drone.** Add a shared `encode_origin_sensor_stream` helper that accepts the object's actual location/velocity and sensor extension while retaining the proven field-3 tuple and independently identified object hash.

The two cases share the same sensor-128 codec and `SensorState` model. This is transport adaptation, not two sensor protocols.

### 4.3 Security, limits and old clients

- Validate the origin's owner and its writer/plugin authorization using existing R3AKT/RCH mission rules. A gateway may publish only objects it is authorized to represent. LXMF sender identity is authenticated, but forwarded object identity needs explicit policy.
- Keep ordinary REM saved-contact/team grants, Connected/SemiAutonomous permission checks, and publication/retry admission. A plugin's sensor grant is not permission to disclose live GPS and camera information to arbitrary recipients.
- Initial maximum 4 readings per snapshot; maximum 32-byte sensor ID; maximum 512 **encoded bytes** for sensor 128; reject duplicate IDs, unexpected nested types, non-finite values, signed overflow, out-of-range angles, and invalid range/source time. The existing 1 MiB packed Telemeter parser ceiling remains the outer hard limit; production LoRa policy must be materially smaller.
- An old REM/Sideband client may display the normal parent position but **will not render the new sensor geometry**. Sideband's `Telemeter.from_packed` recognizes only known SIDs and can discard 128 if it unpacks and repacks a message. RCH collectors preserving the original packed bytes may retain it. Test each forwarding path; do not claim end-to-end backward-compatible geometry.
- `0x80` and R3S version 1 are R3AKT proposals, not ATAK/MITRE or Sideband standard values.

## 5. Geometry and map display

Render FOV as a computed overlay keyed by `(originId, sensorId)`, not as a second persisted marker. On origin movement, move all sensor origins. On sensor change, update only that overlay. Keep the original object's stable identity and application symbol.

**2D ATAK-like sector (first release):** when `azimuthDeg`, `horizontalFovDeg`, `elevationDeg`, and `slantRangeM` are available:

```
ground_extent_m = slant_range_m * cos(radians(elevation_deg))
left_bearing     = (azimuth_deg - horizontal_fov_deg/2) mod 360
right_bearing    = (azimuth_deg + horizontal_fov_deg/2) mod 360
```

Compute geodesic points at the origin for bearings from left to right at `ground_extent_m` and make a fill/outline wedge using MapLibre GeoJSON polygon + center arrow. The formula reproduces ATAK's planar projected-display rule; it is **not** a validated terrain footprint. When looking near −90° (nadir), the center-ray horizontal distance tends to zero even though a camera sees a sizeable ground rectangle. Show the boresight/target if known but **do not** present the degenerate wedge as a true visible footprint. Use a later 3D ray/terrain-intersection operation and camera intrinsics (HFOV/VFOV/roll, height and terrain) for an actual four-corner footprint and sensor point of interest (SPI).

- Boresight azimuth uses true north; `imageNorthDeg` is for camera/video image orientation, not map sector heading.
- Validate/map longitudes across the antimeridian; sample the sector arc in bounded steps. Handle FOV that crosses 0° and very wide FOV, zoom changes and map style resets.
- Display a dashed/stale overlay or remove it when `now - sampleAtMs` exceeds configured TTL; **do not reset sample time to receive time**. For moving objects, mark position/sensor skew > 2 s as unsynchronized and suppress a precise combined footprint. TTL and skew are release defaults subject to field testing.
- Unknown range: no fictitious measured radius. Show an orientation indicator with `range unknown` in details. Unknown origin position: save sensor state for association, but do not place it at `(0,0)`.
- The map should show origin type, name, active sensor(s), azimuth, elevation, FOV H/V, slant range and its kind, sensor age and optional video availability. Sensor visibility is controlled per origin and per sensor.

## 6. Application and plugin API

Use the existing REM Android plugin Binder host. Avoid a second drone-specific socket protocol. Propose **plugin API 1.4** (increment from the examined 1.3 map API):

- Capability `sensorGeometryPublish`: required to register/update geometry; separately grant publish of remote origin positions. `sensorsPublish` may remain the existing scalar sensor projection capability; do not silently repurpose its semantics.
- Operation `map.sensors.upsert` accepts `{originId, sensor: SensorState}`. It validates origin ownership, schema and source age and updates a native origin/sensor projection. Return `{accepted, originId, sensorId, sampleAtMs}` or a typed error.
- Operation `map.sensors.remove` accepts `{originId, sensorId}`; remove the sensor FOV without deleting the origin. Origin deletion cascades to its sensors.
- Operation `map.origins.telemetry.upsert` accepts `{originId, position}` for authorized drone/camera location fixes, with no changes to this REM device's own `map.observer.snapshot`.
- A low-frequency `map.sensors.manifest.upsert` attaches `sensorType`, `model`, operator-approved stream descriptor and optional mounting parameters to an existing origin; network sharing is separately authorized.
- Local plugin updates can be consumed even offline. The native REM runtime coalesces the latest state for transport, persists bounded state and enforces target privacy at enqueue **and** retry.

External apps (DJI/FreeTAKUAS integration, standalone camera app, binocular plugin) implement this contract using signed REM plugin APKs or a shared adapter hosted within their own apps. **The drone/camera app owns image capture and vendor APIs; REM owns origin identity, map state, authorization and LXMF publication.**

Integration sketch:

```
FreeTAKUAS or Camera App
  -> vendor pose, camera optics and optional video endpoint
  -> approved REM plugin: map.origins.telemetry.upsert + map.sensors.upsert
  -> REM native projection + r3akt-map-core R3S codec
  -> LXMF field 0x02 (self) or field 0x03 (independent origin)
  -> authorized REM or RCH receiver -> same shared decoder -> map FOV
```

## 7. Persistence and freshness

Create a dedicated sensor-state store keyed by `(origin_hash, sensor_id)`; retain latest sensor sample and optional bounded history only if already required by mission/event logging. Maintain origin position separately from camera boresight state. Do not merge a drone fix into the controller/operator's own `TelemetryPositionRecord`. Accept updates only when `sampleAtMs` is strictly later than stored state, or same timestamp with idempotent identical payload. Define and test explicit handling for sensor deletion, origin tombstones, restart, late relayed packets, clock skew and loss of positional fixes. Publishers should re-announce manifests after reconnection, but not rebroadcast static metadata with every gimbal movement.

Default sending policy to validate in field tests:

- Wi-Fi/TCP: publish at up to 1 Hz initially, latest state only; faster display may use direct local plugin events without radio messages.
- LoRa: motion/change-triggered telemetry with a minimum interval; as a starting point use 10–30 seconds and coalesce intermediate gimbal values. Never guarantee video-rate pose updates over low-rate LoRa.
- Large changes (e.g. > 2° azimuth, > 2° elevation, > 5% FOV or changed origin position) may trigger publication, subject to rate and permission budgets. These are **recommended tuning values**, not protocol rules.
- Preserve source timestamps and apply existing REM power saver and delivery policies.

## 8. CoT interoperability boundary

Provide **adapters**, not a second primary telemetry stack.

**CoT inbound:** parse parent `event.uid`, `event.point`, `detail.sensor`, optional `detail.__video`. Map the CoT origin UID to an authorized stable R3AKT origin. Preserve `type`, `model`, `north`, `azimuth`, `fov`, `vfov`, `roll`, `elevation`, `range`. Treat MITRE-optional values as optional. Reject invalid numeric values with a sensor diagnostic while preserving a valid parent location. Preserve CoT event time and stale time separately from receive time. An arbitrary remote CoT UID alone does **not** establish R3AKT ownership.

**CoT outbound:** for one selected sensor of an origin, write the parent point and one `<sensor version="0.6" ...>` element. Use geodetic azimuth, real sensor elevation, **slant** range and image north. Optionally attach `<__video url="..."/>` if an operator explicitly enabled a reachable and non-secret endpoint. If more than one R3AKT sensor is present, export one selected sensor or a documented separate CoT identity: do not pretend stock ATAK supports arbitrary multiple per-parent sensor overlays. Do not invent `range` when `rangeKind=unknown`.

An ATAK-compliant video link only establishes an endpoint association. It does not mean an IP stream is reachable through Reticulum or that the receiving ATAK app has the required decoder/credentials.

## 9. Work items, exact code locations

1. **Shared codec/model (RCH repository):** add `SensorState` / `SensorManifest`, strict validation and encoder/decoder for SID 0x80 in `crates/r3akt-map-core/src/telemetry.rs` and `telemetry_encode.rs`; add `encode_origin_sensor_stream` using existing field-3 tuple without using stationary marker defaults. Do not change the standard sensor 2 codec. Preserve unknown sensor bytes through the broker/collector.
2. **REM native sender/receiver:** add typed state and storage under `crates/reticulum_mobile/src/types/` and `app_state/`; extend `src/telemeter.rs`, `runtime/telemeter_receive.rs` and the transport send builders. Route self sensor 0x02 and independent-object sensor 0x03 to the same association logic. Do not treat a field-3 object as the authenticated sender's own position.
3. **REM plugin API:** extend `docs/plugins/android-plugin-system.md`, Android `rem-plugin-sdk`, the JNI/UniFFI bridge and permission checks for the proposed operations. Use current plugin ownership rules and certificate trust. Update plugin integration fixtures.
4. **REM map/UI:** add typed sensor projections to `packages/node-client`, `apps/mobile/src/stores/` and a separate MapLibre source/layer integrated with `TelemetryMapView.vue` and the map marker rendering, avoiding a new second map engine. Handle map style rebuild, zoom, origin removal, stale status and popups.
5. **RCH:** add receiving/storage/forwarding and display for object sensor snapshots. Extend `r3akt-map-core`-based views in RCH; add the explicit new event variant only when manifest replication is needed. Maintain RCH source/identity and team authorization.
6. **CoT bridge:** implement import/export in the TAK adapter (not in LXMF-rs transport). Treat CoT schemas and test samples as fixtures.

`LXMF-rs` core, RNS transport/framing and cryptography require **no protocol changes** for R3S/1. Changes are in the application data contract and consumers.

## 10. Mandatory tests and acceptance criteria

| ID | Test | Required result |
|---|---|---|
| T01 | Parse the uploaded CoT example | North-pointing 20° sector, 500 m slant, 60° vertical; image north 227°; RTMP metadata separate; unknown altitude is not used as real. |
| T02 | ATAK CoT attributes ± boundaries | 0/359.99° wrap; roll −180 invalid; elevation ±90 valid; invalid/NaN inputs do not crash or poison the origin fix. |
| T03 | Centidegree and centimetre codec | Exact deterministic MessagePack v1 roundtrip; null handling; duplicate sensor IDs rejected. |
| T04 | Sideband/Pyxis/Columba standard fixture regression | Existing `FIELD_TELEMETRY 0x02` location/units/collector requests and positions remain byte-compatible. |
| T05 | Unknown-SID compatibility | An old client keeps its position; RCH preserves the binary sensor 128 payload through store/retrieve/restart; explicitly test Sideband repacking loss. |
| T06 | Self vs independent object | Controller at A, drone at B. Sensor cone remains at B with drone origin ID before/after a relay; controller never moves to B. |
| T07 | Multiple sensors on one origin | Four sensor states render and update independently; one update leaves other three unchanged. |
| T08 | Stale / out of order / clock skew | Earlier payload cannot replace newer; stale status uses source time; origin/sensor time skew suppresses misleading combined footprint. |
| T09 | Geometry | Sector crossing 0° north and antimeridian, azimuth ≠ course, changing roll/north, near-nadir and near-horizontal behavior. |
| T10 | Origin lifecycle | Moving, hiding or deleting origin updates/removes FOV; restart restores permitted current state and owner identity. |
| T11 | Authorization and threat model | Unauthorized plugins cannot claim another origin; revocations stop queued/retried sensor traffic; URL credentials cannot leave device. |
| T12 | Physical integration | Synthetic drone app → REM → second REM and RCH, with independent origin and real BLE/TCP/LoRa constraints documented separately. |

**Release gate:** no data loss on ordinary REM/Sideband telemetry; no forged origin attachment; coherent moving-origin FOV on REM map; authenticated RCH relay; old-client degradation is limited to losing the optional sensor overlay; LoRa payload/cadence and on-device permissions have measured acceptance evidence. Source-level tests alone are not a claim of live drone, camera or radio interoperability.

## 11. Design limitations and decisions still requiring review

1. **SID allocation:** 0x80 is provisional R3AKT space, not reserved by Sideband or a global standards body. Confirm no downstream collision before publishing R3S/1.
2. **ATAK interoperability:** the static `<sensor>`/`<__video>` mechanism is verified; ATAK UAS Tool's 3D footprint, SPI and gimbal-control protocol are separate and are not claimed implemented or equivalent here.
3. **Source geometry:** center-ray slant distance and 2D display radius are not the real four-corner footprint. A next revision may add terrain elevation, complete intrinsics, SPI and measured range confidence.
4. **Current REM plugin:** `sensor.publish` is a local sensor value projection. The proposed authenticated origin-sensor operations are **new implementation work**.
5. **Non-REM clients:** old clients do not understand SID 0x80. Do not conflate retention of the original binary packet with support for rendering the extension.
6. **Video:** a `rtsp://` or `rtmp://` URL can be unreachable across LoRa; REM should not disclose local IP addresses to an untrusted recipient or automatically fetch untrusted streams.

**Assessment confidence:** ATAK semantics 0.97 (source and XSD); existing REM/RCH transport 0.95 (current repository); selected R3S/1 encoding/architecture 0.84 (proposed design awaiting client/field tests). Overall 0.90 for suitability as an implementation starting specification, not proof of live interoperability.