#!/usr/bin/env bash
set -euo pipefail

LANGUAGE="${1:-swift}"
OUT_DIR="${2:-}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
UDL_PATH="$REPO_ROOT/crates/reticulum_mobile/src/reticulum_mobile.udl"
TMP_OUT="$REPO_ROOT/target/uniffi/$LANGUAGE"

if [[ -z "$OUT_DIR" ]]; then
  if [[ "$LANGUAGE" == "kotlin" ]]; then
    OUT_DIR="apps/mobile/android/uniffi"
  else
    OUT_DIR="apps/mobile/ios/uniffi"
  fi
fi

if [[ "$LANGUAGE" == "kotlin" ]]; then
  SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Android/Sdk}}"
  if [[ ! -d "$SDK_ROOT" ]]; then
    echo "Android SDK not found at $SDK_ROOT" >&2
    exit 1
  fi
  NDK_ROOT="$SDK_ROOT/ndk"
  if [[ ! -d "$NDK_ROOT" ]]; then
    echo "Android NDK not found at $NDK_ROOT" >&2
    exit 1
  fi

  NDK_DIR=""
  if [[ -n "${ANDROID_NDK_HOME:-}" && -d "${ANDROID_NDK_HOME}" ]]; then
    NDK_DIR="${ANDROID_NDK_HOME}"
  elif [[ -n "${NDK_HOME:-}" && -d "${NDK_HOME}" ]]; then
    NDK_DIR="${NDK_HOME}"
  else
    LOCAL_PROPERTIES="$REPO_ROOT/apps/mobile/android/local.properties"
    if [[ -f "$LOCAL_PROPERTIES" ]]; then
      NDK_LINE="$(grep -m1 '^ndk\.dir=' "$LOCAL_PROPERTIES" || true)"
      if [[ -n "$NDK_LINE" ]]; then
        CANDIDATE="${NDK_LINE#ndk.dir=}"
        CANDIDATE="${CANDIDATE//\\\\/\\}"
        if [[ -d "$CANDIDATE" ]]; then
          NDK_DIR="$CANDIDATE"
        fi
      fi
    fi
  fi

  if [[ -z "$NDK_DIR" ]]; then
    NDK_DIR="$(find "$NDK_ROOT" -mindepth 1 -maxdepth 1 -type d | sort -r | head -n1)"
  fi
  if [[ -z "$NDK_DIR" ]]; then
    echo "No NDK versions found under $NDK_ROOT" >&2
    exit 1
  fi
  NDK_BIN="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin"
  if [[ ! -d "$NDK_BIN" ]]; then
    NDK_BIN="$NDK_DIR/toolchains/llvm/prebuilt/darwin-x86_64/bin"
  fi
  if [[ ! -d "$NDK_BIN" ]]; then
    NDK_BIN="$NDK_DIR/toolchains/llvm/prebuilt/darwin-arm64/bin"
  fi
  if [[ ! -d "$NDK_BIN" ]]; then
    echo "NDK toolchain bin not found under $NDK_DIR/toolchains/llvm/prebuilt" >&2
    exit 1
  fi

  export ANDROID_HOME="$SDK_ROOT"
  export ANDROID_SDK_ROOT="$SDK_ROOT"
  export ANDROID_NDK_HOME="$NDK_DIR"
  export NDK_HOME="$NDK_DIR"
  export PATH="$NDK_BIN:$SDK_ROOT/platform-tools:$SDK_ROOT/cmdline-tools/latest/bin:$PATH"
  export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDK_BIN/aarch64-linux-android21-clang"
  export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$NDK_BIN/armv7a-linux-androideabi21-clang"
  export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$NDK_BIN/x86_64-linux-android21-clang"
  export CC_aarch64_linux_android="$NDK_BIN/aarch64-linux-android21-clang"
  export CC_armv7_linux_androideabi="$NDK_BIN/armv7a-linux-androideabi21-clang"
  export CC_x86_64_linux_android="$NDK_BIN/x86_64-linux-android21-clang"

  echo "Using Android SDK: $SDK_ROOT"
  echo "Using Android NDK: $NDK_DIR"

  TARGETS=(
    "aarch64-linux-android"
    "armv7-linux-androideabi"
    "x86_64-linux-android"
  )
else
  TARGETS=(
    "aarch64-apple-ios"
    "aarch64-apple-ios-sim"
    "x86_64-apple-ios-sim"
  )
fi

TARGET_OUT="$REPO_ROOT/$OUT_DIR"
mkdir -p "$TMP_OUT" "$TARGET_OUT"

echo "Building reticulum_mobile for $LANGUAGE targets..."
for target in "${TARGETS[@]}"; do
  rustup target add "$target" >/dev/null
  cargo build -p reticulum_mobile --release --target "$target"
done

if command -v uniffi-bindgen >/dev/null 2>&1; then
  echo "Generating UniFFI bindings ($LANGUAGE)..."
  uniffi-bindgen generate "$UDL_PATH" --language "$LANGUAGE" --out-dir "$TMP_OUT"

  echo "Copying generated bindings to $TARGET_OUT..."
  cp -R "$TMP_OUT"/. "$TARGET_OUT"/
elif [[ "$LANGUAGE" == "kotlin" ]]; then
  echo "uniffi-bindgen not found; skipping Kotlin binding generation. Native .so files will still be copied."
else
  echo "uniffi-bindgen not found in PATH" >&2
  exit 1
fi

echo "Copying built native libraries..."
for target in "${TARGETS[@]}"; do
  RELEASE_DIR="$REPO_ROOT/target/$target/release"
  if [[ ! -d "$RELEASE_DIR" ]]; then
    continue
  fi
  LIB_DIR="$TARGET_OUT/libs/$target"
  mkdir -p "$LIB_DIR"
  find "$RELEASE_DIR" -maxdepth 1 -type f -name "libreticulum_mobile*" -exec cp {} "$LIB_DIR"/ \;

  if [[ "$LANGUAGE" == "kotlin" ]]; then
    ABI=""
    case "$target" in
      aarch64-linux-android) ABI="arm64-v8a" ;;
      armv7-linux-androideabi) ABI="armeabi-v7a" ;;
      x86_64-linux-android) ABI="x86_64" ;;
    esac
    if [[ -n "$ABI" ]]; then
      JNI_DIR="$REPO_ROOT/apps/mobile/android/app/src/main/jniLibs/$ABI"
      mkdir -p "$JNI_DIR"
      find "$RELEASE_DIR" -maxdepth 1 -type f -name "libreticulum_mobile.so" -exec cp {} "$JNI_DIR"/ \;
    fi
  fi
done

echo "UniFFI generation complete."
