# Reticulum Emergency Management

Reticulum Emergency Management, or REM, helps a field team keep a shared picture when normal phones, internet, or command systems are unreliable.

It answers two urgent questions:

> How is everyone doing?
> What is happening now?

REM is built around simple pages for team status, chat, checklists, map positions, peer discovery, and event logs. It can work directly with trusted peers over Reticulum mesh networking, and it can also use Reticulum Community Hub support when a team chooses that setup.

The current Android source version is `1.3.0`, built against LXMF-rs `v0.9.9`
at the immutable bearer-neutral RNode transport revision
`b7e4018b06f1ef5cb4e226a2b353e1609ccf06f7`.
Automated validation and the two-phone TCP matrix are release gates; the
remaining physical LoRa and mixed-interface rows are documented preview scope
in the [manual release matrix](docs/rem-1.2-manual-release-gate.md).

## Current UI

These screenshots were captured from the current app UI.

| Dashboard | Events with MECP | Chat |
| --- | --- | --- |
| ![REM dashboard](docs/screenshots/rem-dashboard.png) | ![REM Events MECP composer](docs/screenshots/rem-events-mecp.png) | ![REM chat](docs/screenshots/rem-chat.png) |

| Checklists | Map | Peers |
| --- | --- | --- |
| ![REM checklists](docs/screenshots/rem-checklists.png) | ![REM map](docs/screenshots/rem-map.png) | ![REM peers](docs/screenshots/rem-peers.png) |

## What REM Helps You Do

- See a quick team health picture on the Dashboard.
- Send and receive encrypted chat with saved peers.
- Share Emergency Action Messages so others know the current status of a person or team.
- Record Events as short timeline updates.
- Use MECP event codes for clear, compact emergency messages that can be understood across languages.
- Build and share Checklists for field tasks.
- Watch recent locations on the Map when telemetry is enabled.
- Discover, save, and connect to trusted peers.
- Create local color teams, exchange them by JSON or QR code, or use RCH-owned teams to scope every outbound recipient set while keeping shared timelines intact.
- Configure SOS emergency behavior, telemetry, peer lists, and Reticulum settings.

## Connect an RNode

For Android BLE, Bluetooth Classic/SPP, USB-assisted pairing, verification, and
recovery steps, use the [RNode Bluetooth connectivity guide](docs/rnode-bluetooth-connectivity.md).

## Events And MECP

The Events page now uses MECP, the Mesh Emergency Communication Protocol. MECP turns a plain choice like "Safety - Position - Stranded" into a compact message such as:

```text
MECP/2/P01
```

That message is short, readable, and suitable for low-bandwidth mesh links. The MECP project describes it as a structured text format for emergency and everyday communication across language barriers: [xiang-dev-1/MECP](https://github.com/xiang-dev-1/MECP).

In REM, an operator does not need to memorize the code. The Events page shows friendly choices for severity, category, event, and optional details, then shows the exact MECP body before it is added.

## Main Pages

- **Dashboard**: Shows team readiness, checklist counts, and recent activity totals.
- **Chat**: Holds one-to-one LXMF conversations with peers.
- **Checklists**: Creates and tracks shared task lists.
- **Map**: Shows recent peer positions and SOS locations when telemetry is available.
- **More**: Opens Action Messages, Events, Peers, and Settings.
- **Action Messages**: Captures the team status colors used by the Dashboard.
- **Events**: Records short timeline updates using MECP.
- **Peers**: Finds REM-capable peers, creates and shares local color teams by JSON or QR, merges matching RCH rosters, and selects the active team.
- **Settings**: Controls call sign, networking, telemetry, SOS, peer import/export, and node controls.

## Install With Obtainium

Use Obtainium to track releases from this repository and install updates directly:

[![Add to Obtainium](https://img.shields.io/badge/Add%20to-Obtainium-3ddc84?style=for-the-badge&logo=android&logoColor=white)](https://apps.obtainium.imranr.dev/redirect.html?r=obtainium://add/https://github.com/FreeTAKTeam/reticulum_mobile_emergency_management)

## For Maintainers

This repository contains:

- `apps/mobile`: the Vue and Capacitor mobile app.
- `packages/node-client`: the TypeScript bridge used by the app.
- `crates/reticulum_mobile`: the Rust runtime and Reticulum/LXMF bridge.
- `tools/codegen`: UniFFI binding generation scripts.
- `e2e`: browser-based end-to-end tests.

The current module ownership and responsiveness invariants are documented in
[`docs/runtime-modules.md`](docs/runtime-modules.md). Performance and footprint
acceptance results for version 1.2.6 are in
[`docs/performance/final-1.2.6.md`](docs/performance/final-1.2.6.md).
The [documentation index](docs/README.md) identifies the authoritative Markdown
manuals and clearly separates the archived PDF/DOCX snapshots.

Useful checks from the repository root:

```bash
npm ci
npm audit --audit-level=moderate
npm run check:source-size
npm run test:unit
npm --workspace apps/mobile run typecheck
npm run web:build
npm run mobile:build
npm run test:e2e
cargo +1.88 fetch --manifest-path crates/reticulum_mobile/Cargo.toml --locked
cargo +1.88 clippy --manifest-path crates/reticulum_mobile/Cargo.toml --all-targets -- -D warnings
cargo +1.88 test --manifest-path crates/reticulum_mobile/Cargo.toml --locked
cargo audit --file crates/reticulum_mobile/Cargo.lock
```

The source-size check is a hard gate: first-party source and test files, and
class declarations inside them, must remain at or below 500 physical lines.
Generated bindings, vendored code, and build output are excluded.

See [developer examples](docs/developer-examples.md) for classified native
errors, retry policy, runtime readiness, and delivery terminal states.
