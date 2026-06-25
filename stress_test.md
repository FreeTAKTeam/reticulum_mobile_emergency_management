# REM Live Multi-Phone Stress Test Goal Prompt

Use this file as the goal prompt for a live release-readiness stress test of
Reticulum Emergency Management (REM). The test is not complete until REM has
been exercised across multiple real phones, every user-facing function has been
executed, cross-device results have been verified, and all release-blocking
defects found during the run have been fixed or explicitly documented as
non-blocking.

## Goal

Prove that the current REM build is release-ready and rock solid under live
multi-phone use.

Run a live integration stress test with at least three Android phones connected
over USB, plus any additional REM-capable phones available over the same
Reticulum mesh. Exercise every REM function from one phone and verify the result
on the other phones. Rotate the source phone so each USB-connected phone acts as
sender, receiver, and recovery target. Do not claim release readiness until the
full matrix passes with evidence.

## Operating Rules

- Work only in this repository:
  `C:\Users\broth\Documents\work\ATAK\src\reticulum_mobile_emergency_management`.
- Start by running `git status --short` and record the branch, commit, app
  version, and whether the worktree is clean.
- Do not scan or operate on sibling repositories unless a project dependency
  explicitly requires it.
- Treat generated Android/Rust artifacts as volatile. Do not hand-edit generated
  files.
- Use the compiled Rust/LXMF bridge already present in REM. Do not recreate
  protocol behavior in TypeScript or Vue to bypass the runtime.
- If a failure is found, first reproduce it with the smallest possible live
  scenario, capture evidence, identify the owning layer, apply the smallest
  defensible fix, and rerun the failed scenario plus the nearest broader smoke
  test.
- If hardware, permissions, or network conditions prevent a test, mark the row
  `BLOCKED`, record the exact blocker, and do not count that row as passed.
- Do not call the release rock solid unless there are zero critical or high
  severity blockers and every required live matrix row is `PASS`.

## Required Hardware Topology

Use at minimum:

- `Phone A`: USB-connected Android phone, primary sender for round 1.
- `Phone B`: USB-connected Android phone, primary sender for round 2.
- `Phone C`: USB-connected Android phone, primary sender for round 3.
- `Phone D+`: optional non-USB phones on the same Reticulum mesh for extra
  receiver/load coverage.

For each phone record:

- ADB serial from `adb devices -l`.
- Android version and device model.
- REM app version and build commit.
- Callsign/display name used during test.
- Reticulum destination hash shown in REM.
- Whether the phone is USB controlled, manual observer only, or both.
- Whether location permission, notification permission, and background behavior
  are enabled.

## Required Evidence

Create an execution log in this file or in a clearly linked artifact. For every
test row, capture:

- Source phone and receiving phones.
- Exact action performed.
- Start timestamp and first-visible timestamp on each receiver.
- Message/checklist/event/telemetry/SOS identifier when available.
- Screenshots or screen recordings from source and at least one receiver.
- Relevant `adb logcat` excerpts for failures.
- Pass/fail result and severity.
- Fix commit or issue reference for any defect found.

Suggested live evidence commands:

```powershell
adb devices -l
adb -s <serial> shell getprop ro.product.model
adb -s <serial> shell getprop ro.build.version.release
adb -s <serial> logcat -c
adb -s <serial> logcat -v time > tmp/rem-stress-<callsign>.log
adb -s <serial> shell screencap -p /sdcard/rem-stress.png
adb -s <serial> pull /sdcard/rem-stress.png tmp/rem-stress-<callsign>.png
```

## Build And Install Gate

Before live testing, prove the build is coherent:

```powershell
npm --workspace apps/mobile run typecheck
npm --workspace apps/mobile run build
npm --workspace packages/node-client run build
cargo test --manifest-path crates/reticulum_mobile/Cargo.toml
```

Then build and install the same Android artifact on all USB phones:

```powershell
Push-Location apps/mobile
npx cap sync android
Pop-Location
Push-Location apps/mobile/android
cmd /c gradlew.bat clean assembleDebug
Pop-Location
adb -s <phone-a-serial> install -r apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk
adb -s <phone-b-serial> install -r apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk
adb -s <phone-c-serial> install -r apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk
```

Record any command that is skipped and why. A failed build or install is a
release blocker.

## Test Method

Use three rounds. In each round, one USB phone is the originator and every other
phone is a receiver.

- Round 1: Phone A sends/creates/updates/deletes; Phone B and Phone C verify.
- Round 2: Phone B sends/creates/updates/deletes; Phone A and Phone C verify.
- Round 3: Phone C sends/creates/updates/deletes; Phone A and Phone B verify.

For each feature:

- Perform one normal operation.
- Perform one edit/update operation when the feature supports it.
- Perform one delete/cancel/deactivate operation when the feature supports it.
- Verify remote phones receive the correct state.
- Kill and restart REM on at least one receiver and verify state persists.
- Toggle background/foreground on at least one receiver and verify state is not
  lost.
- Repeat under burst load or concurrent operation when the feature is
  communication-sensitive.

## Required Function Matrix

### 1. Startup, Splash, Setup, And Navigation

- Fresh start shows the REM splash/version and reaches the expected first screen.
- First-run setup wizard can configure callsign, team color, Reticulum mode, and
  required permissions.
- Settings can relaunch setup wizard.
- Main navigation reaches Dashboard, Chat, Action Messages, Events, Checklists,
  Map, Peers, and Settings.
- Android back navigation returns through history and falls back to Dashboard.
- Help screens for message status and event/MECP open and return correctly.
- App kill/restart preserves setup choices and current operational state.

Pass condition: all phones can start, configure, navigate, restart, and preserve
settings without losing identity or peer state.

### 2. Settings And Runtime Readiness

- Update callsign/display name and verify announces reflect the new identity.
- Update team color and verify Action Message/Dashboard categorization.
- Validate default TCP community server selection.
- Change and persist TCP endpoint selection.
- Confirm autonomous mode behavior.
- Confirm disabled RCH hub directory does not expose connected-only routing
  states.
- Change telemetry publish interval above 60 seconds and verify save/persist.
- Configure SOS settings and PIN behavior.
- Exercise notification and location permission paths.

Pass condition: settings persist across app restart and any setting that affects
network identity is visible to other phones through announces or received
payloads.

### 3. Peers And Reticulum Connectivity

- Each phone announces itself.
- Each other phone discovers the announce.
- Save each trusted peer.
- Connect to each saved peer.
- Disconnect and reconnect.
- Request peer identity.
- Use manual connect and verify node-not-running/readiness errors are clear when
  intentionally triggered.
- Verify peer sharing drives the rest of the app: Chat, Action Messages, Events,
  Checklists, Telemetry, and SOS all target saved or discovered peers correctly.

Pass condition: all three USB phones see each other as REM-capable peers and can
recover from disconnect/reconnect without stale or duplicate peer rows.

### 4. Chat And LXMF Messaging

- Send direct chat message A -> B/C, B -> A/C, and C -> A/B.
- Verify message body, sender name, timestamp, delivery state, and conversation
  selection on every receiver.
- Send messages containing links and verify links render correctly.
- Open a chat thread from peer status and from telemetry popup.
- Delete a chat thread and verify the intended thread is removed without
  corrupting other conversations.
- Burst test: send at least 100 chat messages rotating among A, B, and C.
- Restart sender and receiver during or after burst and verify chat history
  persists on both sides.
- Test retry/cancel paths if a message is intentionally sent while disconnected.

Pass condition: chat is durable. The sender-side and receiver-side conversation
history must not reset, vanish, duplicate incorrectly, or select the wrong peer.

### 5. Action Emergency Messages

- Create an Action Emergency Message from each phone.
- Verify remote phones display inbound synced messages read-only where
  appropriate.
- Edit status and cycle status.
- Filter by team color.
- Open status help.
- Delete a local Action Message and verify dashboard totals update.
- Burst test: create at least 50 Action Messages across phones with mixed team
  colors and statuses.

Pass condition: Action Messages replicate correctly, keep the right status, feed
Dashboard rings, and do not let inbound read-only records be edited incorrectly.

### 6. Dashboard

