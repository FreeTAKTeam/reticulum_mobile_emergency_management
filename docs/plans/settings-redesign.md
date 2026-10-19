# REM Settings redesign proposal

Status: design only, nothing implemented. Written 2026-10-19 against `main` at 451f8d9.

## 1. What is confusing today

The Settings route is a single long page (`apps/mobile/src/views/SettingsView.vue`) that stacks ten panels in this order:

| # | Panel | Kind | How changes apply |
|---|-------|------|-------------------|
| 1 | Node Config | fold panel | global Save |
| 2 | Telemetry | fold panel | global Save |
| 3 | Household & power (eyebrow "Community profile") | always open | global Save |
| 4 | Signed Block Code (eyebrow "Trusted onboarding") | always open | immediate actions |
| 5 | Plugins | fold panel | immediate actions |
| 6 | RCH Hub Directory | fold panel | global Save **and** immediate buttons |
| 7 | Manage Teams | navigation row | separate screen |
| 8 | SOS Emergency | fold panel | global Save, except PIN and "Trigger SOS" which are immediate |
| 9 | Node Control | fold panel | immediate actions |
| 10 | About | fold panel | read only |

The specific problems, each observed in the current source:

1. **No grouping and no hierarchy.** Ten peers of equal visual weight, in an order that follows implementation history rather than user intent. "Household & power" sits between Telemetry and Block Code; "SOS" sits between Teams and Node Control.
2. **Three different panel idioms on one page.** Fold panels (`<details>`), always-open sections, and a navigation row. The user cannot predict whether tapping a header expands something or leaves the page.
3. **Two save models mixed in the same panels.** One global Save button at the top commits Node Config, Telemetry, Household, Hub and SOS. But inside those same panels, Hub "Refresh Now / Register / Clear", SOS "Set PIN / Clear PIN / Trigger SOS", and every Plugins and Block Code button apply immediately. The "Unsaved: N" chip counts panels, not fields, so it only ever reads 0, 1 or 2.
4. **The header has two "Node Control" entry points that do different things.** The header chip labeled "Node Control" opens the **Node Config** panel. The actual **Node Control** panel (Start/Stop/Restart, interfaces, logs) is at the bottom of the page.
5. **Node Config is a kitchen sink.** In one panel: identity (Call Sign, Own app hash), developer switches (Client mode), expert tuning (Announce capabilities, Announce interval, Broadcast, Transport forwarding), TCP interface list, the whole LoRa/RNode pairing workflow (scan, paired list, USB pairing), read-only propagation-node status, and a telemetry error line that has nothing to do with the panel.
6. **Tasks are presented as settings.** Pairing an RNode, creating or scanning a Block Code, registering with a hub, and triggering SOS are one-off actions with their own flow and feedback. They are rendered as fields and buttons inside configuration panels.
7. **Status is mixed with configuration.** Telemetry status, propagation node, hub registration, interface state and node logs are read-only diagnostics placed inside editable panels.
8. **Restart requirements are discovered after the fact.** Whether a change needs a node restart (call sign, TCP, hub routing, LoRa) is only revealed in the post-save feedback string.
9. **Jargon without a ladder.** "RCH Hub Directory", "Announce capabilities", "Reticulum syntax", "Own app hash", "Semi-autonomous", `rem.registry.team_peers.list` in a help paragraph. Beginner, operator and developer settings share one surface with no "Advanced" tier.
10. **Setup Wizard and Settings disagree on structure.** The wizard orders things as Call sign + Household → TCP → RNode → Telemetry → Permissions → SOS → Review. Settings uses a different grouping and has no Permissions section at all, so a user who learned the wizard cannot find things again.
11. **Hidden fields.** `hubApiBaseUrl` and `hubApiKey` are in the form state but not rendered anywhere.
12. **Teams is not a setting.** Manage Teams is peer/data management and already has its own route; it is also reachable from Peers. Listing it as a Settings panel is a second entrance to a different kind of screen.

