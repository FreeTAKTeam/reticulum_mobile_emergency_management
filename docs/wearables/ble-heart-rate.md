# BLE Heart Rate Wearables

REM supports generic Bluetooth LE devices that expose the standard Heart Rate Service.

## Standard Profile

- Heart Rate Service: `0000180d-0000-1000-8000-00805f9b34fb`
- Heart Rate Measurement characteristic: `00002a37-0000-1000-8000-00805f9b34fb`
- Client Characteristic Configuration Descriptor: `00002902-0000-1000-8000-00805f9b34fb`

The Android adapter scans for devices advertising service `0x180D`, can list bonded devices, connects with `BluetoothGatt`, discovers services, and subscribes to Heart Rate Measurement notifications. BPM values are forwarded as normalized `wearable.heart_rate` events.

## Tested Devices

The first tested target is Amazfit T-Rex 2. The implementation is not Amazfit-specific and should work with any smartwatch, band, chest strap, or fitness device that exposes the standard BLE Heart Rate Service.

## Android Permissions

Android 12 and newer use `BLUETOOTH_SCAN` and `BLUETOOTH_CONNECT`. Older Android versions use `BLUETOOTH`, `BLUETOOTH_ADMIN`, and `ACCESS_FINE_LOCATION`. REM checks permissions before scanning or connecting and reports missing permissions in Settings.

## Pairing And Bonding

Some devices only advertise the Heart Rate Service after pairing, enabling an external HR broadcast mode, or starting a workout. REM can scan live advertisements and can also list bonded devices for manual selection.

## Event Format

```json
{
  "type": "wearable.heart_rate",
  "source": "ble_gatt_standard_hr",
  "device_id": "ble-hr-device-identifier",
  "device_name": "Generic BLE Heart Rate Device",
  "device_model": "generic_ble_heart_rate_device",
  "timestamp_ms": 1760000000000,
  "sensor_type": "heart_rate_bpm",
  "value": 82,
  "unit": "bpm",
  "confidence": 0.95,
  "connection_state": "SUBSCRIBED"
}
```

Rust stores the latest wearable status per `device_id` and sensor type, publishes a wearable projection invalidation, and exposes active, stale, offline, or unsupported status to the UI.

## Operator Association

One watch is not assumed to equal one operator. Settings allows a wearable `device_id` to be manually associated with an operator RNS identity. Unmapped devices remain unassigned wearable telemetry.

Example settings shape:

```yaml
wearables:
  enabled: true
  stale_timeout_seconds: 30
  devices:
    - device_id: "ble-hr-example"
      alias: "Operator watch"
      operator_id: "operator-001"
      sensor_type: "heart_rate_bpm"
```

## Troubleshooting

- Confirm Bluetooth is enabled.
- Grant BLE permissions from Settings.
- Pair the watch if it does not appear while scanning.
- Enable the device's external heart-rate broadcast mode if available.
- Start a workout mode if the watch only exposes live HR during activity.
- Check that the device exposes Heart Rate Service `0x180D`.

Not all smartwatches expose live heart rate through standard BLE. Some devices require a workout mode, external HR broadcast mode, vendor-specific pairing, or proprietary protocol. If Heart Rate Service `0x180D` is not found, the device is not supported by this generic adapter.

## Limitations

This adapter does not implement vendor protocols, cloud sync, SpO2, stress, HRV, sleep, steps, GPS, workout history, accelerometer, or gyro data. Future adapters can add vendor-specific or additional standard services without changing the normalized wearable event shape.
