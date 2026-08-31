# Reticulum Emergency Management

Reticulum Emergency Management, or REM, helps a field team keep a shared picture when normal phones, internet, or command systems are unreliable.

It answers two urgent questions:

> How is everyone doing?
> What is happening now?

REM is built around simple pages for team status, chat, checklists, map positions, peer discovery, and event logs. It can work directly with trusted peers over Reticulum mesh networking, and it can also use Reticulum Community Hub support when a team chooses that setup.

The current Android source version is `1.3.1`, built against LXMF-rs `v0.10.1`
at immutable release revision
`25a976945cb335dff3be692981151c8741a5fdeb`.
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
- Create local color teams, import legacy team JSON or QR records, or use RCH-owned teams to scope outbound recipient sets while keeping shared timelines intact.
- Keep a household profile and publish compact `MECP/2/B04` community status such as All Home, One Missing, Evacuated, or Needs Help.
- Mark saved peers as Inner or Outer: only Inner peers can receive chat and exact direct telemetry.
- Create and import signed Block Codes for reviewed network bootstrap without exporting a Reticulum private key or Hub API key.
- Let native battery saver preserve SOS, exact Inner telemetry, and one status transition while pausing lower-priority mesh traffic.
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
- **Peers**: Finds REM-capable peers, imports legacy local-team JSON or QR records, merges matching RCH rosters, and selects the active team. New onboarding exports use signed Block Codes.
- **Settings**: Controls call sign, household and power policy, networking, telemetry, SOS, reviewed Block Code onboarding, peer import/export, and node controls.

## Household, Circles, And Block Codes

Set the household name, composition, role badges, preferred map layer, and
default status during setup or in Settings. The Dashboard buttons publish All
Home, One Missing, Evacuated, or Needs Help as a compact B04 community event.
This is separate from both Action Messages and MECP incident events.

Use the Peers page to classify every saved peer. Inner means trusted for direct
chat and exact location; Outer means community-directory visibility without
those private flows. Exact GPS is never sent through Connected Hub mode until
the Hub has an explicit recipient policy.

For onboarding another household, create a signed Block Code in Settings and
share its text or QR. The receiving operator reviews the signer fingerprint,
expiry, network settings, trusted destinations, household profile, and a tier
for every peer before importing. A Block Code never contains your Reticulum
private key or Hub API key. Old team QR codes can still be scanned for team
membership, but they are import-only and are not signed Block Codes.

When native battery saver is active, the app displays a saver badge and pauses
chat/retry and other lower-priority sends. SOS remains available. Saver mode
activates at the configured threshold, leaves a three-percent recovery margin,
turns off while charging, and reduces announce/telemetry frequency to at least
five minutes.

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
