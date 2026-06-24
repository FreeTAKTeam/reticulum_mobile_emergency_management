#!/usr/bin/env bash
#
# Run each integration test individually to avoid multiple simultaneous
# BLE connections to the same test peripheral.
#
# Each test_*.rs file under tests/ is its own binary with a single test,
# ensuring process isolation for BLE stack stability.
#
# Usage:
#   ./scripts/run-integration-tests.sh              # run all tests
#   ./scripts/run-integration-tests.sh test_read_*   # run tests matching a glob
#
# Environment:
#   BTLEPLUG_TEST_PERIPHERAL  - peripheral name (default: btleplug-test)
#   RUST_LOG                  - log level (e.g. debug, btleplug=trace)
#   DELAY                     - seconds to wait between tests (default: 2)
#   TIMEOUT                   - seconds before a test is killed (default: 20)

set -euo pipefail

DELAY="${DELAY:-2}"
TIMEOUT="${TIMEOUT:-20}"
PASSED=0
FAILED=0
FAILURES=()

# Discover all test binaries (one per file).
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TESTS_DIR="$(cd "$SCRIPT_DIR/../tests" && pwd)"

# Build list of test names from test_*.rs files (excluding the common/ module).
TEST_NAMES=()
for f in "$TESTS_DIR"/test_*.rs; do
  name="$(basename "$f" .rs)"
  # If a filter was provided, apply it as a glob.
  if [[ $# -gt 0 ]]; then
    matched=false
    for pattern in "$@"; do
      # shellcheck disable=SC2254
      case "$name" in $pattern) matched=true ;; esac
    done
    if ! $matched; then
      continue
    fi
  fi
  TEST_NAMES+=("$name")
done

if [[ ${#TEST_NAMES[@]} -eq 0 ]]; then
  echo "No tests matched."
  [[ $# -gt 0 ]] && echo "Filter: $*"
  exit 1
fi

total=${#TEST_NAMES[@]}
echo "=== btleplug integration tests ==="
echo "Running $total tests sequentially (${DELAY}s delay, ${TIMEOUT}s timeout per test)"
echo ""

test_num=0
for test_name in "${TEST_NAMES[@]}"; do
  test_num=$((test_num + 1))
  printf "[%2d/%2d] %-55s " "$test_num" "$total" "$test_name"

  if timeout "${TIMEOUT}s" cargo test --test "$test_name" -- --ignored 2>/tmp/btleplug-test-output.log; then
    echo "PASS"
    PASSED=$((PASSED + 1))
  else
    exit_code=$?
    if [[ $exit_code -eq 124 ]]; then
      echo "TIMEOUT (${TIMEOUT}s)"
    else
      echo "FAIL"
    fi
    FAILED=$((FAILED + 1))
    FAILURES+=("$test_name")
    # Show output for failed tests.
    echo "  --- output ---"
    sed 's/^/  /' /tmp/btleplug-test-output.log | tail -20
    echo "  --- end ---"
  fi

  # Brief delay to let the BLE stack settle between tests.
  if [[ $test_num -lt $total ]]; then
    sleep "$DELAY"
  fi
done

rm -f /tmp/btleplug-test-output.log

echo ""
echo "=== Results ==="
echo "  Passed:  $PASSED"
echo "  Failed:  $FAILED"
echo "  Total:   $total"

if [[ ${#FAILURES[@]} -gt 0 ]]; then
  echo ""
  echo "Failed tests:"
  for f in "${FAILURES[@]}"; do
    echo "  - $f"
  done
  exit 1
fi

echo ""
echo "All tests passed."
