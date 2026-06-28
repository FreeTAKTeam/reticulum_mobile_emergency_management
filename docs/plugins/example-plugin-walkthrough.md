# Example Status Plug-In Walkthrough

The example plug-in lives under:

```text
plugins/example-status-plugin/
```

## Build Native Logic

For desktop unit checks:

```powershell
cargo test --manifest-path plugins/example-status-plugin/rust/Cargo.toml
cargo clippy --manifest-path plugins/example-status-plugin/rust/Cargo.toml -- -D warnings
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

## Package

Developer package without Android libraries:

```powershell
cargo run --manifest-path tools/rem-plugin-packager/Cargo.toml -- `
  plugins/example-status-plugin `
  output/example-status.remplugin `
  --allow-missing-libraries
```

Signed package after Android libraries are built:

```powershell
cargo run --manifest-path tools/rem-plugin-packager/Cargo.toml -- `
  plugins/example-status-plugin `
  output/example-status.remplugin `
  --publisher FreeTAKTeam `
  --signing-key-base64 <32-byte-seed-base64>
```

Installed plug-ins are disabled by default. Grant `storage.plugin`,
`lxmf.send`, and `lxmf.receive` before enabling this example.
