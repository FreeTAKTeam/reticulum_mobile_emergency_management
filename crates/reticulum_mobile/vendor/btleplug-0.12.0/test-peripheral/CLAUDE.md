# test-peripheral/ -- BLE Test Peripheral Implementations

Freshness: 2026-02-22

## Purpose

Two implementations of the same GATT profile used by the integration tests in `tests/`. One targets real hardware (Zephyr on nRF52840), the other is a virtual peripheral (Bumble on a USB BLE dongle).

## Structure

- `bumble/` -- Python virtual peripheral using the Bumble BLE stack
  - `test_peripheral.py` -- full GATT profile implementation
  - `run.sh` -- launcher script (usage: `./run.sh usb:0`)
  - `requirements.txt` -- Python dependencies (bumble)
- `zephyr/` -- C firmware for Zephyr RTOS (nRF52840 DK, ESP32-S3 DevKitC)
  - `src/gatt_profile.h` -- UUID definitions and GATT service declarations
  - `src/gatt_profile.c` -- read/write/notify handlers
  - `src/control_service.c` -- Control Point command dispatch
  - `src/test_handlers.c` -- test-specific behavior (counters, buffers)
  - `src/main.c` -- BLE init, advertising, connection management

## Contracts

- Both implementations MUST expose identical GATT services with identical UUIDs and behavior.
- The canonical UUID source of truth is `tests/common/gatt_uuids.rs`. Both peripherals mirror these.
- Four GATT services: Control (`0x0001`), Read/Write (`0x0002`), Notification (`0x0003`), Descriptor (`0x0004`).
- Control Point opcodes: `0x01` start notifications, `0x02` stop, `0x03` disconnect, `0x04` change adverts, `0x05` reset, `0x06` set notification payload.
- Peripheral advertises as `"btleplug-test"` with the Control Service UUID in the scan response.

## Invariants

- Adding a new GATT characteristic requires updating all three locations: `gatt_uuids.rs`, `gatt_profile.h`, and `test_peripheral.py`.
- The Bumble implementation must behave identically to the Zephyr firmware for all tests to pass on both.
