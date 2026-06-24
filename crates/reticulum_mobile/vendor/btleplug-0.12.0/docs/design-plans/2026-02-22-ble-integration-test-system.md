# BLE Integration Test System Design

## Summary

Design for an end-to-end integration test system for btleplug. A dedicated nRF52840 DK running Zephyr acts as the primary test peripheral, exposing a custom GATT profile that exercises every btleplug API method. A Python Bumble-based virtual peripheral provides the same profile for quick developer feedback without hardware. Rust integration tests in `tests/` discover the peripheral by name, connect, and validate all operations.

## Definition of Done

**Primary deliverables:**
1. A **Zephyr-based GATT server firmware** for nRF52840 DK that exposes test services covering btleplug's full API (read, write with/without response, notify, indicate, advertisements with service UUIDs/manufacturer data/device name, MTU exchange, connection parameters, RSSI)
2. A **Rust integration test suite** in btleplug that connects to this peripheral and exercises all operations
3. A **virtual/emulated peripheral** (Python Bumble) implementing the same test service for quick developer feedback without hardware
4. **Documentation** for setting up the test environment (flashing the board, running tests)

**Success criteria:**
- A developer can flash the nRF52 DK, run `cargo test`, and get pass/fail results for real BLE operations
- A developer without hardware can run a subset of tests against the virtual peripheral
- Tests are structured so CI automation can be added later (self-hosted runner with hardware)

**Out of scope:**
- CI automation setup (future work)
- Testing btleplug on all host platforms simultaneously (tests run on whatever OS the developer is using)
- Performance/throughput benchmarking

## Glossary

- **GATT** — Generic Attribute Profile. The protocol BLE devices use to expose structured data as services and characteristics.
- **Central** — The BLE client role (btleplug). Scans for, connects to, and communicates with peripherals.
- **Peripheral** — The BLE server role (the test device). Advertises, accepts connections, and hosts GATT services.
- **Characteristic** — A GATT data endpoint within a service. Has a UUID, properties (read/write/notify/indicate), and a value.
- **Descriptor** — Metadata attached to a characteristic (e.g., CCCD for enabling notifications).
- **Notification/Indication** — Server-initiated value updates. Notifications are unacknowledged; indications require acknowledgement.
- **MTU** — Maximum Transmission Unit. The negotiated maximum payload size for a single ATT packet.
- **RSSI** — Received Signal Strength Indicator. Signal power in dBm.
- **Zephyr** — Open-source RTOS used on the nRF52840 DK for the test peripheral firmware.
- **Bumble** — Google's pure-Python BLE stack, used here as the virtual test peripheral.
- **nRF52840 DK** — Nordic Semiconductor development kit (~$45) with BLE 5.0 support.

## Architecture

### Overview

The test system has three components:

```
┌─────────────────────────┐     BLE Radio     ┌─────────────────────────┐
│  Rust Integration Tests │ ◄──────────────► │  nRF52840 DK (Zephyr)   │
│  (tests/*.rs)           │                   │  GATT Test Server       │
│  using btleplug API     │                   │  (test-peripheral/      │
│                         │                   │   zephyr/)              │
└─────────────────────────┘                   └─────────────────────────┘

         OR (no hardware)

┌─────────────────────────┐   Host BLE Stack  ┌─────────────────────────┐
│  Rust Integration Tests │ ◄──────────────► │  Python Bumble           │
│  (same tests)           │                   │  GATT Test Server       │
│                         │                   │  (test-peripheral/      │
│                         │                   │   bumble/)              │
└─────────────────────────┘                   └─────────────────────────┘
```

The Rust tests are identical regardless of which peripheral backend is running. They discover the test peripheral by its advertised device name `"btleplug-test"`.

### Test GATT Profile

Both the Zephyr firmware and Python Bumble peripheral implement this identical GATT profile. UUIDs use a shared base: `XXXXXXXX-b5a3-f393-e0a9-e50e24dcca9e` (btleplug test namespace).