- Verify readiness metrics from stored Action Messages.
- Verify peer count, checklist counts, recent activity, and status summaries
  after each feature creates data.
- Verify Dashboard updates after Action Message status changes, checklist task
  changes, event creation, telemetry receipt, and SOS activation/deactivation.
- Restart the app and verify Dashboard reconstructs the same operational picture.

Pass condition: Dashboard always matches the underlying live state from other
screens and never reports stale critical status after updates or deletes.

### 7. Events And MECP

- Create MECP event timeline entries from each phone.
- Verify category, severity, body text, sender, and timestamp on every receiver.
- Filter by severity and category.
- Delete/remove an event and verify receivers converge.
- Exercise structured MECP encode/decode cases visible in the UI.
- Open Events help and verify it does not break navigation.
- Burst test: create at least 50 events across phones with mixed categories and
  severities.

Pass condition: events replicate through the mission/LXMF path, stay compatible
with MECP expectations, and receivers converge after create/filter/delete flows.

### 8. Checklists

- List built-in checklist templates.
- Import a checklist template CSV if the UI exposes import on the installed
  build.
- Create checklist from template.
- Create online/shared checklist.
- Open checklist detail on every phone.
- Join checklist from another phone.
- Upload/publish checklist if the action is available.
- Update checklist title, description, or start time.
- Set task status from each phone.
- Add a task row.
- Edit a task cell.
- Set task row style/background and line-break behavior.
- Delete a task row.
- Delete the checklist.
- Restart sender and receiver and verify final checklist state persists.
- Concurrent stress: two phones update different task rows while a third phone
  changes task status, then verify deterministic convergence.

Pass condition: checklist state is consistent across Rust runtime, node-client,
Pinia projection, and UI on all phones. No receiver may show an impossible mix of
old and new checklist state after restart.

### 9. Telemetry And Map

- Enable telemetry on each phone with location permission granted.
- Publish one local telemetry fix from each phone.
- Verify Map shows live markers for peers.
- Verify stale and expired marker behavior.
- Verify close positions cluster into count bubbles when zoomed out.
- Open telemetry popup and launch chat thread for matched peer.
- Delete local telemetry and verify marker removal.
- Verify telemetry is hidden for cancelled SOS emergencies.
- Stress test with rapid foreground/background transitions while telemetry is
  enabled.

Pass condition: peer positions appear, age out, cluster, link to chat, and clear
correctly across phones without leaking cancelled SOS locations.

### 10. SOS Emergency

- Configure SOS settings on each phone.
- Trigger SOS from each phone.
- Verify SOS overlay/card appears locally.
- Verify every receiver displays active SOS alert.
- Verify SOS telemetry/location appears on Map.
- Verify chat can open from SOS-related peer/location context.
- Submit SOS telemetry updates.
- Deactivate SOS with the configured PIN if enabled.
- Verify receivers clear or mark cancelled SOS state.
- Restart one receiver during active SOS and verify it reconstructs the active
  alert.

Pass condition: SOS activation, telemetry, receiver alerting, and cancellation
are reliable and cannot leave a false active emergency after deactivation.

### 11. Propagation, Sync, And Recovery

- Select or clear active propagation node when available.
- Request LXMF sync.
- Confirm sync status updates.
- Run a delayed-delivery scenario: disconnect one phone, generate Chat, Action
  Messages, Events, Checklists, Telemetry, and SOS changes, reconnect, request
  sync, and verify the phone catches up.
- Force-stop REM on one receiver during active traffic, restart it, and verify
  no data loss or duplicate user-visible records.
- Reboot one phone if practical and verify recovery.

Pass condition: disconnected phones can recover current state through live
transport/sync without corrupting local projections.

### 12. Notifications And Background Behavior

- Background each phone and send Chat, Action Message, Event, Checklist, and SOS
  updates from another phone.
- Verify notifications appear where expected.
- Tap notifications and verify deep links or app foreground behavior.
- Verify no duplicate notification storm under burst traffic.
- Verify foreground UI still updates after returning from background.

Pass condition: background behavior is reliable and does not drop emergency
traffic or overload the operator with duplicates.

## Load And Chaos Requirements

After the function matrix passes once, run an extended stress window:

- Duration: at least 60 minutes.
- Phones: all three USB phones active; optional extra phones receiving.
- Traffic mix per 10 minutes:
  - 30 chat messages.
  - 15 Action Emergency Messages or status updates.
  - 15 Events.
  - 10 checklist task/cell/status changes.
  - Continuous telemetry when permissions allow.
  - 1 SOS activation/deactivation drill per phone across the full window.
- Chaos actions:
  - Put one receiver in background.
  - Force-stop and restart one receiver.
  - Disconnect/reconnect USB for one device.
  - Toggle network/Reticulum availability if safe for the test environment.
  - Request LXMF sync after reconnect.

Pass condition: no crash, no unrecoverable send failure, no persistent divergent
state, no runaway notification loop, no lost SOS cancellation, and no data loss
after restart.

## Severity Rules

- `CRITICAL`: crash, data loss, false SOS state, missed SOS, app cannot start,
  app cannot send/receive, release build cannot install.
- `HIGH`: cross-device divergence, chat/history loss, checklist corruption,
  incorrect emergency status, recovery failure after reconnect, repeated
  delivery failure.
- `MEDIUM`: wrong count, stale UI until manual refresh, confusing recoverable
  error, duplicate non-critical notification.
- `LOW`: cosmetic issue, copy issue, layout issue that does not block operation.

Release readiness requires zero open `CRITICAL` and zero open `HIGH` findings.
Any open `MEDIUM` finding must have a documented operator impact and a release
decision. `LOW` findings may be deferred only if they do not affect emergency
operations.

## Release Acceptance

The outcome can be recorded as "REM is rock solid for this release" only when:

- Build/typecheck/Rust tests pass or any skipped command has an explicit,
  non-release-blocking reason.
- The same REM build is installed on all three USB phones.
- Every required function matrix row is `PASS`.
- Every communication feature has been verified source-to-receiver across at
  least two receiving phones.
- App kill/restart persistence has been verified for Chat, Action Messages,
  Events, Checklists, Telemetry, SOS, Peers, and Settings.
- The 60-minute stress window passes.
- No open critical or high defects remain.
- The execution log includes enough evidence for another engineer to audit the
  release decision.

## Execution Log Template

Fill this section during the live run.

### Run Metadata

- Date: 2026-05-23T08:29:48.0419325-03:00
- Tester: Codex live adb run
- Branch: `main`
- Commit: `93f7915838be957d3449af47ffcadec67aa14c40` (`Clarify peer action notifications`)
- REM version: `1.0.11`
- Android artifact:
  `apps/mobile/android/app/build/outputs/apk/release/reticulum-mobile-emergency-management-v1.0.11-release.apk`
- Worktree status before stress run: `## main...origin/main`,
  `D docs/REM_User_Manual_IT_v2.zip`,
  `?? docs/rem_manual_plain_english_v3.docx`, `?? stress_test.md`
- Build gate:
  - `npm --workspace apps/mobile run typecheck`: PASS
  - `npm --workspace apps/mobile run build`: PASS
  - `npm --workspace packages/node-client run build`: PASS
  - `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml`: PASS, 221 passed
  - Post-fix `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml`: PASS, 224 passed
  - Post peer-state/SQLite fix `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml`: PASS, 225 passed
  - Post peer-state/SQLite fix `npm --workspace apps/mobile run typecheck`: PASS
  - Post peer-state/SQLite fix `npm --workspace apps/mobile run build`: PASS
  - Post peer-state/SQLite fix `npm --workspace packages/node-client run build`: PASS
  - Post WAL-init SQLite fix `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml`: PASS, 225 passed
