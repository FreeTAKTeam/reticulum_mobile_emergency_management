# T-Rex radar adapter

Watch Status plugin 1.1.3 provides a read-only radar endpoint for the REM Sync T-Rex 2 app, including nullable observer accuracy. Its existing `/info.json`, `/northbound/watch/status` and `/health` routes retain their payloads. Radar data is available only through the authenticated route below. Nothing is bound outside `127.0.0.1`.

## Enable and pair

The plugin declares API 1.3 and requires operator grants for `operationalRead`, `mapObserverRead` and `mapNearbyRead`. Publisher trust and the configured REM host certificate allowlist remain enforced by the Android plugin SDK. A debug build with an empty `remHostFingerprints` value compiles but cannot bind to REM; this is intentional fail-closed behavior.

In the plugin's trusted configuration page, enable the server and **Allow radar access on this phone**, then save. Copy the 64-character radar pairing code to REM Sync's settings in Zepp on the same phone, enable Connect radar and enter the port (29863 by default). The phone Side Service retains this code; the watch does not receive it.

Each process-local session is read-only, randomly generated with 256 bits of entropy, and expires after eight hours using Android's elapsed clock. Disable, save, renewal, host stop/restart and permission denial invalidate it, including in-flight reads. Renew and pair again after expiry or restart. The public snapshot session ID is independent of the bearer secret.

The trusted configuration page renews through the host-supported envelope `{type:"action",action:"radar.renew"}`. Version 1.1.3 replaces the unsupported custom `renewRadar` message type, which the host never forwarded and left the page waiting. The host bridge allowlist and permissions are unchanged. Phone testing confirmed Renew completes and shows a 480-minute lifetime.

The host currently enforces changed grants on the next operation and does not proactively notify this plugin of a grant-only change. Every radar success therefore depends on fresh authorized host operations; no radar location cache is served. `PermissionDenied` revokes the session. The phone/watch path must clear contacts for authentication/configuration denial, while temporary transport failures may retain a visibly aged snapshot.

## HTTP contract v1

Send `GET /northbound/watch/radar` with `Authorization: Bearer <pairing-code>` and `X-REM-Radar-Request: <request-id>`. IDs contain 1–64 ASCII letters, digits, underscores or hyphens. Credentials are never query parameters. No CORS permission is provided. Responses include `Cache-Control: no-store`.

Success is one full JSON snapshot, not a delta:

```json
{
  "protocolVersion": 1,
  "requestId": "watch-request-1",
  "snapshotId": "public-session:1",
  "sessionId": "public-session",
  "sequence": 1,
  "generatedAtUnixMs": 1792493152000,
  "observer": {
    "positionValid": true,
    "positionUpdatedAtUnixMs": 1792493092000,
    "accuracyM": 12.5,
    "bearingReference": "true"
  },
  "markers": [{
    "id": "stable-marker-identity",
    "label": "ALPHA",
    "color": 6602495,
    "distanceM": 111.195,
    "bearingDegTrue": 90,
    "markerUpdatedAtUnixMs": null
  }],
  "truncated": false
}
```

Color is a 24-bit integer. Watch Status 1.1.2 resolves REM's four canonical NATO catalog symbol IDs to the radar palette: `friendly` blue `#007aff`, `hostile` red `#ff303b`, `neutral` green `#00d43b`, and `unknown` yellow `#ffe600`. Generic and unrecognized symbols retain the REM pin fill `#64beff`; names, categories and arbitrary symbol substrings never imply affiliation. This corrects the 1.1.1 adapter's uniform-blue projection, as requested during watch testing. The watch preserves the resolved color, including while disconnected or stale.

Each record has a stable ID (at most 128 UTF-16 units), label bounded to 32 Unicode code points with controls/formatting characters removed, finite nonnegative distance, true bearing in `[0, 360)`, and the original marker update timestamp or null. Bearings are null within 0.5 m of the observer. Source and observer times are never replaced with HTTP receipt time. Notes and coordinates are not exported.

The native source is `map.observer.snapshot` → `map.markers.nearby {radiusM:500,limit:50}` → `map.observer.snapshot`. Observer identity, fix timestamp, coordinates and horizontal accuracy must match. `accuracyM` preserves the host's `horizontalAccuracyM`: finite and nonnegative when known, otherwise null. Geometry is recomputed from returned marker coordinates using the host's mean-Earth radius, filtered to 500 m and sorted by distance then ID. A full 50-item source result sets `truncated:true` to indicate a source limit; a larger total is not asserted. An unavailable position produces `positionValid:false` and no contacts. This is a bounded best-effort read because the host does not provide an atomic combined observer/nearby operation.

Errors use `{protocolVersion:1,requestId,ok:false,error:{code}}`. Authentication failure is HTTP 401; disabled, expired or revoked access is 403; one-active-read/rate limits are 429; source timeout, movement during sampling and invalid source data are 503. Treat 401/403 as a cache-clearing terminal denial. Each host request has a fixed 1.5-second deadline; at most four host requests are pending across status and radar. Radar allows one read and a minimum 500 ms interval. HTTP uses four workers plus four queued sockets, a two-second/8 KiB header budget, 2 KiB line limit and a 64 KiB JSON ceiling. Stop closes the listener, active/queued connections and pending host requests.

## Build and validate

From `apps/mobile/android`:

```powershell
.\gradlew.bat -PremHostFingerprints=<actual-REM-signer-SHA256> :watch-status-plugin:testDebugUnitTest :watch-status-plugin:assembleDebug
```

Release builds additionally require the existing ignored `watch-status-keystore.properties`; secrets must not be committed. The APK is under `plugins/watch-status/android/build/outputs/apk/debug/`. Native tests emit a controlled 50-marker artifact under `build/outputs/radar-contract/snapshot.json`. In the watch repository, `node scripts/verify-radar-source.mjs <absolute-artifact-path>` validates that projection through the phone decoder and actual fragmented MessageBuilder transport.

From the repository root, `node --test plugins/watch-status/test/config.test.cjs` checks the real configuration UI's Renew button against the host's accepted message types and the resulting configuration response. This supplements the 19 native unit tests.

Local unit/contract tests and package builds do not prove installation, live REM grants, phone loopback access from Zepp, Bluetooth delivery while locked, heading reference/calibration, native clock availability, firmware touch/buttons or battery use. Those remain physical-device acceptance gates for issue #2.
