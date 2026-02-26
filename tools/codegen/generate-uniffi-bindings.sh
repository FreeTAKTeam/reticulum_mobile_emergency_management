#!/usr/bin/env bash
set -euo pipefail

LANGUAGE="${1:-swift}"
OUT_DIR="${2:-apps/mobile/ios}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
UDL_PATH="$REPO_ROOT/crates/reticulum_mobile/src/reticulum_mobile.udl"
OUT_PATH="$REPO_ROOT/$OUT_DIR"

uniffi-bindgen generate "$UDL_PATH" --language "$LANGUAGE" --out-dir "$OUT_PATH"
