# Standard Telemeter interoperability

Sideband is the reference implementation. RCH's `r3akt-map-core` owns both
encoding and checked decoding; REM's native `telemeter` module only maps its
application records and units to that contract. No TypeScript wire codec was
added. Map objects remain separate from position telemetry.

## Ordinary positions and SOS

Ordinary position messages carry standard `FIELD_TELEMETRY` (`0x02`) binary
MessagePack and retain the compact REM `T1` command during migration. Field 2
is authoritative for received readings; T1 retains the REM callsign. A standard
message without T1 uses the authenticated envelope's full source destination
as its stable position key. Legacy T1-only messages remain accepted. Numeric
collector command `1` is distinct from the string `T1`.

SOS uses the same shared encoder: big-endian binary coordinates, signed altitude
in centimetres, accuracy in centimetres, and battery percentage (0–100).
The location timestamp is the source fix time, with Telemeter's whole-second
precision; receiving a delayed sample does not make it fresh. Existing
monotonic persistence continues rejecting older replacements. Legacy REM's
ordinary position path remains supported through T1; old nonstandard SOS
sensor arrays are no longer emitted or treated as standard Telemeter.

REM's device/location APIs use m/s. The native adapter converts to/from
Sideband's km/h wire unit. **Columba 2.2.6 is an explicit exception:** its codec
stores m/s in the same field. The byte stream cannot identify which convention
the sender used. Configuring its contact profile selects that conversion;
unconfigured senders use the Sideband standard. RCH preserves the numeric wire
reading and original bytes; it does not guess a sender's speed profile.

## Permission-controlled contact delivery

Settings → Location provides separate Sideband-standard and Columba 2.2.6
contact lists. Enter saved contacts' full LXMF delivery hashes, one per line.
Both lists default to empty. Sharing must be enabled, the fix must remain
within its configured lifetime, and the destination must still be a saved
LXMF contact. Team routing retains its existing authorization rules.

Removing a destination revokes its explicit grant. Unsaving the contact stops
delivery while it is unsaved. Native enqueue and retry admission recheck the
current settings and saved-contact state; a cached route is not permission.
A destination cannot appear in both profiles. Profile changes invalidate old
queued payload identifiers. Transport receipts and REM application acceptance
remain distinct; third-party clients need not return REM application ACKs.

## Collectors

RCH accepts numeric command `1` in `FIELD_COMMANDS` (`0x09`), with seconds or
`[seconds, collector_flag]`. Replies use `FIELD_TELEMETRY_STREAM` (`0x03`) as a
plain list of `[source_hash_bytes, timestamp_seconds, packed_telemeter_bytes,
appearance]`; the list itself must not be encoded as another binary blob.
Existing RCH topic, subscriber and identity policies still apply. REM does not
become a collector or reinterpret T1 as a query. The ordinary receive path in
this change handles field 2; it does not ingest collector stream history.

## Marker interoperability

RCH and REM use `r3akt_map_core::marker_wire::encode_fields` for marker
upserts. The shared numeric field `0x03` stream contains the marker's binary
object hash, original source timestamp, packed Telemeter and icon appearance.
The marker is separate from the envelope sender and its GPS position. Field
`0x0D` retains the full R3AKT replica; REM's existing field `0x09` mission
commands remain present. Routing, recipient selection and TEAM authorization
are unchanged.

RCH's map **Send** action explicitly queues a marker to an entered LXMF
delivery address, including Sideband, Columba or REM. It uses the existing
outbound permission checks and durable queue. Sideband's telemetry receive policy must permit the hub; its separate
trusted-only map display filter also applies to the independent marker hash. Name and notes are carried in the Information
sensor; third-party applications control their own map labels. Marker
altitude/speed/bearing/accuracy are zero because the marker model contains
only latitude/longitude. Ordinary GPS telemetry retains its actual readings.

