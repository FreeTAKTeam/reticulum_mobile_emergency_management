# REM 1.2 Manual Release Gate

REM 1.2.0 stays a prerelease until this matrix passes on connected phones.

This runbook is for manual validation only. Do not clear app data, uninstall the app, delete Bluetooth pairings, or replace saved RNode configuration while executing it.

## Required Sequence

Run every workflow in this order:

1. Announce from the source phone.
2. Confirm the target phone sees the source peer.
3. Connect to the peer and verify the peer is `Connected`, not only `Reachable`.
4. Send or update the workflow payload.
5. Verify delivery on the target phone UI and, when possible, in logs.

Do not count opportunistic delivery as a pass for chat or direct workflow checks. A peer connection must be established before sending.

## Interface Modes

Validate the full workflow set in each mode:

| Mode | Source phone | Target phone | Pass condition |
| --- | --- | --- | --- |
| LoRa-only | RNode LoRa enabled, TCP disabled | RNode LoRa enabled, TCP disabled | Validate RNode traffic and the `REM not ready: RNode LoRa is the only active receiving interface.` degraded state. |
| TCP-only | TCP enabled, RNode LoRa disabled | TCP enabled, RNode LoRa disabled | Announce, connect, and workflow delivery all use TCP-capable paths. |
| Mixed TCP+LoRa | TCP and RNode LoRa enabled on the source | One target TCP-only and one target LoRa-only | The same workflow update from the mixed source reaches both targets. |

After changing TCP or LoRa settings, save settings and restart REM before validating traffic. Restart-free interface reconfiguration is not required for 1.2 final.

RNode-over-BLE is the phone-to-RNode hardware link only. It is not phone-to-phone Bluetooth mesh. Treat RNode firmware 1.86 as the current hardware validation target, but do not fail the gate solely because firmware version metadata is absent or different.

## Diagnostic ADB Control

Diagnostic builds can expose a small adb-only control receiver for repeatable announce and peer-link checks. The receiver is disabled in normal release builds and is enabled only when the Android build is compiled with `-PenableAdbTestControl=true`.

Build example:

```powershell
cmd /c "gradlew.bat :app:assembleRelease -PappVersionName=1.2.0-rc.2 -PappVersionCode=261772128 -PenableAdbTestControl=true -PapkClassifierSuffix=adb-test"
```

Useful commands:

```powershell
adb -s <serial> shell am broadcast -n network.reticulum.emergency/.AdbTestControlReceiver -a network.reticulum.emergency.action.ADB_STATUS
adb -s <serial> shell am broadcast -n network.reticulum.emergency/.AdbTestControlReceiver -a network.reticulum.emergency.action.ADB_APP_SETTINGS
adb -s <serial> shell am broadcast -n network.reticulum.emergency/.AdbTestControlReceiver -a network.reticulum.emergency.action.ADB_ANNOUNCE
adb -s <serial> shell am broadcast -n network.reticulum.emergency/.AdbTestControlReceiver -a network.reticulum.emergency.action.ADB_CONNECT_PEER --es destinationHex <peer-lxmf-destination>
adb -s <serial> shell am broadcast -n network.reticulum.emergency/.AdbTestControlReceiver -a network.reticulum.emergency.action.ADB_DISCONNECT_PEER --es destinationHex <peer-lxmf-destination>
```

The receiver only triggers status, read-only app settings, announce, connect, and disconnect calls. It does not change TCP or LoRa configuration, does not delete saved peers, and does not replace UI verification for chat, events, EAM, or checklist rows.

## Workflow Matrix

Each row requires a unique marker string recorded in issue #149.

| Workflow | LoRa-only | TCP-only | Mixed TCP+LoRa |
| --- | --- | --- | --- |
| Announce peer visibility | Pending | Pending | Passed |
| Peer connection after announce | Pending | Pending | Passed |
| Chat delivery | Pending | Pending | Passed |
| Event replication | Pending | Pending | Passed |
| EAM/preparedness update | Pending | Pending | Passed |
| Checklist update | Pending | Pending | Passed |

## Mixed Mode Routing Contract

REM must not force TCP-first or LoRa-first behavior. In mixed mode, Reticulum routing/interface resolution chooses the outbound interface.

Acceptable evidence:

- Runtime logs show both interfaces active on the mixed source.
- Outbound workflow sends are targeted to the peer destination, not to a REM-selected interface preference.
- The TCP-only target and LoRa-only target both receive the same workflow update.

## Duplicate TCP+LoRa Delivery

Duplicate packet delivery across simultaneous TCP+LoRa must be handled by Reticulum transport packet-cache behavior before REM workflow handlers receive payloads.

Do not satisfy this gate with REM-side message-id filters, TCP-first/LoRa-first routing policy, or workflow/UI cleanup.

Acceptable evidence:

- Reticulum transport duplicate cache test passes: `cargo test -p reticulum-rs-transport drop_duplicates`.
- A connected-phone mixed scenario is observed where simultaneous TCP+LoRa delivery does not create duplicate chat, event, EAM, or checklist rows.
- Logs or UI evidence identify the marker, source, targets, and absence of duplicate rows.

## Evidence Format

For each completed row, update issue #149 with:

- Date and local time.
- Phone serials and roles.
- Interface mode under test.
- Marker string or event/checklist/EAM identifier.
- UI result on each target.
- Relevant log snippets or exact log markers when available.

Leave a row unchecked when delivery is inferred only from passive traffic, when the peer was not connected before sending, or when only one target in a mixed test receives the update.