- Android packaging:
  - `npx cap sync android`: PASS
  - `cmd /c gradlew.bat clean assembleDebug`: PASS
  - Debug APK install: FAIL on all three phones due `INSTALL_FAILED_UPDATE_INCOMPATIBLE`
  - Root cause: installed package was release-signed; debug APK signature did not match
  - `cmd /c gradlew.bat assembleRelease`: PASS
  - Release APK install: PASS on all three USB phones
  - Post-fix `npx cap sync android`: PASS
  - Post-fix `cmd /c gradlew.bat clean assembleRelease`: PASS
  - Post-fix release APK reinstall: PASS on Phone A, Phone B, and Phone C
  - Post peer-state/SQLite fix `npx cap sync android`: PASS
  - Post peer-state/SQLite fix `cmd /c gradlew.bat clean assembleRelease`: PASS
  - Post peer-state/SQLite fix release APK reinstall: PASS on Phone A, Phone B, and Phone C
  - Post WAL-init SQLite fix `npx cap sync android`: PASS
  - Post WAL-init SQLite fix `cmd /c gradlew.bat clean assembleRelease`: PASS
  - Post WAL-init SQLite fix release APK reinstall: PASS on Phone A, Phone B, and Phone C
  - Post 60-minute peer-active remediation `cargo fmt --manifest-path crates/reticulum_mobile/Cargo.toml`: PASS
  - Post 60-minute peer-active remediation `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml active_link -- --test-threads=1`: PASS, 8 passed
  - Post 60-minute peer-active remediation `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml upsert_event_replicates_to_native_peer_projection`: PASS
  - Post 60-minute peer-active remediation `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml`: PASS, 225 passed
  - Post 60-minute peer-active remediation `npm --workspace apps/mobile run typecheck`: PASS
  - Post 60-minute peer-active remediation `npm --workspace packages/node-client run build`: PASS
  - Post 60-minute peer-active remediation `npm --workspace apps/mobile run build`: PASS
  - Post 60-minute peer-active remediation `./tools/codegen/generate-uniffi-bindings.ps1 -Language kotlin`: PASS; refreshed `libreticulum_mobile.so` in Android `jniLibs`
  - Post 60-minute peer-active remediation `npx cap sync android`: PASS
  - Post 60-minute peer-active remediation `cmd /c gradlew.bat assembleRelease`: PASS
  - Post 60-minute peer-active remediation APK verification: PASS; release APK contains `libreticulum_mobile.so` for `arm64-v8a`, `armeabi-v7a`, and `x86_64`
  - Post 60-minute peer-active remediation release APK reinstall: PASS on Phone A, Phone B, and Phone C
  - Post recovery-lane remediation `cargo fmt --manifest-path crates/reticulum_mobile/Cargo.toml`: PASS
  - Post recovery-lane remediation `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml recovery -- --test-threads=1`: PASS, 2 passed
  - Post recovery-lane remediation `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml saturated_mission -- --test-threads=1`: PASS, 1 passed
  - Post recovery-lane remediation `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml accepted_ack_sends_do_not_wait -- --test-threads=1`: PASS, 1 passed
  - Post recovery-lane remediation `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml -- --test-threads=1`: PASS, 229 passed
  - Post recovery-lane remediation `./tools/codegen/generate-uniffi-bindings.ps1 -Language kotlin`: PASS; refreshed Android UniFFI/native artifacts
  - Post recovery-lane remediation `npx cap sync android`: PASS
  - Post recovery-lane remediation `cmd /c gradlew.bat assembleRelease`: PASS
  - Post recovery-lane remediation release APK reinstall: PASS on Phone A, Phone B, and Phone C
- Baseline evidence folder: `tmp/stress-live/`

### Device Inventory

| Role | ADB serial | Model | Android | Callsign | Destination | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Phone A | `35031FDH2003N8` | Pixel 7 | 16 | Pixel | app `6521979f1165965b24731061ef4a6906`; LXMF `fb4c70e20cfac047b899ca2f3671b50a` | USB controlled; package `network.reticulum.emergency` versionName `1.0.11`, versionCode `261422011`, signature hash `726fb92e`; baseline screenshot `tmp/stress-live/phone-a-pixel7-baseline.png`; location and notification permissions granted |
| Phone B | `3C121JEKB11387` | Pixel 8a | 16 | Noemi | app `5ea3149a58998316f6c8200a006e14c2`; LXMF `34eb6bec21defd52736eaca1adff75eb` | USB controlled; package `network.reticulum.emergency` versionName `1.0.11`, versionCode `261422011`, signature hash `726fb92e`; baseline screenshot `tmp/stress-live/phone-b-pixel8a-baseline.png`; location and notification permissions granted |
| Phone C | `988b9b344135304639` | SM-G950W | 9 | S8 | app `0e252632c57fa999a15c4a05c0d1bae2`; LXMF `a1c8126d7cb806e6bde086d582b6cb0d` | USB controlled; package `network.reticulum.emergency` versionName `1.0.11`, versionCode `261422011`, signature hash `726fb92e`; baseline screenshot `tmp/stress-live/phone-c-smg950w-baseline.png`; location permission granted; Android 9 has no runtime notification permission |
| Phone D+ | | | | | | |

### Result Matrix