#### Control Service (UUID: `00000001-b5a3-f393-e0a9-e50e24dcca9e`)

| Characteristic | UUID suffix | Properties | Purpose |
|---|---|---|---|
| Control Point | `...0101...` | Write | Accept commands to change peripheral behavior |
| Control Response | `...0102...` | Notify | Return command responses/acknowledgements |

**Control commands** (1-byte opcode written to Control Point):

| Opcode | Command | Behavior |
|---|---|---|
| `0x01` | Start Notifications | Begin sending periodic notifications (1 Hz) on Notification Test chars |
| `0x02` | Stop Notifications | Stop all periodic notifications |
| `0x03` | Trigger Disconnect | Peripheral disconnects after 500ms (tests reconnection) |
| `0x04` | Change Advertisements | Rotate to alternate advertisement data set |
| `0x05` | Reset State | Return to default state (stop notifications, restore default advertisements) |
| `0x06` | Set Notification Payload | Next N bytes become the notification payload |

#### Read/Write Test Service (UUID: `00000002-b5a3-f393-e0a9-e50e24dcca9e`)

| Characteristic | UUID suffix | Properties | Purpose |
|---|---|---|---|
| Static Read | `...0201...` | Read | Returns fixed known value `[0x01, 0x02, 0x03, 0x04]` |
| Counter Read | `...0202...` | Read | Returns incrementing 4-byte counter (changes each read) |
| Write With Response | `...0203...` | Write | Stores value; readable via Control Response notification |
| Write Without Response | `...0204...` | Write Without Response | Stores value; verifiable via Static Read update |
| Read/Write | `...0205...` | Read, Write | Bidirectional — write a value, read it back |
| Long Value | `...0206...` | Read, Write | 512-byte buffer for testing MTU boundary behavior |

#### Notification Test Service (UUID: `00000003-b5a3-f393-e0a9-e50e24dcca9e`)

| Characteristic | UUID suffix | Properties | Purpose |
|---|---|---|---|
| Notify Char | `...0301...` | Notify | Sends periodic notifications when subscribed (triggered by Control) |
| Indicate Char | `...0302...` | Indicate | Sends periodic indications when subscribed (triggered by Control) |
| Configurable Notify | `...0303...` | Notify | Content/rate controlled via Control Point opcode `0x06` |

#### Descriptor Test Service (UUID: `00000004-b5a3-f393-e0a9-e50e24dcca9e`)

| Characteristic | UUID suffix | Properties | Purpose |
|---|---|---|---|
| Descriptor Test Char | `...0401...` | Read | Has custom descriptors below |

Custom descriptors on Descriptor Test Char:

| Descriptor | UUID | Purpose |
|---|---|---|
| Read-Only Descriptor | `...04A1...` | Returns fixed value, tests `read_descriptor()` |
| Read/Write Descriptor | `...04A2...` | Accepts writes, tests `write_descriptor()` and `read_descriptor()` |

#### Advertisement Configuration

The test peripheral advertises with:
- **Device name**: `"btleplug-test"`
- **Service UUIDs**: All four test service UUIDs
- **Manufacturer data**: Company ID `0xFFFF` (reserved for testing) + `[0xBB, 0xTT, 0x01]` ("bt" + version)
- **Service data**: Control Service UUID + `[0x01]` (status byte)

This ensures btleplug's `properties()`, `CentralEvent::ManufacturerDataAdvertisement`, `CentralEvent::ServiceDataAdvertisement`, and `CentralEvent::ServicesAdvertisement` can all be tested.

### Rust Test Suite

#### Directory Structure

```
tests/
├── common/
│   ├── mod.rs                — Re-exports helpers
│   ├── peripheral_finder.rs  — Scan for "btleplug-test", connect, discover services
│   └── gatt_uuids.rs         — UUID constants matching the GATT profile above
├── test_discovery.rs          — Scanning, advertisement data, CentralEvents
├── test_connection.rs         — Connect, disconnect, reconnect, is_connected
├── test_read_write.rs         — read(), write() with both WriteTypes, long values
├── test_notifications.rs      — subscribe(), unsubscribe(), notifications stream
├── test_descriptors.rs        — read_descriptor(), write_descriptor()
└── test_device_info.rs        — properties(), mtu(), read_rssi(), connection_parameters()
```

