# Connect an RNode to REM on Android

This guide explains how to pair, configure, connect, and verify an RNode with
REM on Android.

> **Build requirement:** use REM `1.3.0` or a later build. The public `1.2.9`
> tag predates the Android BLE and Classic RNode transports introduced by
> **Add Android BLE and Classic RNode transports (#253)** (`e1656ee`).

## Supported Connection Paths

| Path | What REM supports |
| --- | --- |
| Bluetooth Low Energy (BLE) | Live RNode connection over Nordic UART Service (NUS). This is the default. |
| Bluetooth Classic (SPP) | Live RNode connection over the standard Serial Port Profile. |
| USB-assisted Bluetooth pairing | REM can use a USB data connection to put a compatible RNode into Bluetooth pairing mode. The live connection is still Bluetooth. |
| USB serial | Not yet available as a live REM RNode bearer. |
| TCP | Configure normal Reticulum TCP endpoints under **TCP Interfaces**. It is not an RNode Bluetooth selection. |

Phone-to-phone Bluetooth mesh is also separate from the RNode connection. An
RNode Bluetooth link connects one Android phone to one radio.

## Before You Start

1. Install a REM build that includes PR #253 or a later Android transport
   update.
2. Update the RNode to current official firmware. For modern Android BLE
   pairing, update firmware older than `1.83` before troubleshooting the app.
3. Power the RNode and keep it close to the phone.
4. Turn on Bluetooth on both the phone and RNode. On current official firmware
   for display-equipped RNodes, a single user-button press enables Bluetooth
   when it is off.
5. Put the RNode into pairing mode. On those RNodes, hold the user button for
   more than five seconds and release it. The Bluetooth symbol changes and a
   pairing PIN appears on the display. Follow the hardware-specific firmware
   instructions when the RNode has no display or user button.
6. Stop Sideband, `rnsd`, `rnodeconf`, or another program that may already be
   using the RNode. RNode firmware permits only one active host connection.
7. Know the legal region and frequency for the deployment. Every RNode that
   must communicate needs matching frequency, bandwidth, spreading factor,
   and coding rate settings.

Android must grant REM **Nearby devices** or Bluetooth permission. Some older
Android versions also require Location permission for Bluetooth scanning. REM
requests the applicable Android permissions when a scan or paired-device list
is opened.

## Connect an Already Paired RNode

1. Open REM.
2. Open **More > Settings**.
3. Expand **Node Config** and find **LoRa / RNode**.
4. Select the correct **Bluetooth bearer**:
   - **Bluetooth Low Energy (BLE)** for an RNode that advertises NUS.
   - **Bluetooth Classic (SPP)** for a Classic serial RNode.
5. Tap **Show paired Bluetooth**.
6. Tap the correct RNode. Confirm that **RNode device id** is populated and the
   displayed bearer matches the radio.
7. Turn on **Enable RNode Bluetooth LoRa**.
8. Select the correct **Region**, **Frequency (Hz)**, and **REM LoRa profile**.
9. Tap **Save** at the bottom of Settings. REM automatically restarts a running
   node when its RNode settings change. If the app reports that restart failed,
   expand **Node Control** and tap **Restart**.
10. Continue with [Verify the connection](#verify-the-connection).

## Scan and Pair in REM

Use REM's in-app scan for BLE on Android 15 and newer. Those Android versions
can hide BLE peripherals from the system Bluetooth pairing screen.

1. Put the RNode into pairing mode and keep its PIN visible.
2. Open **More > Settings > Node Config > LoRa / RNode**.
3. Choose **Bluetooth Low Energy (BLE)** or **Bluetooth Classic (SPP)** before
   scanning.
4. Tap **Scan RNode BLE** or **Scan RNode Classic**.
5. Grant REM the requested Bluetooth or Nearby devices permission. If the scan
   still cannot start on an older Android version, also grant Location
   permission in Android app settings.
6. Wait for the eight-second scan to finish, then tap the RNode in the result
   list.
7. Accept the Android pairing prompt and confirm or enter the PIN displayed by
   the RNode.
8. If REM says pairing started but does not select the radio automatically,
   finish the Android prompt, tap **Show paired Bluetooth**, and select the
   RNode from that list.
9. Turn on **Enable RNode Bluetooth LoRa**, select the correct region,
   frequency, and profile, then tap **Save**.
10. Continue with [Verify the connection](#verify-the-connection).

For Classic/SPP, the RNode must be discoverable and provide the standard SPP
service. If in-app Classic bonding does not complete, pair the RNode once in
Android Bluetooth settings, return to REM, and use **Show paired Bluetooth**.

## Pair Bluetooth Through USB

USB-assisted pairing is useful when the RNode is not yet Bluetooth-paired or
when Android cannot discover it reliably. It provisions the Bluetooth bond;
it does not turn USB into the live REM transport.

1. Connect the powered RNode directly to the Android phone with a USB data
   cable and any required OTG adapter.
2. Open **More > Settings > Node Config > LoRa / RNode**.
3. Select **Bluetooth Low Energy (BLE)**.
4. Tap **Pair via USB** and grant Android USB access to REM.
5. If more than one USB device is listed, tap the intended RNode and then tap
   **Pair via USB** again.
6. REM asks the RNode to enter Bluetooth pairing mode. Accept the Android bond
   prompt and enter the PIN shown by REM or on the RNode when requested.
7. Wait while REM checks the paired-device list. If it does not select the
   device automatically, tap **Show paired Bluetooth** and select it.
8. Turn on **Enable RNode Bluetooth LoRa**, confirm the region, frequency, and
   profile, then tap **Save**.
9. After pairing, do not leave a desktop serial program connected to the
   RNode. The USB cable may provide power, but another open serial host can
   prevent the Bluetooth session.

## Verify the Connection

1. Open **More > Settings > Node Control**.
2. Confirm the local node says **Node is running**. This only proves the local
   REM runtime started; it does not by itself prove the radio is connected.
3. Find the RNode interface. Its label begins with `rnode-ble:` or
   `rnode-bluetooth-classic:`.
4. Wait up to 30 seconds for the interface to move from `connecting` to
   `connected`. REM reports the LoRa readiness row as **Ready** only after the
   RNode probe detects an online radio.
5. Check that no last-error text appears below the interface.
6. With another correctly configured RNode in range, tap **Announce** on both
   phones and confirm receive packet counters increase. For a complete test,
   save and connect the peer, send a chat message, and confirm delivery on the
   other phone.

An app-level **Ready** badge with an RNode interface still marked `connecting`
or `failed` is degraded operation, not proof of LoRa connectivity. TCP can keep
the local app usable while the RNode retries independently.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| **No RNode BLE devices found** | Confirm the RNode is in pairing mode, Bluetooth is enabled, REM has Nearby devices permission, the radio is close, and the firmware advertises Nordic UART Service. On Android 15/16, use REM's in-app scan instead of relying on Android settings. |
| **No RNode Classic devices found** | Make the radio discoverable, choose Classic before scanning, or pair it once in Android settings and use **Show paired Bluetooth**. |
| **Bluetooth permission denied** | Open Android **Settings > Apps > REM > Permissions** and allow **Nearby devices**. On Android versions that require it for scans, also allow Location. |
| **RNode Nordic UART service is missing** | The selected device is not a BLE NUS RNode, its firmware is too old, or the wrong bearer was selected. Update the firmware and confirm BLE mode. |
| **BLE notification subscription failed** | Power-cycle the RNode, toggle phone Bluetooth, stop other RNode clients, and retry. If it persists, remove the Android bond and pair again. Removing the bond does not delete REM data, but the RNode must be selected again. |
| **RNode BLE is not paired** | Hold the RNode user button for more than five seconds, release it when the Bluetooth icon changes, and note the PIN on its display. In REM, scan for the RNode, select it, enter or confirm that PIN in Android's pairing prompt, then select the newly paired device and save. A power cycle alone does not enter secure pairing mode. |
| **RNode BLE write timed out during startup** | First confirm the radio is bonded to this phone. RNode's BLE UART requires an encrypted paired connection, so an unpaired radio can appear in a scan but reject startup writes. Put it in pairing mode, pair again using the displayed PIN, and retry. |
| **Bluetooth Classic connection timed out** | Confirm the radio is bonded, supports SPP, is powered and in range, and is not connected to another host. |
| **Interface stays connecting, then fails after 30 seconds** | Check the selected device ID and bearer, power, pairing state, and whether another program owns the RNode. The error under **Node Control** is the authoritative failure detail. |
| **Older RNode omits its startup radio-state response** | REM 1.3.0 accepts this one missing echo only after the probe and every reported radio parameter validate. The interface then connects with a compatibility warning. A real mismatch or hardware error still fails startup. Update the RNode firmware when practical. |
| **BLE disconnects after sending an announce or large packet** | Confirm the app is REM 1.3.0 or later. It negotiates a large inbound MTU but preserves firmware-safe 20-byte outbound GATT chunks. Power-cycle a radio left unresponsive by an older build, toggle phone Bluetooth, and reconnect. |
| **Short packets arrive, but announces, proofs, or messages time out** | Move the two RNodes several metres apart and keep both antennas attached. At REM's default 17 dBm transmit power, radios placed very close together can overload each other's receivers. Also confirm both phones run REM 1.3.0 or later; RNode-only mode avoids background saved-peer link warming and automatic propagation-relay polling so retries do not delay a proof past its activation deadline. |
| **Connected, but no packets move** | Confirm both radios use the same frequency and REM profile, the frequency is legal for the region, antennas are attached, and the peer test is not being routed only through TCP. |
| **Settings changed but the old connection remains** | Tap **Save** and wait for the automatic node restart. If REM reports a restart error, use **Node Control > Restart** or fully restart the app. |

## REM Radio Defaults

REM defaults to `US915`, `915000000` Hz, and `REM-LF-RURAL-v1`. Do not use
that frequency outside a region where it is permitted.

| Profile | Bandwidth | Spreading factor | Coding rate |
| --- | ---: | ---: | ---: |
| `REM-LF-RURAL-v1` | 250 kHz | 11 | 4/5 |
| `REM-MF-URBAN-v1` | 250 kHz | 9 | 4/5 |
| `REM-LM-EXTREME-v1` | 125 kHz | 11 | 4/8 |

For implementation and readiness details, see the
[RNode Bluetooth LoRa architecture](architecture.md#rnode-bluetooth-lora-interface).
Official radio-side references:

- [RNode Bluetooth setup on Android](https://unsigned.io/guides/2023_01_19_RNode_Bluetooth_Setup_In_Sideband_On_Android.html)
- [Official RNode firmware releases](https://github.com/markqvist/RNode_Firmware/releases)
- [Android 15/16 BLE pairing guidance](https://github.com/markqvist/Reticulum/discussions/980)
