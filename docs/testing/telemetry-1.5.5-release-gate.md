# REM 1.5.5 telemetry final release gate

Status: **partial physical evidence; release held**. The signed `3d443b4`
candidate demonstrated bidirectional LoRa Resource delivery and correlated
application acceptance on 23 October 2026. Xiaomi used a real Android location;
Pixel used a labelled Android mock location because indoor GNSS was unavailable.
Pixel receiver map persistence survived restart. Reverse delivery required a
controlled retry after an earlier Resource transfer timed out. Do not describe
that as first-attempt acceptance.

The Poco has been returned to the user and is no longer attached to USB. Leave
it untouched. Android battery checks exposed a synchronous community fan-out
ANR; the corrected candidate must repeat those affected checks. Firmware queue
clearing, pre-readiness recovery, malformed two-phone telemetry and remaining
Android cases still need explicit evidence. Do not publish, close milestone 13,
or claim full field qualification from software tests or partial radio results.

Record the candidate APK/AAB SHA-256, REM commit, LXMF-rs commit, RCH commit/version when participating,
phone model/OS, RNode identity/firmware, RF band/frequency/bandwidth/spreading factor/
coding rate/transmit power, and the selected Android transport on both phones.
Keep identity/command/message IDs and reason codes in the evidence; redact exact
positions from normal logs and exported summaries.

| Case | Required observation | Result |
| --- | --- | --- |
| A → B, cold LoRa only | No TCP, chat/SOS/manual connect/Resource warm-up. First telemetry selects the saved team peer, creates its route/link, reaches receiver persistence/map, and returns correlated application acceptance. Record command/message ID, selected Packet/Resource representation and RNode interface. | Pending |
| B → A, cold LoRa only | Repeat from clean process startup, with the opposite sender and the same evidence. | Pending |
| Enable before radio ready | Local fix is visible as local/pending; the newest fix flushes when radio/path becomes ready, before the full periodic interval. | Pending |
| Multiple offline fixes | One latest pending fix per destination/callsign; older fixes do not replay or regress the receiver map. | Pending |
| Saved-only/restart | Restart both apps with existing contacts/team membership. Delivery resumes without re-saving or warm-up; accepted fixes do not replay. Record uncertain in-flight recovery separately. | Pending |
| Trust and fan-out | Removed/out-of-team/non-capable peers receive no exact position. One failed target does not prevent another recipient's acceptance. Connected hub filtering is unchanged. | Pending |
| No route / timeout / rejection | Waiting/queued/transmitted/transport receipt/app ACK remain distinct. Invalid telemetry yields a correlated rejection and a typed position-free diagnostic. | Pending |
| Background / permission / battery | Background and restart behavior; revoked/re-granted location; GPS unavailable; saver cadence; disabled/expired sharing. Never infer delivery from the GPS timer. | Pending |
| RCH #258 coordination | Record the participating RCH build and coordinate upload → persistence → map → collector evidence/status with RCH #258. Its Columba/Sideband installed-client acceptance remains a separate RCH gate. | Tracked separately |

After all required cases pass, attach evidence to the release qualification record,
verify the final APK digest and source identity again, and publish from the approved
source. If code changes after the run, rebuild and repeat affected cases on the new
exact candidate.

RCH #258 is coordination work, not an additional requirement to close that issue
or complete its separate Columba/Sideband matrix before REM publication. This does
not waive the REM LoRa and Android cases above.
