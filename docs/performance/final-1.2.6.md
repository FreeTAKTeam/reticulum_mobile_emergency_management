# Version 1.2.6 Performance And Footprint Results

This closeout compares version 1.2.6 at `main` commit `d532d52` with the
pre-refactor baseline at `e38ffba`. Measurements use the same build commands,
supported ABIs, and two physical Android acceptance devices described in
[`baseline-e38ffba.md`](baseline-e38ffba.md).

## Source health

| Metric | Baseline | 1.2.6 |
| --- | ---: | ---: |
| First-party files checked | 180 | 451 |
| Files above 500 lines | 38 | 0 |
| Classes above 500 lines | 5 | 0 |
| File allowlist entries | 38 | 0 |
| Class allowlist entries | 5 | 0 |

The final count includes the new scale and list-window tests. Run `npm run check:source-size` to
enforce the empty allowlists.

## Web build

Command: `npm run web:build`

| Artifact | Baseline bytes | 1.2.6 bytes | Change |
| --- | ---: | ---: | ---: |
| Initial application JavaScript | 252,434 | 206,594 | -18.2% |
| Vue/vendor JavaScript | 140,548 | 140,604 | +0.0% |
| Lazy MapLibre JavaScript | 803,071 | 803,071 | 0.0% |
| All files under `dist/assets` | 1,744,131 | 1,710,181 | -1.9% |

The initial-bundle target of at least 15% is met. Platform-specific entrypoints
keep native and mock client implementations out of production web builds.

## Android release build

The signed 1.2.6 APK and AAB contain all supported ABIs and use 16 KB-aligned
native libraries.

| Artifact | Baseline bytes | 1.2.6 bytes | Change |
| --- | ---: | ---: | ---: |
| arm64-v8a `libreticulum_mobile.so` | 13,379,216 | 10,283,264 | -23.1% |
| armeabi-v7a `libreticulum_mobile.so` | 9,955,548 | 7,509,440 | -24.6% |
| x86_64 `libreticulum_mobile.so` | 13,646,704 | 11,441,432 | -16.2% |
| Signed universal APK | 33,127,536 | 33,674,057 | +1.6% |
| Release AAB | 32,016,205 | 32,628,089 | +1.9% |

The native-library target of at least 10% is exceeded for every ABI. The
universal APK/AAB target is intentionally not claimed: preserving Android 16 KB
page compatibility changes native packaging and offsets the raw library
reduction. Removing that compatibility or a supported ABI solely to meet the
container-size target would remove supported functionality.

## Scale and responsiveness

Executable tests cover:

- 1,000 peer projections;
- 10,000 message projections;
- 1,000 event and 1,000 telemetry projections;
- 100 checklists with 200 tasks each;
- a 1,000-command priority burst;
- normal-queue saturation while local, SOS, acknowledgement, and lifecycle
  capacity remains available;
- dense and sparse 1,000-position map clustering below the 50 ms long-task
  ceiling.
- shared 200-row render windows for peers, conversations, messages, events,
  action messages, checklists, and checklist tasks, with controls that retain
  access to every record.

The projection scale test enforces p95 below 500 ms. Bounded queues reject
excess normal work with a typed timeout, and network work remains asynchronous.
The list-window unit tests verify that 1,000-row collections never render more
than 200 rows at once and that final partial pages remain reachable.

## Physical-device sample

The signed release APK was installed with `adb install -r` on both devices.

| Device | Cold launches | Average | Warm launches | Average | PSS | Jank | Critical logs |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: |
| Pixel 7, Android 17 / SDK 37 | 502, 529, 498 ms | 509.7 ms | 126, 23, 30 ms | 59.7 ms | 456,070 KB | 4/254 (1.57%) | 0 |
| Samsung SM-G950W, Android 9 / SDK 28 | 920, 551, 516 ms | 662.3 ms | 96, 75, 66 ms | 79.0 ms | 287,763 KB | 68/195 (34.87%) | 0 |

The first Samsung cold launch is an outlier after replacement install; its next
two launches are close to baseline. Frame jank improved on both devices,
including 62.50% to 34.87% on the Android 9 device. Memory is a point-in-time
sample with retained production history and active Reticulum traffic, so it is
not treated as a controlled leak comparison.

## Paired-device acceptance

The Pixel and Samsung both reached READY and completed retained-history and
fresh-workflow checks for discovery/announces, direct LXMF messaging, delivery
acknowledgement, event command acceptance, checklist history, MapLibre,
background/foreground restoration, manual sync, offline transport failure,
automatic reconnect, and SOS activation/cancellation. The controlled Samsung
offline cycle kept the core runtime ready, reported the failed TCP interface,
and recovered after restoring its original Wi-Fi and mobile-data settings.

No crash, ANR, JNI failure, or uncaught exception was found in the acceptance
log samples.
