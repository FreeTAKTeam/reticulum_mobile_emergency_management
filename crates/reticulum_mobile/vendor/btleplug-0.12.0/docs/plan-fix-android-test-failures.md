# Plan: Fix Remaining Android Integration Test Failures

## Context

The Android BLE integration test infrastructure is working — 21/26 tests pass on `ble-integration-test` branch (Pixel 3a XL, API 32). There are 7 remaining failures across 3 categories.

### What's working

All test infrastructure is solid:
- Foreground service (`BleTestService.kt`) with `location|connectedDevice` type keeps permissions alive
- `GrantPermissionRule` + `pm grant` sets app-ops correctly
- Screen kept on via `svc power stayon true` prevents scan throttling
- Rust-side 55s timeout in `run_test()` catches hung tests cleanly
- `ensure_clean_state()` with timeouts prevents cleanup from blocking
- `catch_unwind` in JNI wrapper prevents panics from killing process
- All tests run to completion without process crashes

### Remaining failures

| # | Test | Failure Type | Root Cause |
|---|------|-------------|------------|
| 1 | testSubscribeAndReceiveNotifications | Assertion: service_uuid = 00000000-... | Hardcoded `Uuid::default()` in droidplug peripheral.rs:413-417, TODO in code |
| 2 | testSubscribeAndReceiveIndications | Scan timeout (intermittent) | Insufficient cooldown between BLE operations |
| 3 | testReadRssi | Scan timeout (intermittent) | Insufficient cooldown between BLE operations |
| 4 | testReadStaticValue | Scan timeout (intermittent) | Insufficient cooldown between BLE operations |
| 5 | testWriteWithResponse | Scan timeout (intermittent) | Insufficient cooldown between BLE operations |
| 6 | testReadOnlyDescriptor | Hangs forever (disabled) | `read_descriptor` future never completes — needs investigation |
| 7 | testReadWriteDescriptorRoundtrip | Hangs forever (disabled) | `write_descriptor` future never completes — needs investigation |

## Fixes

### Fix 1: Increase BLE cooldown between tests

**Files:**
- `tests/common/peripheral_finder.rs` — change `ensure_clean_state` sleep from 1s to 2s
- `tests/android/src/androidTest/.../BleIntegrationTest.kt` — change `@Before cooldown()` from 500ms to 1000ms

### Fix 2: Fix notification service_uuid in droidplug

**File:** `src/droidplug/peripheral.rs` (around line 413-417)

The `ValueNotification` is constructed with `service_uuid: Uuid::default()` and a TODO comment. The characteristic UUID is already known. Look up the service UUID from cached services.

**Approach:** After `discover_services()`, the Peripheral has services cached. In the notification handler, look up which service contains the notifying characteristic UUID and populate `service_uuid` from that.

Need to check exact Peripheral struct fields to determine simplest approach. May need a `HashMap<Uuid, Uuid>` (char→service) populated during `discover_services()`.

### Fix 3: Investigate and fix descriptor read/write hang

**Research needed first.** The descriptor read/write futures never complete on Android. Characteristic reads/writes work fine. Key files to investigate:

- `src/droidplug/peripheral.rs` — `read_descriptor` and `write_descriptor` implementations
- `src/droidplug/jni/objects.rs` — `JBluetoothGattDescriptor` JNI handling
- `src/droidplug/java/.../impl/Peripheral.java` — `readDescriptor`/`writeDescriptor` Java side + GATT callbacks (`onDescriptorRead`, `onDescriptorWrite`)

**Key questions:**
1. Are `read_descriptor`/`write_descriptor` actually implemented in droidplug, or are they stubs?
2. Does the Java side have `onDescriptorRead`/`onDescriptorWrite` GATT callbacks wired up?
3. Is the `BluetoothGattDescriptor` object being constructed correctly from the Rust `Descriptor` struct?

Once root cause is found, fix and re-enable the two disabled tests in `BleIntegrationTest.kt`.

### Fix 4: Commit and verify

Commit each fix separately, then run the full test suite:

```bash
# After install + pm grant:
adb shell svc power stayon true
adb shell input keyevent KEYCODE_WAKEUP
adb shell am instrument -w com.nonpolynomial.btleplug.test.test/androidx.test.runner.AndroidJUnitRunner
```

**Expected:** All 28 tests pass, `adb logcat -s btleplug-test` shows `[PASS]` for all.

## Key files

- `tests/common/peripheral_finder.rs` — scan/connect/cleanup logic
- `tests/android/src/androidTest/.../BleIntegrationTest.kt` — test runner
- `src/droidplug/peripheral.rs` — Android Peripheral implementation (notifications, descriptors)
- `src/droidplug/jni/objects.rs` — JNI object wrappers
- `src/droidplug/java/.../impl/Peripheral.java` — Java-side BLE implementation
- `tests/common/test_cases.rs` — shared test logic
- `tests/common/gatt_uuids.rs` — expected UUIDs
