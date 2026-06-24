#!/usr/bin/env bash
# Start the btleplug test peripheral using Bumble.
#
# Usage:
#   ./run.sh                    # Use default USB transport (usb:0)
#   ./run.sh usb:0              # Explicit USB transport
#   ./run.sh hci-socket:0       # Linux HCI socket transport
#
# Prerequisites:
#   pip install -r requirements.txt

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TRANSPORT="${1:-usb:0}"

echo "Starting btleplug test peripheral on transport: ${TRANSPORT}"
python3 "${SCRIPT_DIR}/test_peripheral.py" "${TRANSPORT}"