#### Peripheral Discovery

The shared `peripheral_finder` module:
1. Creates a `Manager` and gets the first adapter
2. Starts a scan with `ScanFilter` for the test service UUID
3. Waits (with timeout) for a peripheral with name `"btleplug-test"`
4. Returns the discovered `Peripheral` for the test to use

The `BTLEPLUG_TEST_PERIPHERAL` environment variable can override the device name to allow multiple test boards.

#### Test Execution

```bash
# With hardware peripheral (nRF52 DK running, or Bumble started):
cargo test --test test_read_write

# All integration tests:
cargo test --test '*'

# Skip integration tests (unit tests only):
cargo test --lib
```

Integration tests are gated with `#[ignore]` by default (since they need an external peripheral running) and run with:
```bash
cargo test --test '*' -- --ignored
```

Alternatively, a `BTLEPLUG_TEST_ENABLED=1` environment variable can be checked at the top of each test to skip gracefully when no peripheral is available.

### Test Peripheral Implementations

#### Zephyr Firmware (`test-peripheral/zephyr/`)

```
test-peripheral/
├── zephyr/
│   ├── CMakeLists.txt        — Zephyr build config
│   ├── prj.conf              — Kconfig: BLE peripheral, GATT, logging
│   ├── boards/
│   │   └── nrf52840dk_nrf52840.conf  — Board-specific config
│   └── src/
│       ├── main.c            — Initialization, advertising start
│       ├── gatt_profile.c    — Service/characteristic definitions
│       ├── gatt_profile.h    — UUID definitions, shared constants
│       ├── control_service.c — Control Point command handler
│       └── test_handlers.c   — Read/write/notify callback implementations
```

**Build and flash:**
```bash
cd test-peripheral/zephyr
west build -b nrf52840dk/nrf52840
west flash
```

The firmware:
- Boots and immediately starts advertising as `"btleplug-test"`
- Accepts connections (one at a time)
- Handles all GATT operations defined in the profile
- Processes Control Point commands to trigger dynamic behavior
- Resumes advertising after disconnection

#### Python Bumble (`test-peripheral/bumble/`)

```
test-peripheral/
├── bumble/
│   ├── requirements.txt      — bumble dependency
│   ├── test_peripheral.py    — GATT server implementation
│   └── run.sh                — Convenience script to start the peripheral
```

**Run:**
```bash
cd test-peripheral/bumble
pip install -r requirements.txt
python test_peripheral.py
```

The Bumble peripheral:
- Implements the same GATT profile with identical UUIDs and behavior
- Uses Bumble's `Device` and `Server` APIs to register services
- Runs on the host OS's BLE stack (requires a BLE adapter, but not dedicated hardware)
- Supports the same Control Point commands

**Limitations vs. hardware:** Bumble runs through the host OS BLE stack, so it may not perfectly replicate timing, MTU negotiation behavior, or RSSI values. Tests that are sensitive to these should be marked as hardware-only.

### Shared GATT Profile Definition

To keep UUIDs and behavior in sync between Zephyr, Bumble, and Rust tests, the canonical source of truth is:
- **UUID constants**: Defined in `tests/common/gatt_uuids.rs` (Rust) and mirrored in `test-peripheral/zephyr/src/gatt_profile.h` (C) and `test_peripheral.py` (Python)
- **Behavior specification**: This design document serves as the specification

A future improvement could generate the profile definitions from a single source (e.g., a TOML file), but manual synchronization is acceptable for the initial implementation.

## Existing Patterns

