# Example Status Plugin

This is the Android-first native plug-in example for REM.

The Rust crate builds a C ABI dynamic library with the entrypoints expected by
the REM Android loader:

- `rem_plugin_metadata`
- `rem_plugin_init`
- `rem_plugin_start`
- `rem_plugin_stop`
- `rem_plugin_handle_event`

The manifest declares one host-owned plug-in LXMF message that can be sent and
received through REM:

```text
plugin.rem.plugin.example_status.status_test
```

Android package builds must place compiled libraries at the paths declared in
`plugin.toml`, then zip this directory as a `.remplugin` archive.

During `rem_plugin_init`, the native library uses the REM host callback table
to:

- read and increment the plug-in-local `status_send_count` storage value
- subscribe to `rem.plugin.lxmf.received`
- send a `status_test` message request through `send_lxmf`

The Settings schema exposes a destination field, message field, and host-rendered
`Send test status` action. The action submits a structured `status_test`
payload to REM; the plug-in never constructs raw LXMF bytes itself.

On receive, REM validates the `status_test` field envelope and delivers a
`rem.plugin.lxmf.received` event to `rem_plugin_handle_event`; the example
handler records that event without touching raw LXMF or Reticulum internals.
