# Performance and Footprint Baseline

This baseline was captured on 2026-07-14 from `main` commit
`e38ffba40774bb5ee4e99445a06b1837790b3d21`, before runtime, dependency,
or UI refactoring. The source-size gate and test-runner metadata do not alter
application behavior.

## Source health

- First-party source and test files checked: 180
- Files above 500 physical lines: 38
- Class declarations above 500 physical lines: 5
- Largest files:
  - `crates/reticulum_mobile/src/runtime.rs`: 15,012 lines
  - `crates/reticulum_mobile/src/node.rs`: 13,044 lines
  - `crates/reticulum_mobile/src/app_state.rs`: 5,472 lines
  - `crates/reticulum_mobile/src/jni_bridge.rs`: 5,004 lines
  - `packages/node-client/src/index.ts`: 4,555 lines

Generated bindings, build output, vendored source, copied third-party Android
code, and the Stitch visual-reference scripts are excluded from the gate.
File and class exceptions are tracked independently so an oversized file cannot
hide a new or growing oversized class.

## Web build

Command: `npm run web:build`

| Artifact | Bytes |
| --- | ---: |
| Initial application JavaScript | 252,434 |
| Vue/vendor JavaScript | 140,548 |
| Lazy MapLibre JavaScript | 803,071 |
| All files under `dist/assets` | 1,744,131 |

## Android release build

The UniFFI script rebuilt all three Android targets before Gradle assembled the
signed release. The APK and AAB contain all supported ABIs.

| Artifact | Bytes |
| --- | ---: |
| arm64-v8a `libreticulum_mobile.so` | 13,379,216 |
| armeabi-v7a `libreticulum_mobile.so` | 9,955,548 |
| x86_64 `libreticulum_mobile.so` | 13,646,704 |
| Signed universal APK | 33,127,536 |
| Release AAB | 32,016,205 |

## Physical-device launch sample

The same signed `1.2.5` APK (`versionCode=261881978`) was installed with
`adb install -r` on both devices. Each cold measurement force-stopped the app;
each warm measurement resumed it from the launcher. Memory and graphics values
are point-in-time `dumpsys` samples after launch.

| Device | Cold launches (ms) | Average | Warm launches (ms) | Average | Memory | Jank | Critical logs |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: |
| Pixel 7, Android 17 / SDK 37 | 495, 497, 500 | 497.3 ms | 0, 0, 0 | 0 ms | 354,003 KB PSS | 5/202 (2.48%) | 0 |
| Samsung SM-G950W, Android 9 / SDK 28 | 515, 484, 520 | 506.3 ms | 14, 68, 78 | 53.3 ms | 260,315 KB PSS | 85/136 (62.50%) | 0 |

The Samsung jank result is the primary device-performance regression target.
Launch measurements cover process/activity startup only; feature-level latency
is covered by deterministic stress tests and final paired-device workflows.

Repeat the device sample with:

```powershell
./tools/performance/measure-android.ps1 `
  -Serial 35031FDH2003N8,988b9b344135304639
```