| Area | Source | Receivers | Operation | Evidence | Result | Defect/Fix |
| --- | --- | --- | --- | --- | --- | --- |
| Build/typecheck/Rust gate | repo | n/a | Source verification before live test | command output in current Codex run | PASS | n/a |
| Android packaging/install | repo | Phone A, Phone B, Phone C | Build release APK and install same artifact | `tmp/stress-live/`, package dumpsys output in current Codex run | PASS after switching from debug APK to release-signed APK | Debug APK install rejected by release-signed existing package; fixed by using release keystore artifact |
| Startup/setup/navigation | all phones | n/a | Force-stop and launch `network.reticulum.emergency/.MainActivity` | baseline screenshots and logcats in `tmp/stress-live/` | PASS for baseline launch only | Baseline launch reached Dashboard on all three phones. Phone C logs show repeated Android 9 WebView `PacProcessor` warnings, but Dashboard still rendered. The follow-up navigation sweep below covers route traversal; destructive first-run setup wizard coverage remains open. |
| Startup/navigation/help sweep | all phones | n/a | Force-stop/relaunch each phone; capture Dashboard, Chat, Action Messages, EAM status help, Events, MECP Events help, Checklists, Map, Peers, Settings, then Android Back from Settings | `tmp/stress-live/navigation-settings-20260523-161613`, including `phone-*-00-launch-dashboard.png/xml` through `phone-*-09-back-after-settings.png/xml`, `phone-*-03a-action-help.png/xml`, `phone-*-04a-events-help.png/xml`, and `driver.log` | PASS for non-destructive navigation/help/back | All three phones reached Dashboard after relaunch and rendered every primary REM route without crash or blank screen. EAM status help and MECP Events help opened on all phones. Android Back from Settings returned to Peers. Narrow log scan found no REM `FATAL EXCEPTION`, `ANR`, `rejectFromNative`, `IoError`, `ack timeout`, `NoPath`, or mission timeout signatures. |
| Settings/runtime readiness | Phone C | n/a | Open Settings, expand SOS Emergency, enable SOS, and trigger manual drill | `phone-c-settings-start.png`, `phone-c-settings-sos-open2.png`, `phone-c-settings-sos-enabled-scrolled.png`, `phone-c-sos-triggered-sos-c-094340.png` | PASS with scope note | Settings rendered READY; SOS Emergency was changed from disabled to enabled/manual-only, saved by the manual trigger path, and Phone C showed `SOS ACTIVE`. Broader settings import/export, node restart controls, setup wizard, and peer-list import/export remain untested. |
| Settings/runtime readiness sweep | all phones | n/a | Open Settings after relaunch and verify app READY state, unsaved-count state, node running/config mode, TCP endpoint count, telemetry cadence, RCH mode, saved peer count, and SOS setting state | `tmp/stress-live/navigation-settings-20260523-161613/phone-a-08-settings.png/xml`, `phone-b-08-settings.png/xml`, `phone-c-08-settings.png/xml`, and `driver.log` | PASS for read-only runtime readiness | Phone A Settings showed READY, `Unsaved 0`, Node Config auto, 4 TCP endpoints, telemetry every 600s, RCH Autonomous/no hub, 7 saved peers, SOS enabled/manual-only, and node running. Phone B showed READY, `Unsaved 0`, Node Config auto, 3 TCP endpoints, telemetry every 360s, RCH Autonomous/no hub, 3 saved peers, SOS disabled, and node running. Phone C showed READY, `Unsaved 0`, Node Config auto, 3 TCP endpoints, telemetry every 60s, RCH Autonomous/no hub, 4 saved peers, SOS enabled/manual-only, and node running. Edit/save/import/export and node restart controls remain open. |
| Setup/settings mutation matrix | all phones | n/a | Clean first-run setup wizard, change/save runtime settings, import/export peer settings, and exercise node restart controls | not yet executed | NOT RUN - RELEASE BLOCKER | The current sweep deliberately avoided destructive edits on the configured USB release fleet. This must be run in a planned destructive slot or on resettable devices before declaring the release rock solid. |
| Peers/connectivity | Phone A | Phone B, Phone C | Manual announce from Phone A | `phone-a-pixel7-announce-a.png`, `phone-b-pixel8a-announce-a.png`, `phone-c-smg950w-announce-a.png`, `*-announce-a-logcat.txt`, `*-announce-a-extended-logcat.txt` | PARTIAL | Phone A sent announce. Phone C received Pixel LXMF announce in extended logs, but clean PeerApp receipt was not visible on every receiver inside the initial window. |
| Propagation/sync/recovery | Phone A | propagation node | Manual LXMF sync | `phone-a-sync-a-logcat.txt`, `phone-b-sync-a-logcat.txt`, `phone-c-sync-a-logcat.txt`, `phone-a-sync-a.png` | PASS with warning | Phone A sync completed `available=19 fetched=19 imported=19 failed=0`. Warning: imported chat acknowledgement sends later logged `network error`; not yet reproduced as a user-visible failure. |
| Chat/LXMF | Phone B | Phone A | Direct chat `stressBtoPixel0852` | `phone-b-chat-b-to-pixel-0852.png`, `phone-a-chat-b-to-pixel-0852.png`, `phone-b-chat-b-to-pixel-0852-logcat.txt`, `phone-a-chat-b-to-pixel-0852-logcat.txt` | PASS | Phone B showed `Delivered`; Phone A showed inbound `stressBtoPixel0852` from Noemi at 8:53:14 AM. |
| Chat/LXMF | Phone A | Phone B | Direct chat `stressAtoNoemi0855` | `phone-a-chat-a-to-noemi-0855.png`, `phone-b-chat-a-to-noemi-0855.png`, `phone-a-chat-a-to-noemi-0855-logcat.txt`, `phone-b-chat-a-to-noemi-0855-logcat.txt` | PASS | Phone A showed `Delivered`; Phone B showed inbound `stressAtoNoemi0855` from Pixel at 8:56:53 AM. |
| Chat/LXMF | Phone C | Phone A | Direct chat `stressCtoPixel0919` | `phone-c-native-select-after-peers-refresh-ui.xml`, `phone-c-chat-c-to-pixel-0920.png`, `phone-a-chat-list-after-c-to-pixel-0920.png`, `phone-c-chat-c-to-pixel-0920-logcat.txt`, `phone-a-chat-c-to-pixel-0920-logcat.txt` | PASS | Phone C native peer picker listed `Poco`, `Noemi`, and `Pixel`; Phone C showed `Delivered`; Phone A log recorded `messageReceived` body `stressCtoPixel0919` at 9:20:14 AM and Chat list showed S8 `stressCtoPixel0919` `Received`. |
| Chat/LXMF load probe | Phone A | Phone B | Direct chat `loadA1` during post-fix load planning | `phone-a-chat-loadA1-before-send.png`, `phone-a-chat-loadA1-after-send.png`, `phone-b-chat-loadA1-receiver.png`, `phone-a-chat-loadA1-logcat.txt`, `phone-b-chat-loadA1-logcat.txt` | PASS | Phone A sent `loadA1` to the current Noemi thread and showed `Delivered`. Phone B received the body in its chat log with state `Received`, confirming the current A->B chat path still worked after the peer-state, mission-ack, and SQLite fixes. |
| Chat/LXMF | Phone A | old saved Noemi identity | Direct chat `stress-A-to-Noemi-084303` | earlier current-run screenshots/logs in `tmp/stress-live/` | INVALID/FAIL | Target was an old saved Noemi thread, not current Phone B identity; Phone A marked failed and log said saved peer was not directly reachable. This row is excluded from A->B pass criteria but documents stale thread risk. |
| Peers/connectivity | Phone A | Phone B | Add current Phone B identity from discovered peers | `phone-a-current-before-b-announce.png`, `phone-a-after-add-current-b.png`, `phone-a-saved-current-b-after-add.png` | PASS | Phone A saved current Noemi app destination `5ea3149a58998316f6c8200a006e14c2` after discovering it. Phone A then showed current Noemi as `PEER`/`CONNECTED`, resolving the earlier A-origin sender setup gap for Phone B. |
| Peers/connectivity | Phone C | Phone A, Phone B | Saved peer recovery before and after force-stop | `phone-c-peers-page.png`, `phone-c-after-connect-all.png`, `phone-c-post-announce-observation.png`, `phone-c-after-restart.png`, `phone-c-after-restart-peers-saved-tab.png`, `phone-c-after-restart-peers-saved-tab-ui.xml` | HIGH - FIXED/RETESTED | Before force-stop, Phone C Peers showed `4 managed peers | 0 online`; `Connect all` timed out for current Noemi/Pixel and fresh announces did not recover the stale UI. After force-stop/relaunch, Phone C recovered to `4/2 READY`, Peers showed Noemi and Pixel `CONNECTED`, telemetry sent direct/acknowledged to both, and C->A chat passed. The later peer-state/SQLite fix verification row showed current saved peers connected immediately after relaunch, and the post-remediation 60-minute window held Phone C at `4/2 READY`, so this is no longer an open high finding. |
| Peers/connectivity fix verification | Phone C | Phone A, Phone B | Rebuild after canonical active-link and SQLite busy-timeout fixes, relaunch all phones, open Phone C saved Peers, then tap `Connect all` | `phone-c-peers-post-sqlite-peerstate-fix-before-connect.png`, `phone-c-peers-post-sqlite-peerstate-fix-before-connect-logcat.txt`, `phone-c-peers-post-sqlite-peerstate-fix-after-connect-all.png`, `phone-c-peers-post-sqlite-peerstate-fix-after-connect-all-scrolled.png`, `phone-c-peers-post-sqlite-peerstate-fix-after-connect-all-logcat.txt` | PASS for current saved peers | Phone C now showed `4/2 READY`, `4 Peers`, `2 Connected`, and `4 managed peers / 2 online` immediately after relaunch. Current Noemi (`5ea3149a58998316f6c8200a006e14c2`) and Pixel (`6521979f1165965b24731061ef4a6906`) both displayed `CONNECTED` before and after `Connect all`. Old stale saved peers such as Peregrine remained `DISCONNECTED`, and timeout log lines after `Connect all` appear tied to stale/non-current saved destinations rather than current Phone A/B reachability. |
| Peers/connectivity navigation sweep | all phones | all phones | Open Peers during the post-remediation navigation sweep after relaunch | `tmp/stress-live/navigation-settings-20260523-161613/phone-a-07-peers.png/xml`, `phone-b-07-peers.png/xml`, `phone-c-07-peers.png/xml`, and `driver.log` | PASS for current peer readiness | Phone A showed `7 Peers`, `1 Connected`, and Dashboard `7/2 READY`; Phone B showed `3 Peers`, `1 Connected`, and Dashboard `3/2 READY`; Phone C showed `4 Peers`, `2 Connected`, and Dashboard `4/2 READY`. This confirms the current USB test fleet remained discoverable after the patched 60-minute run and relaunch. Stale saved-peer cleanup UX remains a non-blocking follow-up unless it produces current-peer reachability loss again. |
| Action Emergency Messages | Phone A | Phone C; Phone B not observed | Add EAM `StressA092940` | `phone-a-eam-StressA092940-after-submit.png`, `phone-c-eam-StressA092940-receiver-screen.png`, `phone-c-eam-StressA092940-receiver-logcat.txt`, `phone-b-eam-StressA092940-receiver-screen.png`, `phone-b-eam-StressA092940-receiver-logcat.txt` | PARTIAL | Phone C received and displayed `StressA092940` with `mission.registry.eam.upsert` logs and acknowledgement back to Phone A. Phone B did not show or log this A-originated EAM in the observation window. |
| Action Emergency Messages | Phone A | Phone B, Phone C | Add EAM `StressA1150` after saving current Phone B identity | `phone-a-eam-form-StressA1150.png`, `phone-a-eam-StressA1150-after-submit.png`, `phone-b-eam-StressA1150-screen.png`, `phone-c-eam-StressA1150-screen-top.png`, `phone-a-eam-StressA1150-logcat.txt`, `phone-b-eam-StressA1150-logcat.txt`, `phone-c-eam-StressA1150-logcat.txt` | PASS | Phone A sent `mission.registry.eam.upsert` direct to current Phone B LXMF `34eb6bec21defd52736eaca1adff75eb` and Phone C LXMF `a1c8126d7cb806e6bde086d582b6cb0d`; both receivers displayed `StressA1150` and sent accepted acknowledgements. |
| Action Emergency Messages load probe | Phone A | Phone B, Phone C | Add EAM `EamLoad1` during post-fix load planning | `phone-a-eam-EamLoad1-before-submit.png`, `phone-a-eam-EamLoad1-after-submit.png`, `phone-b-eam-EamLoad1-receiver.png`, `phone-c-eam-EamLoad1-receiver.png`, `phone-a-eam-EamLoad1-logcat.txt`, `phone-b-eam-EamLoad1-logcat.txt`, `phone-c-eam-EamLoad1-logcat.txt` | PASS for mission transport | Phone A created `EamLoad1` and sent `mission.registry.eam.upsert` direct to current Phone B and Phone C. Phone B and Phone C logs show receipt and accepted acknowledgement. Receiver screenshots were captured while those phones were still on Events, so this row proves transport/acknowledgement but is not a substitute for the full EAM UI matrix. |
| Action Emergency Messages | Phone C | Phone A, Phone B | Add EAM `StressC093154` | `phone-c-eam-StressC093154-after-submit.png`, `phone-a-eam-StressC093154-receiver-screen.png`, `phone-b-eam-StressC093154-receiver-screen.png`, `phone-a-eam-StressC093154-receiver-logcat.txt`, `phone-b-eam-StressC093154-receiver-logcat.txt` | PASS | Phone C created `StressC093154`; both Phone A and Phone B displayed the row and logged `mission.registry.eam.upsert` receipt/acknowledgement. |
| Dashboard | all phones | n/a | Re-open dashboard after Chat/EAM/Event/Telemetry traffic | `phone-a-dashboard-post-matrix-0941.png`, `phone-b-dashboard-post-matrix-0941.png`, `phone-c-dashboard-post-matrix-0941.png`, matching `*-ui.xml` files | PASS | Dashboard rendered READY on all phones with updated aggregate cards and activity feed. Phone A and Phone B both showed the new S8 event `MECP/2/P01 EventC093718` and EAM update; Phone C showed the A-origin event/EAM plus peer-action failure history. |
| Events/MECP | Phone A | Phone C; Phone B not observed | Add MECP event `EventA093522` | `phone-a-event-EventA093522-after-submit.png`, `phone-c-event-EventA093522-receiver-screen.png`, `phone-b-event-EventA093522-receiver-screen.png`, `phone-a-event-EventA093522-logcat.txt` | PARTIAL | Phone A created `EventA093522` and sent/acknowledged `mission.registry.log_entry.upsert` direct to Phone C. Phone C displayed the event. Phone B did not display or log this A-originated event in the observation window. |
| Events/MECP | Phone A | Phone B, Phone C | Add MECP event `EventA1204` after saving current Phone B identity | `phone-a-event-EventA1204-after-submit.png`, `phone-b-events-EventA1204-check.png`, `phone-c-events-EventA1204-check.png`, `phone-a-event-EventA1204-logcat.txt`, `phone-b-event-EventA1204-logcat.txt`, `phone-c-event-EventA1204-logcat.txt` | HIGH - FIXED/RETESTED | Phone A locally showed `EventA1204`, but Phone B and Phone C did not receive it. Phone A log showed `rejectFromNative code=IoError message=io error` and `[events] create failed: io error`, with no clean `mission.registry.log_entry.upsert` for `EventA1204`. Root cause was a lock-sensitive SQLite connection path that re-ran `PRAGMA journal_mode=WAL` on every connection under live traffic. |
| Events/MECP fix verification | Phone A | Phone B, Phone C | Rebuild after moving SQLite WAL setup into initialization, reinstall on all phones, then add MECP event `EventA1229` | `phone-a-post-sqlite-wal-init-launch.png`, `phone-b-post-sqlite-wal-init-launch.png`, `phone-c-post-sqlite-wal-init-launch.png`, `phone-a-event-EventA1229-after-submit-post-wal-init.png`, `phone-b-event-EventA1229-receiver-post-wal-init.png`, `phone-c-event-EventA1229-receiver-post-wal-init.png`, `phone-a-event-EventA1229-post-wal-init-logcat.txt`, `phone-b-event-EventA1229-post-wal-init-logcat.txt`, `phone-c-event-EventA1229-post-wal-init-logcat.txt` | PASS | Phone A created `EventA1229`; Phone B and Phone C displayed it at the top of Events. Phone A sent direct `mission.registry.log_entry.upsert` to Phone C and Phone B and received accepted acknowledgements from both. Phone B and Phone C logs show receipt of the command from Phone A and accepted acknowledgement back to Phone A, with no `IoError` in the clean evidence window. |
| Propagation/sync/recovery | Phone A | Phone B | Receiver force-stop/relaunch persistence for `EventA1229` | `phone-b-after-force-stop-relaunch-EventA1229.png`, `phone-b-events-EventA1229-after-restart.png`, `phone-b-events-EventA1229-after-restart-logcat.txt` | PASS | Phone B was force-stopped and relaunched after receiving `EventA1229`. The Events screen still showed `EventA1229` at the top with the correct `MECP/2/P01 EventA1229` body and Pixel sender, proving persisted receiver state for this event path. |
| Notifications/background | Phone A | Phone C backgrounded; Phone B foreground receiver | Put Phone C on the launcher, create MECP event `EventBg1244` from Phone A, then bring Phone C back | `phone-a-event-EventBg1244-before-submit-bg-receiver.png`, `phone-a-event-EventBg1244-after-submit-bg-receiver.png`, `phone-b-event-EventBg1244-receiver-bg-test.png`, `phone-c-event-EventBg1244-after-background-receiver.png`, `phone-a-event-EventBg1244-bg-receiver-logcat.txt`, `phone-b-event-EventBg1244-bg-receiver-logcat.txt`, `phone-c-event-EventBg1244-bg-receiver-logcat.txt` | PASS for event receive while backgrounded | Phone C was backgrounded before send. After Phone A created `EventBg1244`, Phone B displayed it while foregrounded and Phone C displayed it at the top of Events when brought back. Phone C logs show `mission.registry.log_entry.upsert` received from Phone A and accepted acknowledgement sent back. This does not yet cover Android notification shade behavior or all feature types while backgrounded. |
| Events/MECP | Phone C | Phone A, Phone B | Add MECP event `EventC093718` | `phone-c-event-EventC093718-after-submit2.png`, `phone-a-event-EventC093718-receiver-screen2.png`, `phone-b-event-EventC093718-receiver-screen2.png`, `phone-c-event-EventC093718-logcat2.txt` | PASS | Phone C created `EventC093718`; Phone A and Phone B both displayed it. Phone C log showed direct `mission.registry.log_entry.upsert` sends to both `fb4c70e20cfac047b899ca2f3671b50a` and `34eb6bec21defd52736eaca1adff75eb` with accepted acknowledgements. |
| Checklists | Phone C, then Phone B | Phone A, Phone B, Phone C | Create `StressCL094655` from template on Phone C, open detail on all phones, mark first task complete on Phone B, then attempt Phone C detail sync | `phone-c-checklist-StressCL094655-after-create.png`, `phone-a-checklist-StressCL094655-receiver.png`, `phone-b-checklist-StressCL094655-receiver.png`, `phone-a-checklist-StressCL094655-detail-before-toggle.png`, `phone-b-checklist-StressCL094655-detail-before-toggle.png`, `phone-c-checklist-StressCL094655-detail-before-toggle.png`, `phone-a-checklist-StressCL094655-after-b-toggle-mini-bic.png`, `phone-b-checklist-StressCL094655-after-b-toggle-mini-bic.png`, `phone-c-checklist-StressCL094655-after-b-toggle-mini-bic.png`, `phone-a-checklist-StressCL094655-after-b-toggle-mini-bic-plus85s.png`, `phone-b-checklist-StressCL094655-after-b-toggle-mini-bic-plus85s.png`, `phone-c-checklist-StressCL094655-after-b-toggle-mini-bic-plus85s.png`, `phone-a-checklist-StressCL094655-after-c-sync-recovery.png`, `phone-b-checklist-StressCL094655-after-c-sync-recovery.png`, `phone-c-checklist-StressCL094655-after-c-sync-recovery.png`, matching logcats | HIGH - FIXED/RETESTED | Created checklist replicated to Phone A and Phone B and opened on all phones. Phone B task completion converged to Phone A and Phone B, but Phone C stayed at `0% COMPLETE | 12 PENDING | 0 LATE | 0 DONE` after 25 seconds, after an additional 85 seconds, and after manual Phone C detail sync. Phone B log showed the C-targeted `checklist.task.status.set` direct send stayed `Sent` with no accepted acknowledgement; Phone C log did not receive the command. Later fix verification with `StressAck1027` and the load probe showed all three phones converging with accepted acknowledgements and no `checklist.task.status.set` timeout, so this transport defect is no longer an open high finding. Broader checklist CRUD/import/export coverage remains incomplete. |
| Checklists fix verification | Phone C, then Phone B | Phone A, Phone B, Phone C | Rebuild after per-destination mission send serialization, create `StressFix1010`, mark first task complete on Phone B | `phone-a-StressFix1010-after-b-toggle-postfix-45s.png`, `phone-b-StressFix1010-after-b-toggle-postfix-45s.png`, `phone-c-StressFix1010-after-b-toggle-postfix-45s.png`, matching logcats | PARTIAL FIX | Per-destination mission serialization fixed the user-visible checklist divergence: all three phones converged to `8% COMPLETE | 11 PENDING | 0 LATE | 1 DONE`. Late logs still showed the C-targeted accepted acknowledgement reached Phone B only after the original sender had already timed out, so delivery tracking was not clean enough for release confidence. |
| Checklists fix verification | Phone C, then Phone B | Phone A, Phone B, Phone C | Rebuild after dedicated `MissionAck` send lane, create `StressAck1027`, join from Phone B, mark first task complete on Phone B, and hold past the previous ack timeout window | `phone-c-StressAck1027-after-create.png`, `phone-a-StressAck1027-receiver.png`, `phone-b-StressAck1027-receiver.png`, `phone-a-StressAck1027-detail-before-toggle.png`, `phone-b-StressAck1027-detail-before-toggle.png`, `phone-c-StressAck1027-detail-before-toggle.png`, `phone-b-StressAck1027-after-join.png`, `phone-b-StressAck1027-after-oldcoord-toggle.png`, `phone-a-StressAck1027-after-b-toggle-success-45s.png`, `phone-b-StressAck1027-after-b-toggle-success-45s.png`, `phone-c-StressAck1027-after-b-toggle-success-45s.png`, `phone-a-StressAck1027-after-b-toggle-success-late.png`, `phone-b-StressAck1027-after-b-toggle-success-late.png`, `phone-c-StressAck1027-after-b-toggle-success-late.png`, matching late logcats | PASS for checklist task status convergence and acknowledgement | `StressAck1027` replicated to Phone A and Phone B. After Phone B joined and completed the first task, all three phones showed `8% COMPLETE | 11 PENDING | 0 LATE | 1 DONE` at 45 seconds and again past the previous timeout window. Phone B late log shows the A-targeted status command acknowledged at 10:35:09 and the C-targeted status command acknowledged at 10:35:38, with no `checklist.task.status.set` ack timeout or `TimedOut` entry. Phone C log shows it received the B-origin status command at 10:35:21 and sent the accepted acknowledgement at 10:35:43. Residual note: checklist creation and join traffic still produced some propagation fallback ack-timeout lines, so the broader load window remains mandatory. |
| Checklists load probe | Phone A | Phone B, Phone C | In existing `StressAck1027`, mark pending task `Compact headlamp` complete from Phone A and verify receivers | `phone-a-checklist-current-before-toggle.png`, `phone-a-checklist-after-scroll-for-pending.png`, `phone-a-checklist-StressAck1027-toggle-compact-after.png`, `phone-b-checklist-StressAck1027-toggle-compact-after.png`, `phone-c-checklist-StressAck1027-toggle-compact-after.png`, `phone-b-checklists-after-toggle-nav.png`, `phone-c-checklists-after-toggle-nav.png`, `phone-b-checklist-StressAck1027-detail-after-compact-toggle.png`, `phone-c-checklist-StressAck1027-detail-after-compact-toggle.png`, `phone-a-checklist-StressAck1027-toggle-compact-logcat.txt`, `phone-b-checklist-StressAck1027-toggle-compact-logcat.txt`, `phone-c-checklist-StressAck1027-toggle-compact-logcat.txt` | PASS | Phone A toggled `Compact headlamp` from pending to completed. Phone A sent `checklist.task.status.set` direct to Phone B and Phone C; both receivers logged receipt and accepted acknowledgements. Phone B and Phone C detail screens then showed `17% COMPLETE | 10 PENDING | 0 LATE | 2 DONE`, with `Compact headlamp` completed. |
| Telemetry/Map | all phones | all phones | Open Map and verify live telemetry projection | `phone-a-map-telemetry-0940.png`, `phone-b-map-telemetry-0940.png`, `phone-c-map-telemetry-0940.png`, matching `*-ui.xml` files | PARTIAL | Map rendered on all phones. Phone A UI reported `Live telemetry: 3`, Phone B `Live telemetry: 2`, Phone C `Live telemetry: 2`; peer markers were visible in UI trees. Stale aging, clustering, marker deletion, cancelled SOS hiding, and popup-to-chat behavior remain untested. |
| Telemetry/Map navigation sweep | all phones | all phones | Re-open Map after post-remediation relaunch and 60-minute run | `tmp/stress-live/navigation-settings-20260523-161613/phone-a-06-map.png/xml`, `phone-b-06-map.png/xml`, `phone-c-06-map.png/xml`, and `driver.log` | PASS for current live telemetry projection | All three Map views rendered with `Live telemetry: 2`, `Stale telemetry: 0`, and `SOS alerts: 0`; Phone C also showed S8 and Noemi map markers. This confirms the steady-state telemetry projection was healthy after the long run. Map edge cases remain open: stale aging, clustering, marker deletion, cancelled SOS hiding, and popup-to-chat behavior. |
| SOS | Phone C | Phone A, Phone B | Enable SOS, trigger manual SOS, verify receiver alert/message, then deactivate | `phone-c-sos-triggered-sos-c-094340.png`, `phone-a-sos-receiver-sos-c-094340.png`, `phone-b-sos-receiver-sos-c-094340.png`, `phone-c-sos-cancelled-sos-c-094340.png`, `phone-a-sos-cancelled-sos-c-094340.png`, `phone-b-sos-cancelled-sos-c-094340.png`, matching logcats | PARTIAL | Activation propagated to Phone A and Phone B: both dashboards showed `SOS! I need help... Battery: 100%`, and logs recorded `sosAlertChanged` active true. Deactivation propagated to both: dashboards showed `SOS Cancelled - I am safe.`, and logs recorded `sosAlertChanged` active false/cancelled with accepted acknowledgements. Still untested: restart one receiver during active SOS, SOS map popup/chat flow, audio capture, PIN deactivation, and long-window repeated drills. |
| Propagation/sync/recovery | all phones | all phones | Restart sender/receiver during active Chat, EAM, Event, Checklist, SOS, and mixed-load traffic; verify convergence, persistence, no duplicate fan-out, and no stuck delivery state | `tmp/stress-live/recovery-drill-wrapper-20260523-213259`, `tmp/stress-live/recovery-drills-20260523-213259`, final XML and logcat captures in that folder | HIGH - OPEN | Recovery-lane remediation was installed and exercised, and `mission-recovery` sends appeared in logs. Chat A->B `RECChat213356`, EAM A->B/C `RECEam213356`, checklist status recovery, and SOS cancel propagation had passing evidence. The drill still failed release criteria: C-origin event `RECEvent213356` timed out on Phone A and Phone B and was not visible on Phone A after restart; mixed burst chat `RECBurstChat215339` timed out on Phone B; mixed burst event `RECBurstEvent215339` timed out on Phone B; final scan also found one native bridge `rejectFromNative code=NetworkError`. This is an open high release blocker. |
| Notifications/background | all phones | all phones | Background receivers, send Chat/EAM/Event/Checklist/SOS, verify Android notification shade behavior, then resume and verify in-app state | `EventBg1244` background receiver evidence; failing diagnostic `tmp/stress-live/background-notifications-20260523-172233`; fixed/pass run `tmp/stress-live/background-notifications-20260523-180449`; final driver result `chatB=True eamB=True eventB=True checklistB=True sosB=True sosCancelled=True`; final end-log scan for crashes, `rejectFromNative`, `IoError`, `ack timeout`, `NoPath`, and mission timeouts returned no hits | PASS for focused notification matrix | Event receive while Phone C was backgrounded passed. The earlier focused background run proved Chat and SOS notification behavior but exposed missing EAM/Event/Checklist notification-shade coverage. After adding native background notifications from inbound mission packets and tightening the checklist driver tap bounds, the fixed run passed: Chat `NBChat180521` notification matched and tapped on Phone B; EAM `NBEam180521` reached Phone B and Phone C and both matched notifications; Event `NBEvent180521` notified Phone A and Phone B; checklist `checklist.task.status.set` reached Phone B and Phone C and both matched `Checklist updated`; SOS active notified Phone B and Phone C; SOS cancellation matched on Phone C; all B-side notification taps returned true. |
| 60-minute stress window | Phone A, Phone B, Phone C | Phone A, Phone B, Phone C | 60-minute mixed live run using USB automation: A sends two chat messages to B per cycle, B creates one Action Emergency Message per cycle, C creates one Event/MECP entry per cycle, with screenshots/logcats every five cycles and final end capture | `tmp/stress-live/mixed-load-20260523-133034`, `long-run-20260523-133034.stdout.txt`, `long-run-20260523-133034.stderr.txt`, `cycle-005-*`, `cycle-010-*`, `cycle-015-*`, `cycle-020-*`, `cycle-025-*`, `end-*` | HIGH - FIXED/RETESTED | Automation completed `CYCLES=26` and `FAILURES=0`, but product state diverged. Phone A end screenshot showed A->B chat `MLA026a142920` and `MLA026b142920` delivered with `7/3 READY`. Phone B end screenshot showed latest EAM `BML026142920` and `3/1 READY`. Phone C end screenshot showed latest local events `CEV026142920` and `CEV025142651`, but only `4/0 READY`. Logs show B->C EAM `BML002133255` timed out with `ack timeout` to C LXMF `a1c8126d7cb806e6bde086d582b6cb0d`; after that, B-origin EAM fan-out kept targeting Phone A only for later IDs including `BML010135118`, `BML015140302`, `BML020141451`, `BML025142651`, and `BML026142920`. C-origin event IDs such as `CEV025142651` and `CEV026142920` appeared in Phone C UI but had no matching A/B logcat evidence. The post-remediation 60-minute window below passed without the peer drop, fan-out loss, or C-origin event evidence gap, so this high finding is fixed/retested rather than open. |
| Post-remediation smoke | Phone A, Phone B, Phone C | Phone A, Phone B, Phone C | Reinstall patched release APK, relaunch all phones, run two mixed cycles with hardened chat-thread setup | `tmp/stress-live/post-peer-active-20260523-1433`, `tmp/stress-live/mixed-load-20260523-144802` | PASS as smoke only | Patched APK installed on all three phones. Relaunch baseline showed Phone C recovered to `4/2 READY`. Two-cycle smoke completed `CYCLES=2`, `FAILURES=0`; Phone A showed `MLA001a144806`, `MLA001b144806`, `MLA002a145011`, and `MLA002b145011` delivered to Noemi, Phone B showed latest EAM `BML002145011` with `3/2 READY`, and Phone C showed latest local events `CEV002145011`/`CEV001144806` with `4/2 READY`. Smoke logs had no REM `ack timeout`, `IoError`, `rejectFromNative`, or mission timeout lines. This does not replace the mandatory 60-minute post-fix rerun. |
| Post-remediation 60-minute stress window | Phone A, Phone B, Phone C | Phone A, Phone B, Phone C | Same mixed load as the failed run, using patched APK and hardened driver | `tmp/stress-live/mixed-load-20260523-145347`, `long-run-post-patch-20260523-145346.stdout.txt`, `long-run-post-patch-20260523-145346.stderr.txt`, `cycle-005-*`, `cycle-010-*`, `cycle-015-*`, `cycle-020-*`, `end-*`, `post-run-events-receiver-check/phone-a-events-post-run.png`, `post-run-events-receiver-check/phone-b-events-post-run.png` | PASS for the repaired mixed-load window | Completed 2026-05-23T15:54:22-03:00 with `CYCLES=22` and `FAILURES=0`. Checkpoints at cycles 5, 10, 15, and 20 and final end capture showed Phone A chat remaining delivered to Noemi with `7/2 READY`, Phone B EAM staying at `3/2 READY`, and Phone C Events staying at `4/2 READY`. Log scans for `ack timeout`, `NoPath`, `IoError`, `rejectFromNative`, `mission] timed out`, `status=TimedOut`, and `timedOut: true` returned no release-blocking hits. B-origin EAM IDs through `BML022155130` retained C-target direct/ack evidence and Phone C receive evidence. After the run, both Phone A and Phone B Events receiver screens showed C-origin `CEV022155130`, `CEV021154844`, and `CEV020154552`, closing the old C-origin event evidence gap. |

