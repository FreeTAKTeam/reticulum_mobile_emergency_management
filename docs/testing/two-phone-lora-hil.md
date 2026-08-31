# Two-phone LoRa HIL acceptance

`tools/hil/run-two-phone-lora.sh` is the release gate for the physical RNode path. It must run
against committed, clean REM and sibling LXMF-rs worktrees and a signed Android build compiled
with `-PenableAdbTestControl=true`. The test-control receiver is disabled in normal builds.

Prerequisites:

- two distinct Android devices visible to `adb`, each paired with a distinct RNode;
- attached antennas and enough physical separation to avoid near-field receiver overload;
- the same LoRa profile, region, and frequency on both phones, with TCP clients disabled;
- `adb`, `jq`, `git`, and `base64` on the host;
- the app destination and canonical LXMF delivery destination shown by each phone's live status.

Run from any directory:

```sh
tools/hil/run-two-phone-lora.sh \
  --phone-a SERIAL_A --phone-b SERIAL_B \
  --app-destination-a APP_HEX_A --app-destination-b APP_HEX_B \
  --lxmf-destination-a LXMF_HEX_A --lxmf-destination-b LXMF_HEX_B
```

The run takes several minutes because SF11 radio operations are serialized. The script verifies
strict RNode readiness and ATT MTU, matching radio profiles, TCP-disabled state, peer announces,
successful connections, bidirectional small and Resource-sized direct LXMF messages, and
bidirectional event replication. It exits nonzero on the first failed boundary. A passing run
writes `result.txt`, source commit/tree identities, installed package versions, and filtered
privacy-safe logcat snapshots under ignored `target/lora-regression/<UTC timestamp>/`.
