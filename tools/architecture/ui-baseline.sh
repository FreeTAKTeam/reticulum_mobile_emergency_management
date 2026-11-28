#!/usr/bin/env bash
# Rebuild the REM UI dependency graph, DSM and report-only validation.
# Usage: tools/architecture/ui-baseline.sh <out-dir> [repo-root]
# Needs archgraph 0.1.0 (https://github.com/FreeTAKTeam/archgraph) on PATH or in $ARCHGRAPH.
set -euo pipefail

out="${1:?usage: ui-baseline.sh <out-dir> [repo-root]}"
here="$(cd "$(dirname "$0")" && pwd)"
root="${2:-$here/../..}"
archgraph="${ARCHGRAPH:-archgraph}"
policy="$here/../../docs/architecture/ui-dependency-baseline/rem-ui.policy.json"

mkdir -p "$out"
node "$here/ts-import-graph.mjs" --root "$root" \
  --source apps/mobile/src --source packages/node-client/src --test-source e2e \
  --alias @reticulum/node-client=packages/node-client/src/index.ts --alias /src=apps/mobile/src \
  --out "$out/graph.json"
"$archgraph" analyze --graph "$out/graph.json" --kinds import,type-import,dynamic-import \
  --policy "$policy" --out-dir "$out"
"$archgraph" validate --graph "$out/graph.json" --policy "$policy" --today 2026-11-28 \
  --out "$out/validation.json"

# File-level graph: cycles hidden inside one directory (no policy; layers are per directory).
mkdir -p "$out/files"
node "$here/ts-import-graph.mjs" --root "$root" --granularity file \
  --source apps/mobile/src --source packages/node-client/src --test-source e2e \
  --alias @reticulum/node-client=packages/node-client/src/index.ts --alias /src=apps/mobile/src \
  --out "$out/files/graph.json"
"$archgraph" analyze --graph "$out/files/graph.json" --kinds import,type-import,dynamic-import \
  --out-dir "$out/files"