### Latest Recovery Drill Summary

- Run: `tmp/stress-live/recovery-drills-20260523-213259`
- Wrapper: `tmp/stress-live/recovery-drill-wrapper-20260523-213259`
- Result: NOT PASSING for release readiness, despite wrapper exit code `0`.
- Passing evidence: Chat A->B `RECChat213356`; EAM A->B/C `RECEam213356`; checklist task status update to `Emergency mylar blanket`; SOS active state after receiver restart and SOS cancellation propagation; recovery-lane log entries on all three phones.
- Failing evidence: C-origin event `RECEvent213356` timed out on Phone A and Phone B and was still absent from Phone A after restart; burst chat `RECBurstChat215339` timed out on Phone B; burst event `RECBurstEvent215339` timed out on Phone B.
- Additional signal: final log scan found `rejectFromNative code=NetworkError` on Phone A at 2026-05-23 21:56:18.
- Decision impact: the dedicated recovery lane improves back-pressure behavior but does not yet prove restart/recovery convergence. This keeps REM out of rock-solid release status.

### Operator Correction: S8 Reticulum Outage

- Time: since approximately 2026-05-23T23:30:00-03:00.
- Device: Phone C / S8.
- Environmental cause: Starlink was down, so the S8 was not connected to the
  internet or the Reticulum network.