## 2. Design principles for the new layout

- **Group by user goal, not by subsystem.** "Who am I", "How do I connect", "What do I share", "Emergency", "Device". Subsystem names (Hub, RNode, LXMF) appear as sub-pages, not top-level labels.
- **Hub-and-spoke navigation.** A short index screen of rows, each row opens one focused sub-page. This is the standard mobile settings pattern and it makes every screen fit in one thumb-length. It also keeps each view well under the 500-line gate.
- **One save model per screen.** A sub-page is either a *form* (edit, then Save / Discard, with an unsaved-changes guard on back) or an *action screen* (every button applies immediately). Never both. Immediate actions inside a form move to a separate "Actions" sub-page or to a clearly separated task row.
- **Settings, tasks and status are visually distinct.** Three row styles on the index and in sub-pages: a *setting row* (label, current value, chevron), a *task row* (verb label, no value, chevron or button), a *status row* (label, read-only value, no chevron).
- **Restart needs are visible up front.** Fields that need a node restart carry a small "restart" badge. After saving such a field, a persistent banner at the top of Settings reads "Restart node to apply: call sign, TCP interfaces" with a Restart button. The banner clears when the node restarts.
- **Three tiers of depth.** Each sub-page shows the essential fields first. Expert fields are behind an "Advanced" disclosure at the bottom of the page. Developer-only fields (Client mode) move into Diagnostics.
- **Index rows summarize the current value.** Each row's secondary line shows the state ("2 TCP, LoRa off", "Sharing every 60 s", "Disabled"), reusing the summary strings the panels already compute.
- **Index sections fold.** Each section header (You, Connectivity, Sharing, Emergency, Device) is a tap target that collapses its rows. A folded header shows a one-line summary of what is inside ("2 TCP · LoRa off · Autonomous") so nothing is hidden blind. Fold state is a per-device UI preference remembered across launches; all sections start open on first run. The restart banner and the About / setup footer never fold.
- **Same building blocks as the wizard.** The wizard steps should become the same sub-page forms in a stepped shell, so Settings and Setup share one vocabulary and one order.

## 3. Proposed structure

### Index screen (`/settings`)

```
Settings
────────────────────────────────────────
[banner, only when needed]
⟳ Restart node to apply: call sign, TCP interfaces     [Restart]
────────────────────────────────────────
YOU
  👤 Identity              CORVO · Household "Harbour House"   ›
  👥 Teams                 2 local · 1 from RCH                 ›

CONNECTIVITY
  🔌 Network interfaces    2 TCP · LoRa off                     ›
  🧭 Routing & hubs        Autonomous                           ›

SHARING
  📍 Location sharing      Every 60 s · fresh                   ›
  🗺 Map                   Base layer                           ›

EMERGENCY
  ⚠ SOS                   Shake + power button · PIN set       ›

DEVICE
  🔋 Battery saver         On at 20 %                           ›
  🧩 Plugins               1 installed · 1 enabled              ›
  🔐 Share this setup      Create or scan a Block Code          ›
  🛠 Node & diagnostics    Running · 3 interfaces               ›

  ℹ About                  REM 1.5.3                            ›
  ↻ Run setup wizard again
```

No Save button on the index. No "Unsaved" chip. The header is just the title. Section headers fold; a folded Sharing section would read:

```
SHARING  ·  Location every 60 s · Base map                      ›
```

Interactive mockup (private, REM Night ops style): https://claude.ai/artifact/HuC3k4DreNFYyctU5seXAX

### Sub-pages

Each sub-page below is a route under `/settings/…`. "Form" pages have a sticky bottom bar with Discard and Save. "Action" pages apply immediately.

#### Identity (`/settings/identity`) — form
- Call sign *(restart badge)*
- Household name, Household ID
- Adults, Children, Pets
- Role badges (chips input rather than comma text)
- Status row: Own app hash, with a copy button
- Advanced: Announce interval, Announce capabilities *(restart badge)*