- **btleplug's existing unit tests** use inline `#[cfg(test)]` modules. Integration tests go in `tests/` following standard Rust convention.
- **btleplug's examples** (`examples/`) demonstrate the scanning/connecting/subscribing flow and serve as reference for how the test helper should be structured.
- **Async runtime**: btleplug uses Tokio. Tests should use `#[tokio::test]`.
- **Error handling**: btleplug uses `thiserror` with a custom `Error` enum. Tests should assert specific error variants where appropriate.

## Implementation Phases

### Phase 1: GATT Profile Definition and Rust Test Scaffolding

Define the UUID constants and shared test helpers. Create the `tests/` directory structure with placeholder tests that compile but are `#[ignore]`d. This establishes the contract that both peripheral implementations must fulfill.

**Files:** `tests/common/mod.rs`, `tests/common/gatt_uuids.rs`, `tests/common/peripheral_finder.rs`, all `tests/test_*.rs` files with placeholder tests.

### Phase 2: Zephyr Test Peripheral Firmware

Implement the GATT server firmware for nRF52840 DK using Zephyr. Start with the Read/Write Test Service and Control Service, then add Notification and Descriptor services.

**Files:** Everything under `test-peripheral/zephyr/`.

### Phase 3: Core Integration Tests (Read/Write/Discovery)

Implement the Rust integration tests for discovery (`test_discovery.rs`), connection lifecycle (`test_connection.rs`), and read/write operations (`test_read_write.rs`). These are the most fundamental tests and validate the overall system works end-to-end.

**Files:** `tests/test_discovery.rs`, `tests/test_connection.rs`, `tests/test_read_write.rs`.

### Phase 4: Notification and Descriptor Tests

Implement notification/indication tests (`test_notifications.rs`) and descriptor tests (`test_descriptors.rs`). These require the Control Service to trigger notifications on the peripheral side.

**Files:** `tests/test_notifications.rs`, `tests/test_descriptors.rs`.

### Phase 5: Device Info Tests (MTU, RSSI, Connection Parameters)

Implement tests for `mtu()`, `read_rssi()`, `connection_parameters()`, and `request_connection_parameters()`. These test platform-specific behavior and may need per-platform expected-value adjustments.

**Files:** `tests/test_device_info.rs`.

### Phase 6: Python Bumble Virtual Peripheral

Implement the Bumble-based virtual peripheral matching the same GATT profile. Mark hardware-sensitive tests (RSSI, precise MTU values, connection parameters) as hardware-only.

**Files:** Everything under `test-peripheral/bumble/`.

### Phase 7: Documentation and Developer Setup Guide

Write setup documentation: how to install Zephyr/west, flash the nRF52 DK, run tests with hardware, run tests with Bumble, and troubleshoot common issues.

**Files:** `test-peripheral/README.md`, updates to project `README.md`.

## Additional Considerations

### Platform-Specific Test Behavior

Some btleplug operations have documented platform-specific behavior (e.g., `read_rssi()` on Windows returns cached advertisement values). Integration tests should:
- Test the common behavior across all platforms
- Use `#[cfg(target_os = "...")]` for platform-specific assertions where needed
- Document which tests may behave differently per platform

### Test Isolation and Ordering

BLE operations are inherently stateful (connection state, notification subscriptions). Tests should:
- Reset peripheral state via the Control Point `0x05` command at the start of each test
- Not depend on execution order
- Use reasonable timeouts for all BLE operations (scanning, connecting, notifications)

### Future CI Integration

The test design supports future CI automation by:
- Using environment variables for peripheral selection (`BTLEPLUG_TEST_PERIPHERAL`, `BTLEPLUG_TEST_ENABLED`)
- Keeping tests runnable as standard `cargo test` invocations
- Separating hardware-required tests from virtual-capable tests
- Board reset via `west flash` or DTR line toggle can be scripted

### Hardware Requirements

- **nRF52840 DK** (~$45 USD) — one per developer or test station
- **USB cable** (included with DK)
- **Host BLE adapter** — built-in on most laptops; USB dongle for desktops
- **Zephyr SDK** and **nRF Command Line Tools** for building/flashing firmware