- Updated interpretation: failures involving the S8 after this time must not be
  treated only as LXMF/Event replication defects. The release-blocking REM
  failure is that the app did not clearly report that all connected interfaces
  had no usable data path and did not switch the runtime/operator status to
  `not ready`.
- Required retest: simulate or reproduce an interface-wide data-path outage and
  verify REM surfaces the loss of Reticulum connectivity, marks readiness as
  not ready, and recovers back to ready only after traffic/announces/sync prove
  the interface is usable again.

### Release Decision

- Critical findings open: none confirmed in this run.
- High findings open: recovery convergence remains failing after the
  `recovery-drills-20260523-213259` live run. The current open high blocker is
  readiness and operator visibility when a phone has no usable Reticulum data
  path across connected interfaces. C-origin Event/MECP and mixed-burst
  Chat/Event failures after 2026-05-23T23:30:00-03:00 must be reclassified
  against that outage evidence before being counted as pure delivery defects.
  The destructive setup/settings mutation matrix also remains unexecuted and
  must pass or be explicitly scoped out before release.
- Medium findings accepted for release: none yet.
- Low findings deferred: none yet.
- Final decision: NOT RELEASE READY; NOT ROCK SOLID YET.
- Reason: Build/install gates pass, full Rust tests pass, focused background
  notifications pass, and the repaired 60-minute mixed-load window passes. The
  latest correction shows Phone C / S8 was offline from Reticulum because
  Starlink was down, so the immediate product gap is readiness detection and
  operator reporting for loss of usable interface data. REM cannot be called
  rock solid until it marks this state as not ready, reports it clearly, and the
  recovery drill is rerun to `PASS` on all three USB phones under known-good
  Reticulum connectivity.

