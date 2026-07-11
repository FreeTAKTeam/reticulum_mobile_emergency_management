# REM BLE Heart Rate plugin

This is the first public REM Android plugin. It runs in its own APK/process and consumes the
versioned `rem-plugin-sdk` AAR. REM core contains no BLE-heart-rate-specific behavior.

The implementation adapts the standard Heart Rate Service work by Giu Platania from
[PR #128](https://github.com/FreeTAKTeam/reticulum_mobile_emergency_management/pull/128)
(`a9ce816e8034640b4b660289f145b8c778662eeb` and
`6f9f51f4d52856449969cf1e030c6c736b5f3512`) to the APK/Binder plugin architecture.

## Behavior

- Bluetooth Heart Rate Service `0x180D`, measurement `0x2A37`, and CCCD `0x2902`.
- Companion Device Manager on Android 8+; filtered scan fallback on Android 6–7.1; cached
  bonded Heart Rate devices are accepted.
- The plugin owns permission, pairing, GATT monitoring, configuration, and optional LXMF
  sharing. Sharing defaults to off.
- Normalized `heart_rate_bpm` values are published to REM no more than once per second.
- The configuration UI is loaded dynamically from this APK by REM's isolated offline WebView.

## Build and signing

From `apps/mobile/android`:

```sh
bash ./gradlew :ble-heart-rate-plugin:testDebugUnitTest :ble-heart-rate-plugin:assembleDebug \
  -PremHostFingerprints=<REM_SIGNING_CERT_SHA256>
```

Release builds require `-PremHostFingerprints` and the ignored
`apps/mobile/android/ble-heart-rate-keystore.properties` file:

```properties
storeFile=/absolute/path/to/plugin.keystore
storePassword=...
keyAlias=...
keyPassword=...
```

Install the resulting APK with ADB or an Android file manager. REM discovers it but never
requests package-install permission. Approve its publisher, grant only the required REM host
capabilities, enable it, and use **Configure** to pair the sensor.
