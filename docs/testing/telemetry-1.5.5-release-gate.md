# REM 1.5.5 field-validation record

Status: **publication approved with partial physical evidence**. On 23 October
2026 the user approved release: “OK i think we are good for release”. The
unperformed physical scenarios below are deferred to [#311](https://github.com/FreeTAKTeam/reticulum_mobile_emergency_management/issues/311) and
are not reported as passed. This release decision supersedes earlier candidate
publication holds; full field qualification is not asserted.

Build source: `807041681430404f06dbac9b257fa5a192bed7e9`. The signed APK/AAB bytes
are unchanged from the verified candidate. See the accompanying manifest for
checksums, signer continuity, native libraries and pinned upstream revisions.

## Confirmed evidence

- All applicable build-source CI checks passed: 503 Rust, 56 Android JVM,
  171 app, 40 node-client and 128 browser tests, plus formatting, clippy,
  source-size, builds and dependency audits.
- Prior exact `3d443b4` candidate testing demonstrated LoRa Resource telemetry
  delivery and correlated application acceptance in both directions. Xiaomi
  used a real Android fix; Pixel used a labelled mock fix because indoor GNSS
  was unavailable. Pixel receiver persistence survived restart. Reverse delivery
  required a controlled retry after an earlier Resource timeout.
- Final exact normal `8070416` Pixel APK: five battery state/resume cases passed
  with a connected RNode. Original battery reporting and permission flags were
  restored. A same-source diagnostic build separately verified native permission
  revoke/regrant and unavailable-provider handling; these were limited API checks.
- The user confirmed two-way chat while Poco was off USB. On the final normal
  Pixel APK, Poco telemetry arrived at 14:54:32 local (267-byte Resource, command
  `t:169ee60ea47631b8`); Pixel sent its application result at 14:54:43.
- Marker creation arrived at 14:57:15 (909-byte Resource, command
  `marker-6d4aa7c5773335802709930b858c26ac`). Pixel displayed Drone 1 and sent
  its application result at 14:58:16. Rename command
  `marker-b74031ce99d9354eb1f7d77f3c7c29a4` arrived at 15:05:07 and the public
  map displayed Drone test. The user confirmed both names refer to the same marker.
- These later arrivals recovered naturally: no added test sends, app restarts
  or queue clearing occurred during recovery. One earlier incoming transfer
  targeted an invalidated Pixel session after an APK swap. That observation
  was not associated with every Poco queue entry.

## Deferred physical scenarios

| Case | Current scope |
| --- | --- |
| Both-direction cold LoRa with documented empty firmware queues and no warm-up | Prior bidirectional telemetry evidence is partial; remaining strict matrix deferred to #311. |
| Enable before readiness; multiple offline fixes; independent fan-out; saved-only restart on both phones | Host regressions pass. Remaining physical observations deferred to #311. |
| Trust/membership changes and connected-hub filtering on physical devices | Host coverage passes. Remaining physical observations deferred to #311. |
| Deliberately malformed two-phone telemetry | Missing-latitude Resource obtained transport receipt but no correlated application rejection before timeout; cause unconfirmed. Deferred to #311. |
| Background acquisition/delivery, permission-to-fix workflow, saver cadence, disabled/expired sharing | Battery/resume and limited native APIs verified as above. Remaining observations deferred to #311. |
| Queued marker moves and deletions | Creation and rename confirmed; remaining backlog operations unconfirmed and tracked in #311. |

Poco stayed off USB during the later observations, so its final installed APK
and processing of the latest application results were not independently inspected
again. The screenshot's newer 14:54:46 fix was separately queued for several
recipients; queue admission does not assert delivery. Preserve queued user changes
before any firmware reset or sender restart.

Record exact APK/source/upstream identities, phone/OS, RNode firmware and RF
profile for future runs. Keep command/message IDs and reason codes; omit exact
positions from normal logs and exported summaries.

RCH #258's installed Columba/Sideband acceptance remains a separate RCH gate.