Rationale: everything here describes "who this node is to others". The announce fields are the expert end of that same idea.

#### Teams (`/settings/teams`) — existing screen
Keep the existing Manage Teams view. The Settings index only links to it. Consider also surfacing it from the Peers tab as today and dropping it from Settings later if usage shows the Peers entry suffices.

#### Network interfaces (`/settings/network`) — form with task rows
- Section **TCP**: the known-server checklist, custom endpoint input, active endpoint chips *(restart badge on the section)*.
- Section **LoRa / RNode**: Enable toggle. When enabled: Region, Profile, Frequency, read-only "Reticulum syntax" row.
- Task row inside LoRa: **Pair RNode device ›** opens `/settings/network/rnode-pairing`, an action page that holds bearer choice, Show paired, Scan BLE/Classic, Pair via USB, the device lists, and the device id / display name. Selecting a device writes back to the form and returns.
- Advanced: Broadcast enabled, Transport node forwarding *(restart badge)*.

Rationale: the pairing workflow is the largest block of the current Node Config panel and is used rarely. Moving it to its own action page makes the form short and gives the scan feedback a proper home.

#### Routing & hubs (`/settings/routing`) — form with an Actions section
- Mode: Autonomous / Semi-autonomous / Connected, each option with a one-line plain-language description instead of the current paragraph.
- Hub: picker from announces, manual identity hash *(restart badge)*.
- Refresh interval.
- Status rows: Active propagation node, Selection mode, Hub registration state.
- Actions section (immediate, visually separated): Refresh directory now, Register team member, Clear registration.
- Advanced: Hub API base URL, Hub API key (today hidden).

#### Location sharing (`/settings/location`) — form
- Enable toggle.
- Publish interval, Accuracy threshold.
- Stale after / Expire after, presented as a pair ("Marks peers stale after … and hides them after …") with inline validation that expire ≥ stale.
- Status row: Telemetry status (current readout), and the telemetry error if any.
- Permission row: Location permission state with a Request button. This is where the wizard's Permissions step lands in Settings.

#### Map (`/settings/map`) — form
- Preferred map layer (Base / Satellite).
- Reserved space for offline vector maps (see `docs/plans/offline-vector-maps-243.md`).

#### SOS (`/settings/sos`) — form plus one action section
- Enable SOS.
- Message template, Emergency end template.
- Include GPS and battery.
- Triggers: Shake (with sensitivity shown only when on), Power button, Floating button.
- Deactivation PIN: a single row "PIN · set / not set ›" that opens a small PIN sheet with Set and Clear. Keeps PIN out of the form's save cycle.
- Danger section at the very bottom: **Send a test SOS** with a confirm step. Today "Trigger SOS" is an unguarded button inside the form.
- Permission row: Notifications permission state.

#### Battery saver (`/settings/power`) — form
- Automatic power saver toggle, Activate at %, the hysteresis note.

Rationale: separates it from the household profile, which it has nothing to do with.

#### Plugins (`/settings/plugins`) — action page
Current content, unchanged in behavior. Each plugin becomes its own row that opens a plugin detail page (`/settings/plugins/:id`) with capabilities, enable/disable, configure, map sessions and revoke. The list page stays short.

#### Share this setup (`/settings/share-setup`) — action page
The Block Code feature. Two clear tasks: **Create a signed code** (expiry picker, QR, copy) and **Import a code** (scan or paste, inspect, review). Named by what the user is doing, with "Signed Block Code" as the explanatory subtitle.

#### Node & diagnostics (`/settings/node`) — action page
- Start / Stop / Restart / Recreate client.
- Interfaces list with state and counters.
- Node log.
- Developer section: Client mode (Auto / Capacitor only).
- Last error.

This is the one place for runtime control. The header chip that currently opens Node Config is removed.

#### About (`/settings/about`)
Unchanged content.

### Where every current control goes

