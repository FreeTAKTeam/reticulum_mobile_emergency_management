# Android Plugin System

REM plugins are separate Android APKs. They run under their own Android UID and process and bind to
`ReticulumNodeService` through the public `rem-plugin-sdk` AAR. REM does not download, install, or
load executable code from plugin packages.

## Discovery manifest

Export one service using the SDK base class and the v1 action:

```xml
<service android:name=".ExamplePluginService" android:exported="true">
  <intent-filter>
    <action android:name="network.reticulum.emergency.PLUGIN_V1" />
  </intent-filter>
  <meta-data android:name="rem.plugin.id" android:value="org.example.rem.plugin.sample" />
  <meta-data android:name="rem.plugin.displayName" android:value="Sample Plugin" />
  <meta-data android:name="rem.plugin.version" android:value="1.0.0" />
  <meta-data android:name="rem.plugin.apiMajor" android:value="1" />
  <meta-data android:name="rem.plugin.apiMinor" android:value="1" />
  <meta-data
    android:name="rem.plugin.capabilities"
    android:value="{&quot;sensorsPublish&quot;:true}" />
  <meta-data android:name="rem.plugin.messages" android:value="[]" />
  <meta-data
    android:name="rem.plugin.configurationEntrypoint"
    android:value="rem-plugin-config/index.html" />
</service>
```

Plugin IDs use lower-case reverse-DNS notation. REM accepts API major `1` and negotiates minor
features. The package name, service component, Android permissions, and certificate lineage come
from `PackageManager`, not from the plugin descriptor.

## Mutual trust and grants

REM shows the plugin signing-certificate SHA-256 fingerprint before binding. Approving a publisher
does not enable the plugin or grant capabilities. Certificate changes revoke the active session
unless Android proves that the new signer belongs to the trusted signing lineage.

Plugins must also validate the REM caller on every Binder call. Override
`allowedHostPackageNames()` and `allowedHostCertificateFingerprints()` in `RemPluginService`
with the REM package and release signing fingerprints accepted by that plugin. Signing keys and
keystore properties stay outside git.

REM host capabilities are `events.publish`, `sensors.publish`, `lxmf.send`, `lxmf.receive`,
`notifications.raise`, and `operational.read`. Android permissions requested by the plugin APK are
independent of these host grants. API 1.1 adds `operational.read`; its `operational.snapshot`
request returns the current node status, operational summary, EAM readiness, latest active event,
and latest telemetry position. The operation is rejected unless the plugin declares and is granted
that capability.

