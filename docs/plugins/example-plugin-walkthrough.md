# Example Plug-In Walkthrough

The minimal status example lives under:

```text
plugins/example-status-plugin/
```

The BLE heart-rate example lives under:

```text
plugins/example-ble-heart-rate-plugin/
```

## Build Native Logic

For desktop unit checks:

```powershell
cargo test --manifest-path plugins/example-status-plugin/rust/Cargo.toml
cargo clippy --manifest-path plugins/example-status-plugin/rust/Cargo.toml -- -D warnings
cargo test --manifest-path plugins/example-ble-heart-rate-plugin/rust/Cargo.toml
cargo clippy --manifest-path plugins/example-ble-heart-rate-plugin/rust/Cargo.toml -- -D warnings
```

For Android CI, the workflow builds `arm64-v8a` with `cargo-ndk` and places the
library under the path declared in `plugin.toml`.

## Runtime Behavior

During `rem_plugin_init`, the example:

1. Reads `status_send_count` from plug-in-local storage.
2. Writes the incremented counter back through `storage_set`.
3. Subscribes to `rem.plugin.lxmf.received`.
4. Sends a `status_test` request through the host `send_lxmf` callback.

The example handler increments an in-memory receive counter when REM delivers a
validated `status_test` receive event.

The BLE heart-rate example keeps the BLE Heart Rate Measurement byte parser in
native plug-in code, stores the normalized sample through plug-in-local storage,
subscribes to validated plug-in LXMF receive events, and sends a
`heart_rate_sample` request through `send_lxmf`. Android v1 does not expose host
BLE scanning, so the example models the parser and message boundary without
adding wearable state to the core app.

## Package

Developer package without Android libraries:

```powershell
cargo run --manifest-path tools/rem-plugin-packager/Cargo.toml -- `
  plugins/example-status-plugin `
  output/example-status.remplugin `
  --allow-missing-libraries

cargo run --manifest-path tools/rem-plugin-packager/Cargo.toml -- `
  plugins/example-ble-heart-rate-plugin `
  output/example-ble-heart-rate.remplugin `
  --allow-missing-libraries
```

Signed package after Android libraries are built:

```powershell
cargo run --manifest-path tools/rem-plugin-packager/Cargo.toml -- `
  plugins/example-status-plugin `
  output/example-status.remplugin `
  --publisher FreeTAKTeam `
  --signing-key-base64 <32-byte-seed-base64>

cargo run --manifest-path tools/rem-plugin-packager/Cargo.toml -- `
  plugins/example-ble-heart-rate-plugin `
  output/example-ble-heart-rate.remplugin `
  --publisher FreeTAKTeam `
  --signing-key-base64 <32-byte-seed-base64>
```

Installed plug-ins are disabled by default. Grant `storage.plugin`,
`lxmf.send`, and `lxmf.receive` before enabling these examples.