R3AKT deletion remains an application event: the standard stream has no
delete operation for Sideband or Columba. Explicit sends do not add
subscriptions or marker collector history. Pyxis marker support is excluded
at the user's request; its existing location byte fixtures remain regression
coverage for the shared codec.

## Synthetic verification

Synthetic coordinates are used for source-level and local LXMF tests.

- Sideband `2000d81a`: original `Telemeter.packed`, `Location.pack` and
  `Location.unpack` execute with synthetic readings.
- Pyxis `5181fdd7`: unmodified C++ codec compiles and encodes/decodes all five
  upstream byte vectors, including GPS bounds, negative altitude, km/h,
  distinct sensor/fix timestamps, and 64-bit timestamps.
- Columba 2.2.6 `a402ed10`: unmodified Kotlin codec executes on JVM using
  Kotlin 2.3.20 and MessagePack 0.9.10, with an Android-free model stub.
- Shared Rust encoding must exactly match those source-pinned bytes. REM
  decodes the same fixture shapes, converts units explicitly, and exercises
  field-2-only receipt and persistence through a local two-node LXMF pair.
- RCH broker tests persist uploads, ensure T1 emits no reply, and verify
  numeric-command-1 stream replies retain the original bytes and timestamps.
- REM tests cover SOS, saved-contact authorization, migration T1, native
  delivery and restart, plus settings save/revocation in desktop/mobile web.

These checks do not establish Android GPS acquisition, real-peer radio
delivery, deployed collector permissions, or firmware behavior.

### Verification results (2026-10-30)

