# Plug-In Manifest Reference

REM Android native plug-ins declare their host contract in `plugin.toml`.
The manifest is parsed before installation and again when the runtime discovers
installed plug-ins.

## Required Fields

```toml
id = "rem.plugin.example_status"
name = "Example Status Plugin"
version = "0.1.0"
rem_api_version = ">=1.0.0,<2.0.0"
plugin_type = "native"
```

- `id`: reverse-DNS style identifier. Example: `rem.plugin.example_status`.
- `name`: display name used by Settings.
- `version`: semantic plug-in version.
- `rem_api_version`: supported REM plug-in API range.
- `plugin_type`: Android v1 only accepts `native`.

## Android Libraries

```toml
[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"
armeabi_v7a = "logic/android/armeabi-v7a/libexample_status_plugin.so"
x86_64 = "logic/android/x86_64/libexample_status_plugin.so"
```

Library paths must be relative package paths. Absolute paths, `..`, path
prefixes, missing files, and files outside the installed plug-in directory are
rejected.

## Settings Schema

```toml
[settings]
schema = "ui/settings.schema.json"
```

Android v1 renders Settings controls from JSON. It does not execute arbitrary
side-loaded Vue bundles. Settings actions may call host-owned operations such
as `send_lxmf`.

## Message Declarations

```toml
[[messages]]
name = "status_test"
version = "1.0.0"
direction = ["send", "receive"]
schema = "schemas/status_test.schema.json"
```

Each message name must be unique. REM namespaces message wire types as:

```text
plugin.<plugin_id>.<message_name>
```

The schema path is validated during install and payloads are checked before
send or receive.
