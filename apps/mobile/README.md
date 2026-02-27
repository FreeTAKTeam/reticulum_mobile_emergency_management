# mobile

Vue + Capacitor app shell.

## Android production build (signed)

1. Build and sync web assets:
   - From repo root: `npm --workspace apps/mobile run build`
   - From `apps/mobile`: `npx cap sync android`
2. Build signed Android release artifacts:
   - From `apps/mobile/android`: `cmd /c gradlew.bat assembleRelease bundleRelease`

Outputs:
- `apps/mobile/android/app/build/outputs/apk/release/app-release.apk`
- `apps/mobile/android/app/build/outputs/bundle/release/app-release.aab`

## Local signing config

Release signing is loaded from `apps/mobile/android/keystore.properties` (ignored by git).

Expected keys in `keystore.properties`:
- `storeFile` (example: `keystore/reticulum-mobile-release.jks`)
- `storePassword`
- `keyAlias`
- `keyPassword`
