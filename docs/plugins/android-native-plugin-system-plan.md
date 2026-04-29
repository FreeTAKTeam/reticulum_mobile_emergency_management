# Android Native Plug-In System Plan

This branch, `codex/android-plugin-system`, is the long-term integration branch for REM Android plug-ins. Keep `main` clean while this work evolves, and merge or rebase from `main` at milestone boundaries so runtime, Android bridge, and UI drift is handled early.

## Summary

REM plug-ins are trusted, side-loadable Android plug-ins that provide:

- `plugin.toml`
- a native Rust dynamic library for Android ABIs
- declarative Settings configuration
- one or more REM application-level LXMF message structures
- a permission declaration

The plug-in defines payload structure. REM owns LXMF encoding, Reticulum routing, delivery tracking, permission checks, and all access to native runtime internals.

## Branch Rules

- Keep this work on `codex/android-plugin-system`.
- Commit by milestone instead of one large final commit.
- Keep the branch disabled-by-default until install, permission, and message tests are in place.
- Do not expose raw SQLite handles, raw filesystem access, raw RNS objects, raw LXMF objects, or mutable runtime references.
- Do not add marketplace distribution, network downloads, or auto-enable behavior in v1.

## Milestones

1. Manifest, permission, Settings schema, and plug-in LXMF message descriptors.
2. Plug-in registry and disabled/enabled state persistence.
3. Safe `.remplugin` archive installation into app-private storage.
4. Android C ABI loader and test plug-in loading.
5. Restricted host API with declared-plus-granted permission enforcement.
6. Host-owned plug-in LXMF message send/receive path.
7. `packages/node-client` and Android Java bridge methods.
8. Settings **Plugin** section with host-rendered per-plugin configuration.
9. Example status plug-in and documentation.

## Manifest Contract

```toml
id = "rem.plugin.example_status"
name = "Example Status Plugin"
version = "0.1.0"
rem_api_version = ">=1.0.0,<2.0.0"
plugin_type = "native"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"

[settings]
schema = "ui/settings.schema.json"

[permissions]
storage_plugin = true
lxmf_send = true
lxmf_receive = true

[[messages]]
name = "status_test"
version = "1.0.0"
direction = ["send", "receive"]
schema = "schemas/status_test.schema.json"
```

Message names are namespaced by the host as:

```text
plugin.<plugin_id>.<message_name>
```

Example:

```text
plugin.rem.plugin.example_status.status_test
```

## LXMF Message Rule

Plug-ins do not build raw LXMF messages. A plug-in submits a structured request to REM, and REM validates the message type and payload, checks permissions, builds the host-owned LXMF field envelope, and sends it through the existing Rust runtime.

Use a dedicated plug-in field key:

```text
rem.plugin.message
```

This keeps plug-in traffic separate from existing REM/RCH mission and SOS traffic that shares `FIELD_COMMANDS (0x09)`.

## Settings Rule

Settings gets a fold-out section named **Plugin**. Each installed plug-in may contribute configuration controls there through a declarative schema. Android v1 uses host-rendered controls only; it does not execute arbitrary side-loaded Vue bundles in Settings.

## Validation

Use the narrowest command that proves each milestone:

```powershell
cargo test --manifest-path crates/reticulum_mobile/Cargo.toml --test plugin_manifest
cargo test --manifest-path crates/reticulum_mobile/Cargo.toml
npm --workspace packages/node-client run build
npm --workspace apps/mobile run typecheck
npm run web:build
```

For Android bridge milestones:

```powershell
npm --workspace apps/mobile run sync
cd apps/mobile/android
cmd /c gradlew.bat assembleDebug
```