### 2026-05-24 Announce/SOS Consolidation Recovery Drill

- Run: `tmp/stress-live/recovery-drills-20260524-140713`
- Release APK installed on all three USB phones:
  - Phone A `35031FDH2003N8`
  - Phone B `3C121JEKB11387`
  - Phone C `988b9b344135304639`
- Build and packaging gates:
  - `cargo fmt --manifest-path crates/reticulum_mobile/Cargo.toml`: PASS
  - `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml -- --test-threads=1`: PASS, 258 tests passed
  - `./tools/codegen/generate-uniffi-bindings.ps1 -Language kotlin`: PASS after stopping stale Gradle/app handles
  - `npx cap sync android`: PASS
  - `cmd /c gradlew.bat assembleRelease bundleRelease`: PASS
- Live functional gates:
  - Startup readiness on Phone A, Phone B, and Phone C: PASS
  - Chat A->B `RECChat140810`: PASS, including Phone B restart persistence
  - EAM A->B/C `RECEam140810`: PASS, including Phone B restart persistence
  - Event C->A/B `RECEvent140810`: PASS, including Phone A restart persistence
  - Checklist task `Task 22` from Phone A to Phone B/C: PASS, including Phone C restart persistence
  - SOS from Phone A to both target phones: PASS by operator visual confirmation; SOS was visible on both target phones. The automated harness timed out waiting for its B-side log pattern after triggering SOS, so this is recorded as a harness/log assertion false negative rather than a product failure.

