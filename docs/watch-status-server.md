# REM Watch Status Server

REM Android exposes a loopback-only watch status endpoint from `ReticulumNodeService`.

- Default URL: `http://localhost:29863/info.json`
- Alias: `http://localhost:29863/northbound/watch/status`
- Default settings: enabled, port `29863`
- Valid custom port range: `1024..65535`

The server binds to `127.0.0.1` only and emits REM watch JSON v1:

```json
{
  "type": "rem.watch.status",
  "version": 1
}
```

The payload includes connection state, operator/team EAM status, sync age, event counts,
highest priority, alert state, and the latest event when one is available. No bearer token is
required for the default loopback bridge. If the configured port is unavailable, the service
records a bind error and the Settings screen can change the port.

Zepp REM Sync expects the default endpoint unless its local config is changed to match a custom
port.