| Current location | Control | New location |
|---|---|---|
| Node Config | Client mode | Node & diagnostics › Developer |
| Node Config | Call Sign | Identity |
| Node Config | Own app hash | Identity (status row) |
| Node Config | Announce capabilities, interval | Identity › Advanced |
| Node Config | Broadcast, Transport forwarding | Network › Advanced |
| Node Config | TCP interfaces | Network › TCP |
| Node Config | RNode enable, region, profile, frequency, syntax | Network › LoRa |
| Node Config | RNode bearer, device id, name, scan/pair buttons, device lists | Network › Pair RNode device |
| Node Config | Active propagation node, Selection mode | Routing (status rows) |
| Node Config | telemetry error line | Location sharing (status) |
| Telemetry | all fields | Location sharing |
| Household & power | household fields, role badges | Identity |
| Household & power | Preferred map | Map |
| Household & power | power saver | Battery saver |
| Signed Block Code | create / scan / inspect | Share this setup |
| Plugins | all | Plugins (list + detail) |
| RCH Hub Directory | mode, hub, hash, refresh | Routing & hubs |
| RCH Hub Directory | Refresh / Register / Clear | Routing & hubs › Actions |
| (hidden) | hub API base URL, API key | Routing & hubs › Advanced |
| Manage Teams | row | Teams row (unchanged target) |
| SOS Emergency | settings | SOS |
| SOS Emergency | Set / Clear PIN | SOS › PIN sheet |
| SOS Emergency | Trigger SOS | SOS › Send a test SOS (confirmed) |
| Node Control | all | Node & diagnostics |
| Header chip "Node Control" | opens Node Config | removed |
| Header chip "Setup Wizard" | | index footer row |
| Header Save / Unsaved chip | | per-page sticky Save bar |
| About | | About |
| Setup wizard Permissions step | location, notifications | Location sharing and SOS permission rows |

## 4. Save and restart semantics

- Each form page owns its own draft, built from the same store settings the current single form reads. Save calls the existing `nodeStore.updateSettings` with only that page's slice merged over the current settings. No new domain logic in the UI; validation and normalization stay where they are today (`utils/rnodeProfiles`, `utils/settingsSaveErrors`, the Rust runtime).
- Back navigation with unsaved changes prompts Discard / Keep editing.
- After Save, if any saved field carries the restart badge, the store records the pending restart reasons and the index banner shows them. The banner's Restart button calls the existing restart action. This replaces the current guesswork feedback string.
- Immediate actions never live on a form page except inside a clearly bordered "Actions" section at the bottom, and they never affect the form's dirty state.

## 5. Setup wizard alignment

Reorder the wizard to match the index: Welcome → Identity → Network (TCP, then optional LoRa) → Routing (optional, default Autonomous) → Location sharing (with permission) → SOS (with notification permission) → Review. Each step renders the same form component as the corresponding Settings sub-page, inside the wizard shell. Settings and Setup then share labels, order and validation, and the "Run setup wizard again" row becomes a guided walk through the same screens.

## 6. Suggested phasing

1. Index + routing shell, with each existing panel moved unchanged into its own sub-page. No field moves yet. This alone removes problems 1, 2 and 4.
2. Split Node Config into Identity, Network, Routing and Node & diagnostics. Move Household, Map, Battery saver. Per-page Save bar and restart banner.
3. RNode pairing page, PIN sheet, SOS test confirmation, Plugins detail page, Advanced disclosures.
4. Wizard reuse of the sub-page forms.

Each phase is independently shippable and keeps `e2e/` selectors updatable one screen at a time.

## 7. Open questions

- Should Teams stay reachable from Settings at all, or only from Peers?
- Is "Connected" hub mode common enough to show the hub picker on the main Routing page, or should it appear only when mode ≠ Autonomous?
- Do field operators want the index to show a compact "node health" status row at the top (running, interfaces up, hub reachable) in addition to the restart banner?
