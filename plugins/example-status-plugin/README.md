# Example Status Plugin

This is the Android-first native plug-in example for REM.

The Rust crate builds a C ABI dynamic library with the entrypoints expected by
the REM Android loader:

- `rem_plugin_metadata`
- `rem_plugin_init`
- `rem_plugin_start`
- `rem_plugin_stop`
- `rem_plugin_handle_event`

The manifest declares one host-owned plug-in LXMF message:

```text
plugin.rem.plugin.example_status.status_test
```

Android package builds must place compiled libraries at the paths declared in
`plugin.toml`, then zip this directory as a `.remplugin` archive.