REM pins the shared RCH crates to commit
[`979e8fbb`](https://github.com/FreeTAKTeam/Reticulum-Community-Hub/commit/979e8fbb313a4a280a16bcd6f42b309cd5478ed2),
proposed in [RCH PR #290](https://github.com/FreeTAKTeam/Reticulum-Community-Hub/pull/290) and not yet merged.

- REM `cargo test --lib --no-fail-fast`: **510 passed** using an
  isolated source copy with the existing CI LXMF 0.14.0 baseline
  (`7822b608f2cd2debd8170895bc580d01dda5356e`). The root workspace's sibling
  LXMF checkout is 0.16.0; its unrelated changes were preserved. An earlier
  development run against that checkout passed all 508 then-present tests.
- REM `cargo clippy --locked --all-targets -- -D warnings` and `cargo fmt
  -- --check`: passed. The baseline dependencies emit existing warnings;
  REM itself passes the strict lint gate.
- `npm run test:unit`, `npm --workspace apps/mobile run typecheck`,
  `npm run web:build`, and `npm run check:source-size`: passed; the size gate
  checked 683 files with no exceptions.
- `npx playwright test e2e/telemetry.spec.ts`: **14 passed** across desktop
  and mobile Chromium, including grant save, validation and revocation.
- RCH source-oracle scripts for Sideband/Pyxis, Columba and collector replies
  passed. The Sideband/Pyxis runner checked all five byte vectors in both
  directions against the original Python and C++ implementations.
- After the marker change, `cargo test --workspace -- --test-threads=1`
  passed: **725 passed, 2 existing tests ignored**. Formatting, strict Clippy
  and the Rust module-size gate passed. System GCC/G++ were selected with
  absolute paths because this machine's PATH contains Clang wrappers.
- RCH UI lint, typecheck, build and **210 tests** passed. The new Send dialog
  tests cover explicit submission, rejection, invalid receipts and duplicate
  clicks. REM's existing marker Playwright suite passed **4 tests** across
  desktop and mobile Chromium.

Kotlin UniFFI bindings were regenerated from the updated UDL into the ignored
Android output directory. Test-only Android libraries and isolated APKs were
subsequently built for both connected phones. The earlier ordinary-position
device attempts did not establish successful interoperability; the original REM packages and settings were restored. The
subsequent marker phone test results are below. No iOS build, deployment, merge or release was performed.

### Marker phone results (2026-10-30)

Both connected phones used the isolated package
`network.reticulum.emergency.telemeter`, version
`1.5.5-marker-hil-979e8fb`, built with LXMF 0.16.0 and shared map-core
`979e8fbb`. Their ordinary REM packages and settings were left untouched.
Tests used synthetic markers over a local TCP Reticulum relay. Normal local
TEAM membership was configured in the isolated apps; no routing bypass was
introduced.

- Pixel REM → POCO REM and POCO REM → Pixel REM: received records match the
  created marker, including identity, name, coordinates and timestamps.
- RCH's actual Share API → REM on both phones: marker persisted separately
  from sender positions, with the same object hash and source time.
- RCH → Sideband 2.1.0 on Pixel: the map pin and details were visually verified.
  Sideband showed the fire icon, `44.5, -1.25`, Information sensor, original
  source age and RCH as the forwarding peer. Its trusted-only map display
  filter was temporarily disabled for the synthetic object, then restored.
- RCH → Columba 2.3.1-beta-kt on POCO: the native receive/storage log confirms
  the same object hash and `44.5, -1.25`. This installed build reports
  `appearance=false`; its marker icon/map rendering was not visually proven.
  The pinned source oracle separately verifies the newer stream decoder's
  appearance handling. No Columba behavior or firmware was changed.

Evidence is retained locally in `target/telemeter-phone-test/`:
`marker-phone-results.json`, `marker-*-created.json`, `marker-*-received.json`,
`marker-*-from-rch.json`, `marker-sideband-details.png` and
`marker-columba-received.log`. These are ignored test artifacts, not release
assets. The temporary Columba TCP interface, test processes and ADB forwards
were removed; the Pixel display density was restored. Synthetic received
markers may remain in the test clients' history.

This verifies the marker exchange paths above. Negative GPS altitude, speed
units, ordinary-position source age and numeric collector responses have
source-level/local LXMF regression coverage, **not a completed live phone
matrix**. No radio-range, real GPS acquisition, iOS, deployment or release
qualification is claimed. Pyxis remains excluded.

Sources: [Sideband](https://github.com/markqvist/Sideband/blob/2000d81a44bff57e3b3cb7d45915ba29bbde4e18/sbapp/sideband/sense.py),
[Pyxis vectors](https://github.com/torlando-tech/pyxis/blob/5181fdd7b589e544b5825fbf1c631c4d79a7039d/tests/fixtures/location_telemetry_vectors.json),
[Columba codec](https://github.com/torlando-tech/columba/blob/a402ed1052ba6993454352ce51ddfc336b729d9f/rns-api/src/main/java/network/columba/app/rns/api/util/TelemeterCodec.kt).

### Marker display and offline persistence follow-up

REM uses the same 20 display colors as RCH, including NATO friendly blue,
hostile red, neutral green and unknown yellow. Map pins display their symbols
without surrounding tiles; selection retains the symbol color. The picker,
placement preview and details use the same palette.

The Pixel reported `DatabaseBusy` while saving a marker. The standalone
projection-revision transaction now acquires the SQLite writer lock before
reading the revision, allowing the busy timeout to wait for concurrent writes
instead of failing during a deferred read-to-write upgrade. A contention test
fails before the fix and passes after it. A separate native test creates,
renames, moves, reopens and deletes markers without TCP clients and with
telemetry disabled. Routing and telemetry settings are unaffected.

The updated test APK was installed on both phones. Pixel create, rename,
move, persisted readback and deletion passed. On request, the isolated test
package was subsequently uninstalled on both phones and the original REM
installations reopened with their original data retained.

The PR is based on REM main `897c78b`, preserving its custody/routing and
offline-plugin changes. Current-main checks include 525 passing Rust tests, Rust 1.99.1 formatting and
strict Clippy, JS unit checks, typecheck, web/mobile builds, the source-size
gate (706 files), and 20 marker/telemetry Playwright tests. Physical phone
results above used the earlier isolated build, not this current-main
integration. No native generated files or APKs are committed.