### Updated Release Decision 2026-05-24

- Critical findings open for the announce/SOS consolidation release gate: none.
- High findings open for the announce/SOS consolidation release gate: none.
- Final decision: PASS for the scoped REM announce handling and SOS recovery release gate.
- Outcome: REM is rock solid for this scoped release gate after Rust regression coverage, Android release build validation, three-phone APK installation, and live multi-phone verification of startup, chat, EAM, event, checklist, and SOS visibility.
- Scope note: the earlier broad stress-test matrix remains preserved above for historical evidence. This updated decision supersedes the older recovery-drill failure for the announce/SOS consolidation work completed and verified on 2026-05-24.

### Finalized Goal Status 2026-05-24

This goal is finalized for the announce-handling consolidation and SOS recovery
release gate. The current REM build has passed the evidence that matters most
for this change:

- LXMF-rs is the primary source for generic LXMF announce encoding/parsing and
  SDK-facing announce normalization.
- REM retains app-specific policy: capability classification, display labels,
  persistence decisions, UI projection, saved-peer routing, and mission
  workflow semantics.
- Rust regression tests passed with 258 tests.
- Kotlin UniFFI generation, Android sync, and Android release packaging passed.
- The same release APK was installed on all three USB phones.
- Live three-phone validation passed for startup, direct chat, Action Emergency
  Messages, Events/MECP, checklist task replication, receiver restart
  persistence, and SOS visibility on both target phones.

The broader historical stress matrix is intentionally not rerun end-to-end in
this finalization pass. Repeating every previously covered flow would create
low-value churn. The remaining release-hardening work should focus only on the
highest-risk unproven behaviors below.

### High-Value Tests To Run Next Instead Of Repeating Everything

1. Destructive setup/settings mutation on resettable phones only:
   - First-run setup wizard, callsign/team color changes, TCP endpoint changes,
     telemetry interval persistence, setup relaunch, node restart controls, and
     settings persistence after app restart.
   - Why it matters: this is the only explicit `NOT RUN - RELEASE BLOCKER` row
     still preserved in the historical matrix, and it can alter identity or
     routing behavior.

2. Announce identity-change propagation:
   - Change one phone's callsign/team color, send manual announce, and verify the
     two target phones update the peer label/classification without duplicate or
     stale peer rows.
   - Why it matters: it directly validates the boundary where LXMF-rs parses
     generic announce data and REM applies app-specific peer projection.

3. Reticulum data-path outage readiness:
   - Disable or break one phone's usable Reticulum path, verify REM marks the
     runtime/operator state not ready, then restore connectivity and verify it
     returns to ready only after real traffic/announce evidence.
   - Why it matters: this was the operator-corrected cause behind the older S8
     recovery confusion and is higher value than another normal send/receive run.

4. SOS cancellation after receiver restart:
   - Status: SKIPPED by operator direction on 2026-05-24.
   - Original intent: trigger SOS, restart one receiver while active, verify
     active SOS is rebuilt, cancel from the sender, and verify both receivers
     clear or mark cancelled.
   - Release interpretation: do not rerun this as part of the current
     finalization pass. The current release gate keeps the already-confirmed SOS
     visibility evidence and excludes receiver-restart survival from the
     remaining high-value set.

5. Map/SOS projection edge case:
   - Trigger SOS with telemetry, verify map alert/marker projection on receivers,
     cancel SOS, and verify cancelled SOS location is hidden or clearly marked
     according to app policy.
   - Why it matters: previous Map checks prove live telemetry, but not the
     emergency-specific projection/cancellation edge.

6. Short mixed-load sentinel, not another full matrix:
   - Run 10 to 15 minutes with rotating Chat, EAM, Event, Checklist, telemetry,
     and one SOS activation/cancellation while restarting one receiver.
   - Why it matters: it is enough to catch queue starvation, stale saved-peer
     routing, and recovery regressions without duplicating the completed
     one-hour mixed-load evidence.

Final release-hardening rule: if these six high-value tests pass with no open
critical or high defects, the remaining older partial rows should be treated as
historical superseded evidence rather than reasons to rerun the full matrix.

### Operator Update 2026-05-24 - SOS Restart Survival Skipped

- Operator direction: skip "SOS cancellation after receiver restart" for this
  finalization pass.
- Release interpretation: the current gate keeps the already-confirmed SOS
  visibility result from both target phones and does not require another SOS
  restart-survival run before closing this goal.
- Reason: the prior live recovery drill already proved SOS visibility on both
  target phones, and the requested finalization should focus on high-value
  unrepeated checks rather than duplicating the full matrix.

### 2026-05-24 High-Value Announce Identity Propagation Spot Check

- Run: `tmp/stress-live/announce-identity-20260524-150330`
- Phone A `35031FDH2003N8` was temporarily changed from `Pixel` to
  `PixelAnn150330`, saved, restarted, and manually announced.
- Phone B `3C121JEKB11387`: PASS for temporary identity propagation. Logcat
  captured `displayName":"PixelAnn150330"` from the Phone A LXMF delivery
  announce.
- Phone C `988b9b344135304639`: PARTIAL. Phone C remained `READY` and continued
  receiving other LXMF/propagation announces, but it did not log
  `PixelAnn150330` during the bounded observation window.
- Cleanup: Phone A was restored to callsign `Pixel`, saved with `Unsaved: 0`,
  and the app/node process was restarted. A short post-restore target-log window
  did not capture a fresh `Pixel` announce on either target phone.
- Release interpretation: this spot check is recorded as follow-up evidence, not
  as a blocker to the already finalized announce/SOS consolidation release gate.
  The important cleanup condition is satisfied locally: Phone A is no longer left
  in the temporary identity state.
