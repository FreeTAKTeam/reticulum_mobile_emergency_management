#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
: "${UNIFFI_TEST_JNA_JAR:?Set UNIFFI_TEST_JNA_JAR to a JNA 5.12.0+ jar}"
output="$(mktemp -d)"
trap 'rm -rf "$output"' EXIT
export CARGO_TARGET_DIR="$root/target/uniffi-test"

cargo build --manifest-path "$root/crates/reticulum_mobile/Cargo.toml" --locked
(
  cd "$root/crates/reticulum_mobile"
  cargo run --manifest-path "$root/tools/uniffi-bindgen/Cargo.toml" --locked -- \
    generate --no-format --language kotlin --language swift --language python \
    --out-dir "$output" src/reticulum_mobile.udl
)
cp "$CARGO_TARGET_DIR/debug/libreticulum_mobile.so" "$output/"

python3 - "$output" <<'PY'
from pathlib import Path
import sys

output = Path(sys.argv[1])
assert 'return "reticulum_mobile"' in (output / "uniffi/reticulum_mobile/reticulum_mobile.kt").read_text()
assert (output / "reticulum_mobile.swift").is_file()
assert (output / "reticulum_mobileFFI.h").is_file()
sys.path.insert(0, str(output))
import reticulum_mobile

assert reticulum_mobile.healthcheck() == "reticulum-mobile-ready"
PY

cat > "$output/Smoke.kt" <<'KOTLIN'
import uniffi.reticulum_mobile.Node
import uniffi.reticulum_mobile.healthcheck

fun main() {
    check(healthcheck() == "reticulum-mobile-ready")
    Node().use { node ->
        node.subscribeEvents().use { subscription ->
            subscription.closeSubscription()
            check(subscription.next(0u) == null)
        }
    }
}
KOTLIN
kotlinc "$output/uniffi/reticulum_mobile/reticulum_mobile.kt" "$output/Smoke.kt" \
  -classpath "$UNIFFI_TEST_JNA_JAR" -include-runtime -d "$output/smoke.jar"
(
  cd "$output"
  java -Djna.library.path="$output" -cp "$output/smoke.jar:$UNIFFI_TEST_JNA_JAR" SmokeKt
)
