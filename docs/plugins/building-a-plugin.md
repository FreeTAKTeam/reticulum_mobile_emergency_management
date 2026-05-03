# Building A Native Plug-In

Use `plugins/example-status-plugin` as the template for Android v1.

## Files

```text
plugin.toml
rust/
  Cargo.toml
  src/lib.rs
ui/
  settings.schema.json
schemas/
  <message>.schema.json
logic/android/<abi>/
  lib<plugin>.so
```

## Steps

1. Choose a reverse-DNS plug-in ID.
2. Declare Android library paths in `plugin.toml`.
3. Declare only the permissions the plug-in needs.
4. Declare every plug-in LXMF message and JSON schema.
5. Implement the C ABI entrypoints.
6. Use the host callback table for storage, subscriptions, and sends.
7. Build Android `.so` artifacts for the declared ABIs.
8. Package with `tools/rem-plugin-packager`.
9. Install from Settings. The plug-in starts disabled.
10. Grant permissions explicitly, then enable.

## Verification

```powershell
cargo test --manifest-path plugins/example-status-plugin/rust/Cargo.toml
cargo clippy --manifest-path plugins/example-status-plugin/rust/Cargo.toml -- -D warnings
cargo test --manifest-path tools/rem-plugin-packager/Cargo.toml
```

For production-like packages, omit `--allow-missing-libraries` and sign the
archive with `--publisher` plus `--signing-key-base64`.
