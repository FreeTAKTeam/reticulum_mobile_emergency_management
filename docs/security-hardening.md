# Local cache and plugin hardening

Browser deployments keep operational projections in page memory: chat messages,
EAMs, events, telemetry, saved peers, hub linkage, and notification activity.
These values survive route changes but disappear on reload or tab closure. Browser
settings and UI preferences remain persistent; the hub API key remains in the
current page's settings only and is neither written nor restored from browser
settings. Re-enter it after reload when needed.

Older browser localStorage operational caches are ignored, not deleted or migrated
into page memory. Historical records and old API keys can therefore still exist on
disk. Operators who no longer need them should explicitly clear this site's stored
data in their browser settings; export any authored records they need first. This
also clears other settings. Page memory is accessible to scripts running in the
page; this change reduces durable plaintext exposure and does not protect against
an active script injection.

Android's native runtime remains the authority for durable operational data. The
existing WebView legacy import reads old caches and removes them only after native
verification; native settings and credentials are not stripped by browser rules.
This change does not add native database encryption.

The opt-in ADB test receiver requires Android's `android.permission.DUMP`, a
signature/privileged/development permission held by ADB shell but unavailable to
ordinary installed apps. The build flag remains disabled by default.

BLE heart-rate sampling and sharing intervals use Android elapsed realtime, while
published sample timestamps keep wall-clock time. Incoming sample timestamps are
clamped within five minutes of the local wall clock. Incoming-event parse errors
identify that operation in logs.

The destination scheduler drops idle per-destination lock registry entries when
obtaining another lock; active holders and waiters keep the same lock. Tap detection
uses the existing effective 260 ms debounce without a misleading unused walking
constant. This preserves behavior and does not claim walking discrimination or
hardware gesture calibration. SOS case-variant alert replacement already used a
normalized identity on this revision and now has regression coverage. Browser mock
identities use the platform cryptographic random source.