API 1.2 and 1.3 add the map API described in [Map API](#map-api-12-13). REM negotiates the API
minor at bind time: the session JSON passed to `start()` carries `apiMinor`, the lower of the
host's and the plugin's minor. A plugin declaring a newer minor than the host is `Incompatible`
and is never bound. Plugins check `apiMinor` before using an operation that needs a newer minor.

## Map API (1.2, 1.3)

This section is the stable contract consumed by map plugins such as the BinoX rangefinder
(`atn-rangefinder#1`). Within API major `1`, fields and operations here are only ever added;
existing names, types, and semantics do not change. Map objects are ordinary shared REM markers
handled by the same native marker service (`r3akt-map-core`) as the REM UI and replicated to
REM peers and RCH; plugins never get a second, plugin-only marker store.

### Capabilities

Declare in `rem.plugin.capabilities`; the operator grants each one in REM settings. An operation
is allowed only while the plugin is trusted, enabled, installed, compatible, and the capability is
both declared and granted. Revoking a grant takes effect on the next request.

| Capability | Minor | Allows |
| --- | --- | --- |
| `mapMarkersRead` | 2 | `map.symbols.list`, `map.markers.get`, `map.markers.list` on markers this plugin created |
| `mapMarkersWrite` | 2 | `map.symbols.list`, `map.markers.create`, and update/move/delete of markers this plugin created |
| `mapObserverRead` | 2 | `map.observer.snapshot` |
| `mapSelectionRead` | 3 | `map.session.*` and session events for objects the operator selected |
| `mapNearbyRead` | 3 | `map.markers.nearby`: every map marker near this device, without per-marker sharing (REM 1.5.2+) |

### Operations

Requests use the envelope in [Lifecycle and requests](#lifecycle-and-requests). Results are in
`result` when `ok` is `true`.

- `map.symbols.list` → `{items: [{id, set, mdi?, description, tak?, category, aliases}]}`: the
  symbol catalog shared with RCH. Use an `id` (or alias) as `symbol`.
- `map.markers.create` → `{marker, created}`. Payload: `idempotencyKey` (required, unique per
  observation), `symbol`, `lat`, `lon`, optional `name`, `category`, `markerType`, `notes`. The key is
  scoped to the plugin; replaying it returns the original marker with `created: false`. The new
  marker is a normal shared marker and replicates like one created in the UI.
- `map.markers.get` (`objectDestinationHash`), `map.markers.list` (`limit` 1–50, default 25) →
  markers this plugin created.
- `map.markers.update` (`objectDestinationHash`, `name`), `map.markers.position.update`
  (`objectDestinationHash`, `lat`, `lon`), `map.markers.delete` (`objectDestinationHash`) → own
  markers only. Any other marker, including one shared in a session, answers `NotFound`.
- `map.observer.snapshot` → `{available, observer, source, datum, lat, lon, alt,
  horizontalAccuracyM, course, speed, updatedAtMs, ageMs}`, or `{available: false, observer}`.
  `observer` is always `{kind: "localDevice", identityHex, lxmfDestinationHex}`: the fix is this REM
  device's own latest location, never a peer's telemetry and never a value a plugin supplied. Plugins
  cannot set the observer. Use `ageMs` and `horizontalAccuracyM` to reject stale or poor fixes.
- `map.markers.nearby` (`radiusM` up to 100000, `limit` 1–50, default 50) → `{available, radiusM,
  items}`: markers within `radiusM` of this device's own fix, nearest first, each with `distanceM`
  and with `notes` omitted. `available` is `false` (and `items` empty) while REM has no fix. Granting
  `mapNearbyRead` is the operator's one-time opt-in for a plugin to follow the whole map around
  them, for example to mirror it on a display device.
- `map.session.list` → `{items: [session]}`: this plugin's active sessions.
- `map.session.objects.get` (`sessionId`) → `{session, objects}`: current snapshots of exactly the
  objects the operator selected. Objects that no longer exist are omitted.
- `map.session.stop` (`sessionId`) → `{stopped: true}`. No `map.session.stopped` event follows.

A `session` is `{sessionId, pluginId, objectDestinationHashes, startedAtMs, startedBy}`. A marker
object uses the RCH marker representation (`object_destination_hash`, `origin_rch`, `type`, `name`,
`category`, `symbol`, `notes`, `position: {lat, lon}`, `time`, `stale_at`, `created_at`,
`updated_at`) plus `textTruncated`. REM truncates `name` to 128, `notes` to 1024, and other text
fields to 64 characters in plugin payloads and sets `textTruncated` when it did; the shared marker
itself is unchanged.

### Selected-object sessions (1.3)

Plugins cannot browse the map. Only the operator starts a session, from a marker's details sheet
(“Share with …”), which REM offers for running plugins granted `mapSelectionRead`. A session holds
1–16 objects; a plugin has at most 4 active sessions. Active sessions are listed on the map and in
the plugin's settings card, each with a **Stop** button.

REM delivers session events through `onHostEvent` as
`{"protocolVersion": 1, "event": "<name>", "payload": {...}}`:

| Event | Payload |
| --- | --- |
| `map.session.started` | the `session` fields plus `objects` (snapshots of the selection) |
| `map.session.object.updated` | `sessionId`, `reason`, `object` |
| `map.session.object.removed` | `sessionId`, `objectDestinationHash`; the object leaves the session |
| `map.session.stopped` | `sessionId`, `reason` |

Events concern only objects in the session. `reason` for `stopped` is `operator`,
`capability-revoked`, `plugin-disabled`, `plugin-unavailable`, or `objects-removed` (the last
selected object was deleted). Sessions also end, without an event, when the plugin's connection
ends: `stop()`, unbind, Binder death, node stop, or REM restart. Sessions are never restored; a
plugin treats every unknown `sessionId` as ended and must not cache objects past `stopped` or
`stop()`. Delivery is best effort: if REM cannot reach the plugin the event is dropped, so a plugin
that needs current state calls `map.session.objects.get`.

### Ownership and removal

REM records which plugin created each marker so the plugin can edit it later. Uninstalling,
disabling, or revoking a plugin never deletes its markers; they are shared objects that other
devices may already hold. If the same plugin ID returns signed by a different publisher, it loses
edit rights over the old markers.

### Errors and limits

| Code | Meaning |
| --- | --- |
| `PermissionDenied` | API minor too old, capability not declared or granted, plugin not enabled |
| `InvalidRequest` | missing or invalid field; `message` carries the marker service's reason |
| `NotFound` | marker not created by this plugin, or session unknown, stopped, or another plugin's |
| `ResponseTooLarge` | the response would exceed 64 KiB |
| `HostError` | unexpected REM failure; retry with backoff |

Every request, response, and event is UTF-8 JSON of at most 64 KiB
(`PluginProtocol.MAX_JSON_BYTES`). Bounds on session size and object text keep the largest session
event below that limit; plugins should still reject larger messages with
`PluginProtocol.requireJsonSize`. `PluginProtocol` provides `request(...)`, `hostEvent(...)`,
`negotiateApiMinor(...)`, and the event name constants.

[`plugins/map-fixture`](../../plugins/map-fixture/android) is the reference consumer: it creates a
marker 100 m north of the observer fix and reads only the objects shared with it.

## Lifecycle and requests

Trusted, enabled plugins bind when the REM node starts and remain bound while the foreground node
service runs, even if the Vue UI is detached. They stop when the node stops. Binder death records a
failed state and uses bounded retry.

Plugin host requests are JSON objects no larger than 64 KiB:

```json
{
  "protocolVersion": 1,
  "requestId": "uuid",
  "operation": "sensor.publish",
  "payload": {}
}
```

REM derives plugin identity from the Binder connection and overwrites any supplied `pluginId`.
Responses carry the request ID, `ok`, and either `result` or a typed `error`.
Plugins must treat a Binder exception, `ok: false`, timeout, and Binder death as
different outcomes. Retry only bounded transient failures and preserve the
request ID in diagnostics; never loop on validation or permission failures.

## Configuration web UI

Configuration assets live inside the signed plugin APK under `assets/`. REM loads the declared
entrypoint in a dedicated `PluginConfigurationActivity`, never in the Capacitor WebView. The
configuration WebView has no Capacitor bridge, network, file/content access, cookies, DOM storage,
external navigation, frames, or objects. REM serves only the selected plugin asset directory with
a host-controlled CSP.

The page receives a `WebMessagePort` named `rem-plugin-config-v1`. It may send `ready`, `getState`,
`update`, and `action`; the plugin responds with `state`, `validationError`, or `actionResult`.
Values are validated and persisted by the plugin APK. Android permission or pairing UI may be
launched by returning an `actionResult` with
`{"activity":{"className":"org.example.plugin.PairingActivity"}}`. REM resolves that explicit
component and launches it only when it belongs to the already verified plugin package.

## Sensors and LXMF

`sensor.publish` stores the latest value for `(pluginId, deviceId, sensorType)` and emits a
`PluginSensors` projection invalidation. Heart-rate samples must be integers from 1 through 240.
REM computes `Active`, `Stale`, or `Offline` from the sample timestamp, stale interval, and
connection state.

Plugin LXMF messages use `FIELD_CUSTOM_TYPE (0xFB)` with
`org.freetakteam.rem.plugin.v1` and `FIELD_CUSTOM_DATA (0xFC)` with the plugin ID, message name,
message version, and schema-validated payload. Incoming messages are delivered only when the
matching plugin is trusted, enabled, compatible, and granted `lxmf.receive`.

## Build and test

```bash
cd apps/mobile/android
bash ./gradlew :rem-plugin-sdk:assembleDebug :plugin-test-fixture:assembleDebug \
  :map-fixture-plugin:assembleDebug testDebugUnitTest
```

The fixture APKs are integration aids, not public examples. BLE Heart Rate and Watch Status Server
are supported plugins built as separate APKs. Complete offline releases carry their installers
alongside the ATN Rangefinder installer; REM does not install any of them automatically.

The example plugins log cleanup failures from scan stop, GATT disconnect/close,
socket close, and callback delivery. Cleanup remains best-effort and must not
replace the primary operation result, but exceptions are never silently
discarded. Tests cover reconnect policy, sample bounds/rate limits,
configuration validation, SDK packaging, discovery, and Binder integration.

## Offline release packages

Each offline release contains `rem-complete.apk`, `rem-light.apk`, the individually signed
`ble-heart-rate.apk`, `watch-status.apk`, and `atn-rangefinder.apk`, plus `SHA256SUMS` and
`BUILD-INFO.txt`. Transfer either the complete APK or the light APK and the desired individual
plugin APKs. Both REM APKs use the same application ID, version, and signing certificate.

On the first launch of the complete APK, REM copies its three bundled plugin installers into
`Downloads/REM Plugins`. Android 8 and 9 require the user to grant storage access for this copy.
If access is denied or copying fails, reopening REM retries. Install only the desired APKs from
that folder with the Android file manager, then refresh Settings → Plugins and approve, grant,
enable, and configure them. Android's installation confirmation and publisher trust remain in
force. REM never installs plugins automatically. Plugin names include the APK SHA-256; a new
release leaves previous installers in place. The light APK does not copy installers.

### Building the packages

Use a clean, release-ready REM checkout with Node dependencies and its pinned LXMF-rs checkout
in the location required by `crates/reticulum_mobile/Cargo.toml`. Configure the Android SDK,
NDK, Rust Android targets, REM signing, and the existing Heart Rate and Watch Status signing
properties. Point `ATN_SOURCE` to an ATN git repository and `ATN_COMMIT` to the full commit to
build; the script exports that commit to a temporary directory and does not use its working-tree
changes. `ATN_GRADLE` must point to Gradle 8.11.1. Set `ANDROID_HOME`, `ANDROID_NDK_HOME`,
`REM_VERSION`, `REM_VERSION_CODE`, `ATN_KEYSTORE`, `ATN_KEY_ALIAS`, `ATN_STORE_PASSWORD`, and
`ATN_KEY_PASSWORD` in the environment, then run:

```sh
bash tools/build-offline-release.sh
```

Keep signing passwords and keys outside version control. The script builds both bundled REM
plugins, builds the ATN Rust and Android APK using the locally built REM SDK, signs the ATN APK,
and builds both REM variants. It verifies APK signatures, plugin package IDs, the REM signer,
and equality of bundled and standalone APK bytes before writing `output/offline-release-<version>`.
It does not publish, install, or verify real hardware. Android version codes must increase for
updates, and all plugin signing keys must remain consistent across releases. REM host fingerprints
are derived from the actual signed light APK and applied to all plugin builds.

## Origin sensor telemetry (1.4)

API minor 4 adds the R3S/1 geometry operations below. The existing scalar `sensor.publish` operation and all 1.0–1.3 contracts retain their behavior. A plugin must declare minor 4, be installed, enabled and trusted under its current signing certificate, and receive explicit operator grants:

| Grant | Purpose |
|---|---|
| `sensorGeometryPublish` | Register/update/remove local sensor geometry and manifests |
| `originTelemetryPublish` | Publish an independently owned object's position |
| `sensorGeometryShare` | Share those components over existing telemetry routes |
| `sensorVideoShare` | Allow non-secret endpoint metadata to be considered for sharing |

Network sharing also requires `lxmfSend`, enabled telemetry and current recipient/team/privacy policy. Revoking the producer, publisher or any required grant invalidates pending snapshots, retries and recovered local custody. Geometry and position grants are separate; a revoked position producer cannot leak its fix through parent metadata. API 1.4 does not grant any of these capabilities to legacy plugins.

Create the independent origin with `map.markers.create` first. Its plugin ID and signing fingerprint own the origin. Legacy objects without verified plugin provenance cannot be adopted. Use the returned `object_destination_hash` as `originId`. `originId:"self"` attaches a sensor to REM's authenticated destination after the node has established its identity; another plugin cannot modify that component. Plugins cannot replace the controller observer fix.

| Operation | Payload |
|---|---|
| `map.origins.telemetry.upsert` | `{originId, position}`; independent origin only |
| `map.sensors.upsert` | `{originId, sensor}` |
| `map.sensors.manifest.upsert` | `{originId, manifest}` |
| `map.sensors.remove` | `{originId, sensorId}` |

Example position and pose (JSON numbers remain nullable when unknown):

```json
{"originId":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","position":{"lat":45,"lon":-63,"altM":null,"altitudeDatum":"unknown","fixAtMs":1793440000000,"speedMps":15,"courseDeg":90,"accuracyM":null}}
```

```json
{"originId":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","sensor":{"sensorId":"eo-main","sampleAtMs":1793440000000,"azimuthDeg":0,"elevationDeg":-30,"rollDeg":12,"horizontalFovDeg":20,"verticalFovDeg":10,"slantRangeM":2000,"rangeKind":"display-limit","imageNorthDeg":180}}
```

The host returns `{accepted,originId,sensorId,sampleAtMs}` for pose updates and `{accepted,originId}` for other accepted operations, or the existing typed error envelope (`PermissionDenied`, `InvalidRequest`, `HostError`, `UnsupportedOperation`). Native validation enforces four sensors per origin, source ordering, clock bounds, ownership and the wire limit. Omission does not delete another component. Deletion persists a source-time tombstone; deleting the parent removes all its sensors and permanently retires its identity.

Manifests use `{sensorId,label?,sensorType?,model?,mountId?,intrinsics?,video?}`. Video is `{sensorId,protocol,endpointRef}` with a non-secret RTSP/RTMP/HTTP/HTTPS reference: credentials, query tokens, fragments, escaping and control characters are rejected. The operator must also approve the exact descriptor in the native sensor panel. Approval binds to descriptor, plugin ID and signer; editing the endpoint or changing signer invalidates it. Plugins cannot approve themselves. REM does not fetch or play a stream automatically.

The native runtime coalesces latest snapshots at up to 1 Hz for TCP, no more than once per 30 seconds when RNode/unknown routes are present, or once per 300 seconds in power saver. Static metadata is confirmed by delivery receipts and is re-announced after changes or route reconnection. It is not repeated with each gimbal sample. Those cadence values require physical radio qualification.

`plugins/map-fixture` demonstrates the API 1.4 marker → nullable origin fix → manifest → pose sequence. SDK/fixture JVM tests and Java compilation cover the contract; physical Binder/radio acceptance is separate. See [the specification](../plans/R3AKT_Sensor_Telemetry_Extension_v1.md) and [execution evidence](../goals/r3s-telemetry/EVIDENCE.md).

## Communication relay (1.5)

The experimental [goTenna Mesh plugin](../../plugins/gotenna/README.md) uses the
explicit `communicationsRelay` grant. It bridges addressed chat and exposes new
received REM text, canonical MECP event codes and remote telemetry/map positions.
The operator separately enables default-off public radio forwarding. Recipient
hashes provide application addressing; radio Shouts remain unencrypted.

`relay.poll {cursor?,limit?}` (limit 1–16) returns `{items,cursor,gap}`. Omitted
cursor establishes the current head without history. Items are
`{sequence,id,kind,source,receivedAtMs,payload}` with monotonic local sequence,
stable ID, native source and local receive time. Payloads are:

- `text`: `{messageId,destinationHex,text}`; recipient comes from LXMF wire, not the conversation correspondent.
- `event`: `{uid,code}`; canonical full MECP code, such as `MECP/2/P01`.
- `position`: `{callsign,lat,lon,updatedAtMs}`.

The feed retains at most 2,048 entries, reports cursor gaps and bounds responses
below 64 KiB. Every request rechecks current trust, installation/compatibility,
enablement and declared/granted capability.

`relay.identity {}` returns `{destinationHex}` for the running native node.
`relay.chat.accept`, `relay.chat.pending` and `relay.chat.sent` take
`{id,sourceHex,destinationHex,text,radioGid}`. IDs/hashes contain 32 lowercase hex
digits and GIDs 12; text is bounded to 4,096 UTF-8 bytes. Both chat operations
require the communication-relay grant. The native host forces the local source
for outgoing admission, records an unconfirmed attempt before radio submission, and advances it to radio-accepted/unconfirmed via `sent`. It uses an
internal runtime command so normal chat caches/projections remain consistent.

`accept` routes only a matching local destination into normal conversations,
with native-forced unverified goTenna provenance. A nonlocal recipient also
requires declared/granted `lxmfSend` (otherwise `RecipientNotLocal` means ignore
this reception, not display it locally) and uses compiled LXMF under gateway
identity. Accepted result is `{accepted,messageId?,local?,duplicate?,forwardedMessageId?}`.
A prior unresolved forwarding claim returns `SubmissionUnconfirmed`; it is never
silently retried or reported as an accepted duplicate. Stable native namespaces
and durable forwarding outcomes own deduplication, not caller-selected native IDs.

`relay.event.accept` takes the same fields with `destinationHex:"-"` and `text`
containing the MECP code. It additionally requires `eventsPublish`. The native
runtime forces gateway identity/unverified display attribution, scopes imported
UIDs to plugin/reported-source/event-ID and uses normal event persistence and
replication. Responses use the existing typed error envelope. These operations
add no public UniFFI or generated-binding contract.

Received raw bytes, bounded fragment snapshots and pending admissions are only
internal custody. A separate pending-sent receipt survives native recording failures without retransmitting the radio message. Terminal `InvalidRequest` is rejected visibly and does not block later receptions. The operator reads chat in normal REM conversations and events
in the native event timeline. The APK is independently built and is not yet in
the existing offline release bundle.
