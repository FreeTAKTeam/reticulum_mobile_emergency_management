# Two-phone LoRa HIL acceptance

`tools/hil/run-two-phone-lora.sh` is the release gate for the physical RNode path. It must run
against committed, clean REM and sibling LXMF-rs worktrees and a signed Android build compiled
with `-PenableAdbTestControl=true`. The test-control receiver is disabled in normal builds.

Prerequisites:

- two distinct Android devices visible to `adb`, each paired with a distinct RNode;
- attached antennas and enough physical separation to avoid near-field receiver overload;
- both phones configured for the controlled RF profile: 914.625 MHz, 250 kHz bandwidth,
  SF11, coding rate 4/5, and 17 dBm, with TCP clients disabled;
- `adb`, `jq`, `git`, and `base64` on the host;
- the app destination and canonical LXMF delivery destination shown by each phone's live status.

Run from any directory:

```sh
tools/hil/run-two-phone-lora.sh \
  --phone-a SERIAL_A --phone-b SERIAL_B \
  --app-destination-a APP_HEX_A --app-destination-b APP_HEX_B \
  --lxmf-destination-a LXMF_HEX_A --lxmf-destination-b LXMF_HEX_B
```

The run takes several minutes because SF11 radio operations are serialized. After an initial
readiness check, the script sends the standard RNode hard-reset command to both radios and waits
for them to reconnect. This clears firmware transmit queues left by an interrupted earlier run;
it does not change pairing or the persisted radio profile. The script then verifies strict RNode
readiness and a negotiated ATT connection, the exact controlled radio profile, TCP-disabled state, peer announces,
successful connections, bidirectional small and Resource-sized direct LXMF messages, and
bidirectional event replication. It exits nonzero on the first failed boundary. A passing run
writes `result.txt`, source commit/tree identities, installed package versions, and filtered
privacy-safe logcat snapshots under ignored `target/lora-regression/<UTC timestamp>/`.
Failure and final snapshots include the RNode firmware `stat_rx` and `stat_tx` counters so a
host-to-radio enqueue can be distinguished from an over-the-air transmit or receive.
