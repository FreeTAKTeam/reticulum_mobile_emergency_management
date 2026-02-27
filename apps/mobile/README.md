# Reticulum Mobile Emergency Management App

Mobile emergency operations client built with Vue 3 + Capacitor and backed by a
Reticulum node runtime (`@reticulum/node-client`).

The app is designed for field-team coordination over mesh-style connectivity,
including peer discovery, replicated status messaging, and incident/event tracking.

## Runtime profiles

The app ships with separate Vite profiles so browser and mobile-native workflows stay isolated.

- `web` profile (`VITE_RUNTIME_PROFILE=web`)
  - Browser-safe workflow with a web runtime client
  - Node controls remain available for browser testing
- `mobile` profile (`VITE_RUNTIME_PROFILE=mobile`)
  - Native Capacitor runtime workflow
  - Full native Reticulum runtime (start/stop/restart, connect/disconnect, hub refresh)

## What you can do with this app

1. Run and control a local Reticulum node
   - Start, stop, and restart the node from the UI
   - Recreate the client runtime without restarting the app
   - Configure announce interval, announce capability string, TCP interfaces, and broadcast
2. Discover and manage peers
   - View peers discovered from announces, hub directory, and imported lists
   - Save/unsave peers locally (allowlist model; discoveries are not auto-saved)
   - Connect/disconnect individual peers, or connect/disconnect all saved peers
   - Label peers and filter by destination/label/capability data
3. Exchange peer allowlists (PeerListV1)
   - Export saved peers as JSON
   - Share on-device (native share sheet) or copy/download on web
   - Import lists in `merge` or `replace` mode
4. Manage Emergency Action Messages
   - Create/update/delete callsign-based status cards
   - Track Security, Capability, Preparedness, Medical, Mobility, and Comms states
   - Replicate updates across connected peers
5. Manage Events
   - Create/delete incident/event timeline entries
   - Replicate event updates across connected peers
6. Monitor dashboard metrics
   - View readiness widgets and peer counts (discovered/saved/connected)

## Data behavior

- Messages, events, settings, and saved peers persist in local storage on-device.
- No demo records are auto-seeded. Messages and events only come from local/user input or peer replication.
- Replication uses JSON payloads exchanged over the node packet channel.

## Local development

From repo root:

1. Install dependencies (once): `npm install`
2. Start browser-safe dev server: `npm run web:dev`
3. Start mobile profile dev server (for native workflow): `npm run mobile:dev`

Or from `apps/mobile` directly:

- `npm run dev:web`
- `npm run dev:mobile`

## Build and native sync

From repo root:

1. Build browser profile assets: `npm run web:build`
2. Build mobile profile assets: `npm run mobile:build`
3. Sync Capacitor platforms with mobile build output: `npm --workspace apps/mobile run sync`

Open native projects:

- Android: `npm --workspace apps/mobile run android`
- iOS: `npm --workspace apps/mobile run ios`

## Android production build (signed)

1. Build and sync web assets:
   - From repo root: `npm --workspace apps/mobile run build`
   - From `apps/mobile`: `npx cap sync android`
2. Build signed Android release artifacts:
   - From `apps/mobile/android`: `cmd /c gradlew.bat assembleRelease bundleRelease`

Outputs:

- APK: `apps/mobile/android/app/build/outputs/apk/release/<app-name>-v<versionName>-release.apk`
- AAB (default): `apps/mobile/android/app/build/outputs/bundle/release/app-release.aab`
- AAB (renamed copy): `apps/mobile/android/app/build/outputs/bundle/release/<app-name>-v<versionName>-release.aab`

Release builds auto-generate:

- `versionCode` from UTC timestamp (`yyDDDHHmm`)
- `versionName` as `1.0.<yyyyMMddHHmmss>`
- artifact file names containing the application name and version

## Local signing config

Release signing is loaded from `apps/mobile/android/keystore.properties` (ignored by git).

Expected keys in `keystore.properties`:

- `storeFile` (example: `keystore/reticulum-mobile-release.jks`)
- `storePassword`
- `keyAlias`
- `keyPassword`
