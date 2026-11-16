# Device plugin foundation

How REM plugins for decoded field devices (rangefinders, radios, sensors) are built. The
[Android plugin system](android-plugin-system.md) is the host contract; this document is the
pattern a device plugin follows on top of it. The reference implementation is the
[ATN Rangefinder plugin](https://github.com/FreeTAKTeam/atn-rangefinder) for the ATN Binox 4K.

## Layout

Each decoded device gets its own folder, `plugins/<device>/`, with its own README describing
the device, its protocol evidence and its limits. A plugin with its own repository is built from
that repository by `tools/build-offline-release.sh`.

## Split of responsibilities

- **Rust core** (when the device has a protocol worth testing): wire codecs, physical-semantic
  validation (units, reference frames, plausible ranges), geometry and the decision of what may
  be written to the device. It exposes one JSON command/action interface through JNI and is
  tested with synthetic fixtures only.
- **Android shell**: Bluetooth and platform permissions, the trusted REM Binder boundary,
  scheduling and the screen. It executes the actions the core returns and never reinterprets
  device bytes itself.
- **REM** owns marker identity and storage, replication, the observer identity, sessions and
  capability grants. A plugin never mints marker IDs or stores a second copy of the map.

## Convention over configuration

- Everything defaults to the plugin's main target device, from one built-in defaults file in the
  core. Nothing has to be entered before first use.
- Overrides are optional, partial JSON merged onto the defaults, behind an **Advanced** section.
  Unknown keys are rejected; `{}` restores the defaults.
- Values not yet confirmed on the real device are marked "to confirm" in the defaults' evidence
  text. They do not block operation, but the safety checks in the core stay in place.
- Engineering limits (timeouts, queue sizes, MTU) are built in and never part of the operator's
  flow.

## Operator flow

- One screen: a plain-language status block first (REM, device, what flows where, the last
  reading), then the few actions the operator needs, for example Scan, Connect and Disconnect.
- Connecting asks only what software cannot know. For a device whose readings are relative to
  the phone, the question is "Is this phone with the device?".
- Data flows automatically after connecting, with one switch to turn it off. Nothing is resent
  after a reconnect or restart.
- Never ask the operator to pair in Android settings when the device works without pairing.
- Problems are reported once, in plain words, naming the REM permission or step that fixes them.

## Position and map

- **Observer position:** while connected, the plugin keeps the phone's GPS running and uses that
  fix. REM's `map.observer.snapshot` is the fallback, because REM refreshes it only at the
  telemetry interval and not in the background. Apply explicit age and accuracy limits, and
  re-ask REM no faster than the plugin's reconciliation interval.
- **Device readings become REM markers** through `map.markers.create` with a per-reading
  idempotency key. A missing grant stops only that feature.
- **REM markers on the device:** follow the map with `mapNearbyRead` and `map.markers.nearby`,
  nearest first within the device's range. Rewrite only what changed. Operator shares
  (`mapSelectionRead`) remain supported.
- **NATO colours:** hostile red, unknown yellow, neutral green, friendly blue. Generic symbols
  use the device's default colour.

## Screen style

Use REM's Tactical Blue tokens from [design.md](../../design.md): dark surface `#020710`,
gradient panels with blue borders, primary `#64beff`, uppercase condensed headings and buttons
that invert when pressed, dark dialogs. Respect window insets, so nothing sits under the status
or gesture bar. The configuration web page REM opens follows the same tokens and keeps raw state
under "Technical details".

## Versions and releases

- One version value in the plugin's build file feeds the APK version, the `versionCode` and the
  manifest's `rem.plugin.version`. Every build handed out gets a new number.
- The plugin declares the lowest REM API minor it needs and only the capabilities it uses.
- Release builds are 16 KiB-aligned, signed with the release key, and recorded in the REM
  release manifest with their source commit.

## Verification

Synthetic tests, lint and builds prove software behaviour only. Each plugin's README keeps a
qualification checklist for the real device and records which rows were run on hardware.
