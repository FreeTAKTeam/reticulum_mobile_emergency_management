# Example BLE Heart Rate Plug-In

This plug-in is a reference implementation for moving BLE heart-rate telemetry
out of the core REM app and into the Android native plug-in system.

Android v1 plug-ins do not receive a host Bluetooth scanning API. This example
therefore keeps the BLE Heart Rate Measurement parser in native plug-in logic
and demonstrates how normalized readings are packaged as host-validated plug-in
LXMF messages. A future Android BLE host surface can feed real GATT bytes into
the same parser without adding wearable-specific state to the core app.

## Behavior

During `rem_plugin_init`, the plug-in:

1. Parses a sample BLE Heart Rate Measurement characteristic value.
2. Stores the latest sample through plug-in-local storage.
3. Subscribes to validated plug-in LXMF receive events.
4. Sends a `heart_rate_sample` plug-in LXMF request through the host callback.

Settings also expose a host-rendered `send_lxmf` action for sending manual
sample readings through REM's existing plug-in message validation path.

## Build

```powershell
cargo test --manifest-path plugins/example-ble-heart-rate-plugin/rust/Cargo.toml
cargo clippy --manifest-path plugins/example-ble-heart-rate-plugin/rust/Cargo.toml -- -D warnings
```

For Android packaging, build the native library for each declared ABI and place
it under `logic/android/<abi>/libexample_ble_heart_rate_plugin.so`.

Developer package without Android libraries:

```powershell
cargo run --manifest-path tools/rem-plugin-packager/Cargo.toml -- `
  plugins/example-ble-heart-rate-plugin `
  output/example-ble-heart-rate.remplugin `
  --allow-missing-libraries
```

Installed plug-ins are disabled by default. Grant `storage.plugin`,
`lxmf.send`, and `lxmf.receive` before enabling this example.
