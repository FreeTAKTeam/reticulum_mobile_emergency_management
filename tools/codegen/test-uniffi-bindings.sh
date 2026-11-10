#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
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
