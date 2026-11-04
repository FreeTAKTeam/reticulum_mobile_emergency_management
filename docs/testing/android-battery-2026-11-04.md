# REM Android battery investigation — 2026-11-04

The connected Pixel 7 running REM 1.5.7 attributed the heavy CPU work to
`network.reticulum.emergency`, specifically its Rust `rem-node` workers.
Android's initial CPU report showed 96% of one core; subsequent five-second
thread samples showed about 44–77%. The Java event and notification workers
were nearly idle. The phone was USB powered, so these are CPU measurements,
not a battery-discharge or battery-life estimate.

## Attribution

The production APK disallows native sampling. An isolated checkout of release
`1.5.7` (`5ef033e1814bdda44f8d7d40e172d0173e371fea`) was built with Android's
temporary `profileable` manifest element. Its packaged Rust library and web
assets were verified byte-for-byte against the installed APK before profiling.
The original APK was retained, and updates preserved application data.

Two 20-second simpleperf captures identified the startup path:

```text
runtime::run_node
  MessagingStore::record_announce
    MessagingStore::replace_announce_for_identity
      full announce-table scan, normalize_hex, allocation, comparison, free
```

In the first capture, the worker accounted for 97.77% of sampled cycles;
99.75% of its runtime work was under `record_announce`, predominantly the
replacement scan. Replaying a history of N distinct announces performed
quadratic work. Live announces also scanned all retained history.

After removing that scan, another device capture found the operational hotspot:
managed-peer reconnects and peer queries repeatedly called `list_peers`, which
cloned all historical announces and decoded their capabilities. Relay selection
also cloned and sorted the full history to select one propagation node.

## Change

- Index received announces by normalized identity and destination kind, separately
  from resolution results, so a changed resolution cannot hide an old announce.
- Maintain the set of mission-capable LXMF announces when an announce is admitted,
  using the existing native capability decoder after metadata preservation.
  Peer projections inspect that set and saved peers instead of all mesh history.
- Select the automatic relay from borrowed records using the existing complete
  ordering key. The UI's full, sorted announce history remains available.

The change does not delete stored history, change wire formats, introduce a new
protocol decoder, or alter delivery/retry intervals. Staleness is still evaluated
at query time. No generated bindings or frontend sources changed.

## Reproduction

Run the ignored performance fixtures in release mode:

```sh
cargo test --manifest-path crates/reticulum_mobile/Cargo.toml --release --lib \
  announce_history_performance -- --ignored --nocapture
cargo test --manifest-path crates/reticulum_mobile/Cargo.toml --release --lib \
  peer_projection_history_performance -- --ignored --nocapture
```

The initial host measurements were 4.177 seconds for 20,000 announce inserts and
685.6 milliseconds for 1,000 updates. The index-only intermediate version took
57.5 milliseconds and 1.37 milliseconds respectively. Before the peer projection
change, 100 projections over 20,000 announces (10 mission peers) took 2.786 seconds.
These synthetic measurements explain scaling; they are not phone battery savings.

Regression coverage includes metadata retention, old-update rejection, identity
replacement after independent resolution changes, destination reassignment, and
capability membership changes. The existing native suite exercises peer aliases,
staleness, propagation selection, messaging, and telemetry.

Local captures, build/test logs, and APKs are retained under the ignored
`output/battery-2026-11-04/` directory. Raw captures are not committed.

The active development checkout initially failed to compile in unrelated
`src/node/plugin_relay.rs:104` because it references `chrono` without declaring
that dependency. Device builds therefore use the exact released REM and LXMF
revisions plus the battery fix, keeping unrelated in-progress work off the phone.

## Final verification

| Check | Result |
| --- | --- |
| Release native suite (`cargo test --release --lib`) | 529 passed; two manual performance fixtures ignored |
| Both manual release performance fixtures | Passed separately |
| Final 20,000-announce replay / 1,000 updates | 37.68 ms / 1.33 ms |
| Final 100 peer projections over 20,000 announces | 0.969 ms |
| Android arm64 release build and signed APK | Passed |
| Source-size gate | Passed, 750 first-party files |
| Touched-module formatting and diff checks | Passed |

The final 20-second simpleperf capture recorded 4,923,152,127 CPU cycles versus
46,158,171,384 in the original startup capture (about 89% fewer). The intermediate
announce-index-only build still consumed 51,170,087,258 cycles while serving peer
queries; this is why the peer projection and relay-selection changes are included.
These live windows have different startup/network activity and are not an
equal-work energy benchmark. The deterministic host fixtures isolate the scaling
changes.

After installing the final build and returning to the Android home screen, six
follow-up five-second process samples ranged from 10.6% to 20.4% of one CPU core
(mean 16.5%). The retained capture is `rem-top-installed-settled.txt`; its first
sample is excluded from that summary. This is a short observation with the
configured network active, not a claim of negligible background drain.

Installed build: `1.5.7-battery.1`, original version code `263031829`, arm64 only,
signed with the existing REM release identity. The final manifest has no
`profileable` element. Original application assets, including bundled plugins,
were checked for equality; the public release was not changed. Both the fixed
APK and `rem-1.5.7-original.apk` are retained in the local evidence directory.

No discharge-duration test, long soak, physical-radio delivery qualification, or
iOS build was performed. RNode BLE connection timeouts were visible during the
session; the optimization does not claim to fix that separate connectivity issue.

Android profiling mechanism:
[profileable manifest element](https://developer.android.com/guide/topics/manifest/profileable-element).
