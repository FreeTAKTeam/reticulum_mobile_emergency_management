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
bash ./gradlew :rem-plugin-sdk:assembleDebug :plugin-test-fixture:assembleDebug testDebugUnitTest
```

The fixture APK is an integration aid, not a public example. BLE Heart Rate and Watch Status Server
are supported plugins built and distributed as separate APK artifacts; neither is embedded or
automatically installed by REM.
