# codegen

UniFFI helper scripts for generating platform bindings from
`crates/reticulum_mobile/src/reticulum_mobile.udl`.

## Usage

PowerShell:

```powershell
./tools/codegen/generate-uniffi-bindings.ps1 -Language kotlin
./tools/codegen/generate-uniffi-bindings.ps1 -Language swift
```

Shell:

```bash
./tools/codegen/generate-uniffi-bindings.sh kotlin
./tools/codegen/generate-uniffi-bindings.sh swift
```

Each script:

- Builds `reticulum_mobile` for platform targets (Android or iOS)
- Runs `uniffi-bindgen` for the chosen language
- Copies generated bindings into `apps/mobile/{android|ios}/uniffi`
- Copies built native libraries into `apps/mobile/{android|ios}/uniffi/libs/<target>`
- For Android (`kotlin`), also copies `libreticulum_mobile.so` into:
  - `apps/mobile/android/app/src/main/jniLibs/arm64-v8a`
  - `apps/mobile/android/app/src/main/jniLibs/armeabi-v7a`
  - `apps/mobile/android/app/src/main/jniLibs/x86_64`

## Android Prerequisites

- Android NDK toolchains must be available in `PATH` (`clang` plus Android target clang wrappers).
- Rust Android targets are installed by the script (`rustup target add ...`).
- If toolchains are missing, Cargo cross-compiles will fail before `.so` copy.

The scripts auto-detect SDK from `ANDROID_SDK_ROOT`/`ANDROID_HOME` (or default SDK paths).  
For NDK, precedence is:

1. `ANDROID_NDK_HOME`
2. `NDK_HOME`
3. `apps/mobile/android/local.properties` (`ndk.dir`, if present)
4. Latest side-by-side NDK in `<sdk>/ndk`

Then they set linker and `CC_*` env vars for Android targets, build, and copy `.so` files into `jniLibs`.
