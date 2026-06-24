# JNI Host Tests Design

## Summary

Enable the existing `jni_utils` test suite to run on host (macOS/Linux) machines without an Android SDK. Currently the entire `droidplug` module is gated behind `#[cfg(target_os = "android")]`, but the `jni_utils` submodule only uses pure Java classes (no Android APIs). A new `jni-host-tests` cargo feature conditionally compiles a minimal `droidplug` shim on host targets, and a new script compiles the Java sources with plain `javac` to produce the jar the tests need.

## Definition of Done

1. A `jni-host-tests` cargo feature flag that conditionally compiles the `jni_utils` module (and its tests) on non-Android host targets
2. The gradle `testJar` task is fixed to produce compiled `.class` files instead of raw `.java` sources
3. A host test script that compiles only the gedgygedgy Java sources with plain `javac` (no Android SDK required), produces a jar, and runs the `jni_utils` tests via `cargo test`
4. The existing Android build path (`build-java.sh`) continues to work as before

**Out of scope:** Extracting `jni_utils` into its own crate, CI integration, testing on Android targets.

## Glossary

- **jni_utils**: `src/droidplug/jni_utils/` — Rust JNI bridge utilities wrapping the gedgygedgy Java interop library
- **gedgygedgy**: `io.github.gedgygedgy.rust.*` — pure Java classes providing Future, Stream, Waker, and ops abstractions for Rust-Java interop
- **droidplug**: `src/droidplug/` — the Android BLE backend module, currently gated by `#[cfg(target_os = "android")]`
- **btleplug-jni.jar**: Java archive containing compiled classes needed by jni_utils tests at runtime
- **host**: any non-Android target (macOS, Linux, Windows) where tests run on a desktop JVM

## Architecture

### Conditional Compilation Strategy

Use a minimal `droidplug` shim on non-Android targets to preserve the existing module hierarchy:

```rust
// src/lib.rs

// Full droidplug on Android (existing, unchanged)
#[cfg(target_os = "android")]
mod droidplug;

// Minimal shim on host — only jni_utils, no Android BLE code
#[cfg(all(not(target_os = "android"), feature = "jni-host-tests"))]
mod droidplug {
    pub mod jni_utils;
}
```

This preserves all `super::` path references within `jni_utils` modules without modifying any existing code.

### Dependency Changes

```toml
# Cargo.toml

[features]
jni-host-tests = ["jni", "once_cell"]

# Existing android-only deps (unchanged)
[target.'cfg(target_os = "android")'.dependencies]
jni = "0.19.0"
once_cell = "1.21.3"

# Host deps enabled only by feature flag
[target.'cfg(not(target_os = "android"))'.dependencies]
jni = { version = "0.19.0", optional = true }
once_cell = { version = "1.21.3", optional = true }
```

`lazy_static`, `futures`, and `static_assertions` are already unconditional deps (or dev-deps) — no changes needed.

### Gradle testJar Fix

The current `testJar` task copies `.java` source files instead of compiled `.class` files:

```gradle
// Current (broken) — copies source files
tasks.register('testJar', Jar) {
    archiveBaseName = 'btleplug-jni'
    from android.sourceSets.main.java.srcDirs
    destinationDirectory = file("$buildDir/libs")
}

// Fixed — uses compiled class output
tasks.register('testJar', Jar) {
    archiveBaseName = 'btleplug-jni'
    from compileDebugJavaWithJavac.destinationDirectory
    destinationDirectory = file("$buildDir/libs")
    dependsOn 'compileDebugJavaWithJavac'
}
```

### Host Test Script

New script at `scripts/run-jni-tests.sh`:

1. **JDK detection** — reuse patterns from existing `build-java.sh` (ensure_java, ensure_java_home)
2. **Compile** — `javac` only the gedgygedgy sources at `src/droidplug/java/src/main/java/io/github/gedgygedgy/**/*.java`
3. **Package** — create `target/debug/java/libs/btleplug-jni.jar` from compiled classes
4. **Test** — run `cargo test --features jni-host-tests`

No gradle, no Android SDK, no build.gradle involvement.

## Existing Patterns

- `scripts/build-java.sh` — existing script with JDK detection, JAVA_HOME resolution, and gradle build orchestration. The new script reuses JDK detection patterns.
- `src/droidplug/jni_utils/mod.rs` — contains `test_utils` module with `JVM_ENV` thread-local that creates a JVM and looks for `btleplug-jni.jar` relative to the test binary path (`target/debug/java/libs/`).
- `Cargo.toml` feature pattern — existing `serde` feature demonstrates optional dep enablement.

## Implementation Phases

### Phase 1: Cargo.toml and lib.rs changes

Add the `jni-host-tests` feature flag, optional host deps, and the conditional droidplug shim in `lib.rs`.

**Files:** `Cargo.toml`, `src/lib.rs`

**Verify:** `cargo check --features jni-host-tests` compiles without errors on host (tests will fail at runtime without the jar, but compilation should succeed).

### Phase 2: Fix gradle testJar

Update `build.gradle` testJar task to produce compiled `.class` files. Update `build-java.sh` if needed.

**Files:** `src/droidplug/java/build.gradle`

**Verify:** Run `scripts/build-java.sh` (requires Android SDK), inspect resulting jar contains `.class` files.

### Phase 3: Host test script

Create `scripts/run-jni-tests.sh` that compiles gedgygedgy Java sources with javac, packages a jar, and runs the cargo tests.

**Files:** `scripts/run-jni-tests.sh`

**Verify:** Run the script on a host machine with a JDK installed. All jni_utils tests pass.

## Additional Considerations

- **JNI crate version**: pinned to `0.19.0` to match the existing Android dependency
- **once_cell**: used by `classcache.rs` for lazy class caching — must be available on host
- **Test binary path**: the test_utils code resolves the jar path relative to `current_exe()`, navigating up to `target/debug/` then into `java/libs/`. The script must place the jar at this exact path.
- **Thread safety**: tests use `thread_local!` for `JVM_ENV` — only one JVM per process is allowed by JNI spec, which `lazy_static` handles correctly.
