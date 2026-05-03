# Android Native Plug-In System

This document tracks the implemented Android-first REM plug-in surface on the
`codex/android-plugin-system` branch.

## Scope

Android plug-ins are trusted, side-loaded packages installed into app-private
storage. A plug-in package contains:

- `plugin.toml`
- an Android native library for one or more ABIs
- optional host-rendered Settings schema
- one or more declared REM plug-in LXMF message schemas
- declared host permissions

REM owns Reticulum routing, LXMF field construction, delivery tracking,
permission checks, Settings rendering, and all access to runtime internals.
Plug-ins do not receive raw database handles, filesystem handles, LXMF objects,
RNS objects, or mutable runtime references.

## Package Layout

```text
plugin.toml
logic/
  android/
    arm64-v8a/
      libexample_status_plugin.so
ui/
  settings.schema.json
schemas/
  status_test.schema.json
assets/
```

The installed copy lives under:

```text
<app storage>/plugins/<plugin id>/
```

Staged package installation is limited to extracted package directories or
`.remplugin` archives under:

```text
<app storage>/plugin-packages/<package directory>/
```

The node rejects staged installs outside that app-private staging root before it
calls the package installer.

## Packaging

Use the local packager tool to create a `.remplugin` archive:

```powershell
cargo run --manifest-path tools/rem-plugin-packager/Cargo.toml -- plugins/example-status-plugin output/example-status.remplugin --allow-missing-libraries
```

Omit `--allow-missing-libraries` for production packages. Without that flag, the
packager requires every Android library path declared in `plugin.toml` to exist
before the archive is written.

## Manifest

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

Validation rejects missing platform libraries, missing settings schemas, missing
message schemas, unsafe relative paths, absolute paths, and plug-in IDs that are
not reverse-DNS style.

## Runtime State

Installed plug-ins are disabled by default. The persisted registry stores
runtime state and granted permissions separately from the manifest:

- `Disabled`
- `Enabled`
- `Loaded`
- `Initialized`
- `Running`
- `Stopped`
- `Failed`

Declared permissions are never granted automatically. Host API calls require the
permission to be both declared by the manifest and granted by the registry.
Persisted grants are intersected with the manifest declarations when loaded.

## Android Bridge

The native Android bridge exposes:

- `getPlugins`
- `installPluginArchive`
- `installPluginPackage`
- `setPluginEnabled`
- `grantPluginPermissions`

`installPluginArchive` accepts a `.remplugin` filename plus base64 archive
bytes from the Capacitor UI, writes the archive under app-private
`plugin-packages` storage, and then calls the native package installer.

`installPluginPackage` accepts a staged `packagePath` and returns the updated
plug-in catalog. The path may point to an extracted package directory or a
`.remplugin` archive, but it must be under app-private `plugin-packages`
storage. It does not accept network URLs or marketplace identifiers.

## Settings

Settings contains a fold-out section named **Plugin**. Operators can install a
local `.remplugin` archive from this section. Each installed plug-in is listed
with its state, declared/granted permission controls, and declared LXMF message
count. Plug-ins with a valid Settings schema also get host-rendered
configuration controls in the same section. The section also shows the most
recent validated plug-in LXMF receive events observed by the app store.

Android v1 does not execute side-loaded Vue bundles inside Settings.

Settings schemas may declare host-rendered actions. The initial action type is
`send_lxmf`, which maps configured fields to a `PluginLxmfSendRequest` and lets
REM validate permissions, build the plug-in LXMF field envelope, and send
through the native runtime.

## Plug-In LXMF Messages

Plug-ins declare structured REM message types in `plugin.toml`. REM namespaces
each message as:

```text
plugin.<plugin_id>.<message_name>
```

Plug-ins submit structured send requests to REM. REM validates the declared
message, checks `lxmf.send`, builds the host-owned field envelope, and sends the
message through the existing Rust runtime.

Plug-in LXMF sends are only available through the native REM plug-in host. Web
and mock TypeScript clients fail the request instead of degrading it to a normal
LXMF send, because that would bypass manifest, permission, and schema checks.

Plug-in message traffic uses the field key:

```text
rem.plugin.message
```

This separates plug-in traffic from existing REM/RCH mission and SOS traffic.

Received plug-in LXMF messages are decoded through the native bridge, checked
against the plug-in manifest and granted `lxmf_receive` permission, emitted to
the owning native plug-in runtime as `rem.plugin.lxmf.received`, and mirrored to
the mobile store as `pluginLxmfReceived` for UI observability.

For both outbound and inbound plug-in LXMF messages, REM loads the declared
message schema from the installed plug-in directory and validates the structured
payload before building or accepting the host-owned field envelope. Android v1
supports the schema subset used by host-rendered Settings actions: object
payloads, required fields, primitive property types, string length bounds, and
`additionalProperties = false`.
