# REM 1.5.5 telemetry final release gate

Status: **pending physical run**. The user deferred LoRa testing until all software
and signed-candidate preparation is complete. Do not publish, close milestone 13,
or claim field qualification from host/browser tests.

Record the candidate APK/AAB SHA-256, REM commit, LXMF-rs commit, RCH commit/version,
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
| RCH #258 chain | Record exact RCH build and independently verify upload → persisted record → map → collector, with correlated command/ACK and installed-client evidence. | Pending |

After all required cases pass, attach evidence to the release qualification record,
verify the final APK digest and source identity again, and publish from the approved
source. If code changes after the run, rebuild and repeat affected cases on the new
exact candidate.
