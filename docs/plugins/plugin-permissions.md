# Plug-In Permissions Reference

Plug-ins declare permissions in `plugin.toml`, but declarations are not grants.
The operator-controlled registry stores granted permissions separately. A host
API call succeeds only when the permission is both declared and granted.

## Implemented Android V1 Permissions

```toml
[permissions]
storage_plugin = true
lxmf_send = true
lxmf_receive = true
messages_read = true
notifications_raise = true
```

- `storage_plugin`: allows plug-in-local persistent key/value JSON storage.
- `lxmf_send`: allows host-validated plug-in LXMF sends.
- `lxmf_receive`: allows plug-in LXMF receive decoding and event delivery.
- `messages_read`: allows subscription to sanitized message topics.
- `notifications_raise`: reserved for host-rendered notification support.

## Explicitly Unsupported In V1

Map overlays, BLE, location, local/internet network access, raw filesystem
access, shared storage, direct database handles, raw LXMF objects, and raw RNS
objects are not exposed. Any future callable surface for these permissions must
be implemented as a narrow host API with typed permission checks.

## Runtime Topics

The native runtime only forwards sanitized v1 topics:

```text
rem.message.received
rem.message.sent
rem.plugin.lxmf.received
rem.plugin.started
rem.plugin.stopped
```

Plug-ins do not subscribe to all events by default.
