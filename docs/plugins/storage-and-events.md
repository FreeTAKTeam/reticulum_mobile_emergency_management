# Plug-In Storage And Events

## Persistent Storage

`storage.plugin` enables plug-in-local JSON storage. Values are persisted in
SQLite under a host-owned table keyed by plug-in ID and storage key.

Callbacks:

```text
storage_get(ctx, key, out)
storage_set(ctx, key, value_json)
free_buffer(ctx, buffer)
```

The host uses the plug-in identity from `ctx`. A plug-in cannot read or write
another plug-in's storage.

## Subscriptions

Plug-ins subscribe with:

```text
subscribe(ctx, topic)
```

The host records subscriptions per loaded plug-in. Event delivery checks both
subscription and permission. Unsupported topics are rejected by the runtime
event bridge.

## Event Delivery

Native events arrive through:

```text
rem_plugin_handle_event(event_json)
```

The event JSON shape is:

```json
{
  "topic": "rem.plugin.lxmf.received",
  "payload": {
    "pluginId": "rem.plugin.example_status",
    "messageName": "status_test",
    "wireType": "plugin.rem.plugin.example_status.status_test",
    "payload": {}
  }
}
```

Payloads are sanitized. Raw packets, raw LXMF objects, Reticulum identities, and
database records are not exposed.
