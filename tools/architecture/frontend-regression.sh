#!/usr/bin/env bash
# Run the frontend suite on a clean checkout of one commit, with locked dependencies.
# Usage: tools/architecture/frontend-regression.sh <commit> <work-dir> [--e2e]
# Writes <work-dir>/<short-sha>/results.tsv (step, exit code, seconds) and one log per step.
set -uo pipefail

commit="${1:?usage: frontend-regression.sh <commit> <work-dir> [--e2e]}"
work="${2:?usage: frontend-regression.sh <commit> <work-dir> [--e2e]}"
e2e="${3:-}"
repo="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
sha="$(git -C "$repo" rev-parse --short=12 "$commit")"
out="$work/$sha"
tree="$out/tree"

mkdir -p "$out"
git -C "$repo" worktree add --detach "$tree" "$commit" >/dev/null || exit 2
trap 'git -C "$repo" worktree remove --force "$tree"' EXIT

{
  printf 'commit\t%s\n' "$(git -C "$tree" rev-parse HEAD)"
  printf 'node\t%s\nnpm\t%s\n' "$(node -v)" "$(npm -v)"
  printf 'lockfile_sha256\t%s\n' "$(sha256sum "$tree/package-lock.json" | cut -d' ' -f1)"
} > "$out/environment.tsv"

printf 'step\texit\tseconds\n' > "$out/results.tsv"
run() {
  local name="$1"; shift
  local start=$SECONDS
  (cd "$tree" && "$@") > "$out/$name.log" 2>&1
  local code=$?
  printf '%s\t%s\t%s\n' "$name" "$code" "$((SECONDS - start))" >> "$out/results.tsv"
  return $code
}

run npm-ci npm ci --no-audit --no-fund || exit 1
run unit npm run test:unit
run typecheck npm --workspace apps/mobile run typecheck
run web-build npm run web:build
run source-size npm run check:source-size
if [ "$e2e" = "--e2e" ]; then
  run e2e npx playwright test --reporter=line
fi
(cd "$tree" && npx vitest --version && npx playwright --version) > "$out/tool-versions.txt" 2>&1
cat "$out/results.tsv"
