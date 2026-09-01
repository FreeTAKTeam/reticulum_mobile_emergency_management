#!/usr/bin/env bash
set -euo pipefail

PACKAGE="network.reticulum.emergency"
RECEIVER="$PACKAGE/.AdbTestControlReceiver"
ACTION_PREFIX="$PACKAGE.action"
RADIO_WAIT_SECONDS="${RADIO_WAIT_SECONDS:-45}"
STARTUP_ATTEMPTS="${STARTUP_ATTEMPTS:-18}"
RNODE_RESET_WAIT_SECONDS="${RNODE_RESET_WAIT_SECONDS:-8}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LXMF_ROOT="$(cd "$REPO_ROOT/../LXMF-rs" && pwd)"

usage() {
  echo "usage: $0 --phone-a SERIAL --phone-b SERIAL --app-destination-a HEX --app-destination-b HEX --lxmf-destination-a HEX --lxmf-destination-b HEX --apk FILE --signer-sha256 HEX [--output DIR]"
}

PHONE_A="" PHONE_B="" APP_DESTINATION_A="" APP_DESTINATION_B=""
LXMF_DESTINATION_A="" LXMF_DESTINATION_B="" APK="" EXPECTED_SIGNER_SHA256="" OUTPUT=""
while (($#)); do
  case "$1" in
    --phone-a) PHONE_A="$2"; shift 2 ;;
    --phone-b) PHONE_B="$2"; shift 2 ;;
    --app-destination-a) APP_DESTINATION_A="$2"; shift 2 ;;
    --app-destination-b) APP_DESTINATION_B="$2"; shift 2 ;;
    --lxmf-destination-a) LXMF_DESTINATION_A="$2"; shift 2 ;;
    --lxmf-destination-b) LXMF_DESTINATION_B="$2"; shift 2 ;;
    --apk) APK="$2"; shift 2 ;;
    --signer-sha256) EXPECTED_SIGNER_SHA256="$2"; shift 2 ;;
    --output) OUTPUT="$2"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

for tool in adb jq git base64 sha256sum find sort awk sed tr wc mktemp; do
  command -v "$tool" >/dev/null || { echo "required tool not found: $tool" >&2; exit 2; }
done
if [[ -z "$PHONE_A" || -z "$PHONE_B" || -z "$APP_DESTINATION_A" \
  || -z "$APP_DESTINATION_B" || -z "$LXMF_DESTINATION_A" || -z "$LXMF_DESTINATION_B" \
  || -z "$APK" || -z "$EXPECTED_SIGNER_SHA256" ]]; then
  usage >&2
  exit 2
fi
[[ "$PHONE_A" != "$PHONE_B" ]] || { echo "phones must be distinct" >&2; exit 2; }
for destination in "$APP_DESTINATION_A" "$APP_DESTINATION_B" \
  "$LXMF_DESTINATION_A" "$LXMF_DESTINATION_B"; do
  [[ "$destination" =~ ^[0-9a-fA-F]{32}$ ]] \
    || { echo "destinations must be 32 hexadecimal characters" >&2; exit 2; }
done
APP_DESTINATION_A="${APP_DESTINATION_A,,}"
APP_DESTINATION_B="${APP_DESTINATION_B,,}"
LXMF_DESTINATION_A="${LXMF_DESTINATION_A,,}"
LXMF_DESTINATION_B="${LXMF_DESTINATION_B,,}"
[[ "$APP_DESTINATION_A" != "$APP_DESTINATION_B" \
  && "$LXMF_DESTINATION_A" != "$LXMF_DESTINATION_B" ]] \
  || { echo "each destination pair must be distinct" >&2; exit 2; }
[[ -f "$APK" ]] || { echo "APK not found: $APK" >&2; exit 2; }
APK="$(cd "$(dirname "$APK")" && pwd)/$(basename "$APK")"
EXPECTED_SIGNER_SHA256="$(printf '%s' "$EXPECTED_SIGNER_SHA256" \
  | tr '[:upper:]' '[:lower:]' | tr -d ':[:space:]')"
[[ "$EXPECTED_SIGNER_SHA256" =~ ^[0-9a-f]{64}$ ]] \
  || { echo "signer SHA-256 must be 64 hexadecimal characters" >&2; exit 2; }
if [[ -n "$(git -C "$REPO_ROOT" status --porcelain --untracked-files=normal)" \
  || -n "$(git -C "$LXMF_ROOT" status --porcelain --untracked-files=normal)" ]]; then
  echo "HIL must run from committed, clean REM and LXMF worktrees" >&2
  exit 2
fi

find_android_build_tool() {
  local name="$1" sdk_root candidate
  if command -v "$name" >/dev/null; then
    command -v "$name"
    return
  fi
  sdk_root="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"
  [[ -n "$sdk_root" && -d "$sdk_root/build-tools" ]] \
    || { echo "ANDROID_SDK_ROOT or ANDROID_HOME must identify the Android SDK" >&2; return 1; }
  candidate="$(find "$sdk_root/build-tools" -mindepth 2 -maxdepth 2 -type f -name "$name" \
    | sort -V | tail -n 1)"
  [[ -n "$candidate" ]] || { echo "Android build tool not found: $name" >&2; return 1; }
  printf '%s\n' "$candidate"
}

APKSIGNER="$(find_android_build_tool apksigner)"
AAPT="$(find_android_build_tool aapt)"
REM_SHA="$(git -C "$REPO_ROOT" rev-parse HEAD)"
LXMF_SHA="$(git -C "$LXMF_ROOT" rev-parse HEAD)"
REM_TREE="$(git -C "$REPO_ROOT" rev-parse HEAD^{tree})"
LXMF_TREE="$(git -C "$LXMF_ROOT" rev-parse HEAD^{tree})"
APK_SHA256="$(sha256sum "$APK" | awk '{print $1}')"
APK_BADGING="$("$AAPT" dump badging "$APK" | sed -n '1p')"
APK_PACKAGE="$(sed -n "s/^package: name='\([^']*\)'.*/\1/p" <<<"$APK_BADGING")"
APK_VERSION_CODE="$(sed -n "s/.*versionCode='\([^']*\)'.*/\1/p" <<<"$APK_BADGING")"
APK_VERSION_NAME="$(sed -n "s/.*versionName='\([^']*\)'.*/\1/p" <<<"$APK_BADGING")"
APK_SIGNER_SHA256="$("$APKSIGNER" verify --print-certs "$APK" \
  | sed -n 's/^Signer #1 certificate SHA-256 digest: //p' \
  | sed -n '1p' | tr '[:upper:]' '[:lower:]' | tr -d ':[:space:]')"
[[ "$APK_PACKAGE" == "$PACKAGE" ]] \
  || { echo "APK package mismatch: expected $PACKAGE, got $APK_PACKAGE" >&2; exit 2; }
[[ -n "$APK_VERSION_CODE" && -n "$APK_VERSION_NAME" ]] \
  || { echo "could not read APK version metadata" >&2; exit 2; }
[[ "$APK_VERSION_NAME" == *"${REM_SHA:0:7}"* \
  && "$APK_VERSION_NAME" == *"${LXMF_SHA:0:7}"* ]] \
  || { echo "APK version does not identify current REM and LXMF revisions: $APK_VERSION_NAME" >&2; exit 2; }
[[ "$APK_SIGNER_SHA256" == "$EXPECTED_SIGNER_SHA256" ]] \
  || { echo "APK signer mismatch: expected $EXPECTED_SIGNER_SHA256, got $APK_SIGNER_SHA256" >&2; exit 2; }

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RFC3339_TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
RUN_START_MS="$(( $(date +%s) * 1000 ))"
OUTPUT="${OUTPUT:-$REPO_ROOT/target/lora-regression/$TIMESTAMP}"
mkdir -p "$OUTPUT/$PHONE_A" "$OUTPUT/$PHONE_B"
RESULT_FILE="$OUTPUT/result.txt"
echo "FAIL: runner did not reach all acceptance assertions" >"$RESULT_FILE"
CAPTURE_RADIO_FAILURE=0

adb_for() { local serial="$1"; shift; adb -s "$serial" "$@"; }
b64() { base64 | tr -d '\r\n'; }
broadcast() {
  local serial="$1" action="$2"; shift 2
  adb_for "$serial" shell am broadcast -n "$RECEIVER" -a "$ACTION_PREFIX.$action" "$@" >/dev/null
}
logcat_raw() { adb_for "$1" logcat -d -v raw -s ReticulumAdbTest:I '*:S'; }
service_logcat_raw() { adb_for "$1" logcat -d -v raw -s ReticulumNodeService:I '*:S'; }

verify_installed_apk() {
  local serial="$1" package_output package_path pull_dir pulled_apk installed_sha256
  local version_output installed_version_code installed_version_name
  package_output="$(adb_for "$serial" shell pm path "$PACKAGE" | tr -d '\r')"
  printf '%s\n' "$package_output" >"$OUTPUT/$serial/package-path.txt"
  package_path="$(sed -n 's/^package://p' <<<"$package_output")"
  [[ -n "$package_path" && "$(wc -l <<<"$package_path")" -eq 1 ]] \
    || { echo "expected one installed base APK on $serial" >&2; return 1; }

  pull_dir="$(mktemp -d)"
  pulled_apk="$pull_dir/installed.apk"
  if ! adb_for "$serial" pull "$package_path" "$pulled_apk" >/dev/null; then
    rm -f "$pulled_apk"
    rmdir "$pull_dir"
    echo "failed to pull installed APK from $serial" >&2
    return 1
  fi
  installed_sha256="$(sha256sum "$pulled_apk" | awk '{print $1}')"
  rm -f "$pulled_apk"
  rmdir "$pull_dir"
  [[ "$installed_sha256" == "$APK_SHA256" ]] \
    || { echo "installed APK digest mismatch on $serial" >&2; return 1; }

  version_output="$(adb_for "$serial" shell dumpsys package "$PACKAGE" \
    | grep -E 'versionCode=|versionName=' | tr -d '\r')"
  printf '%s\n' "$version_output" >"$OUTPUT/$serial/package-version.txt"
  installed_version_code="$(sed -n 's/.*versionCode=\([^ ]*\).*/\1/p' <<<"$version_output" | head -n 1)"
  installed_version_name="$(sed -n 's/.*versionName=\([^[:space:]]*\).*/\1/p' <<<"$version_output" | head -n 1)"
  [[ "$installed_version_code" == "$APK_VERSION_CODE" \
    && "$installed_version_name" == "$APK_VERSION_NAME" ]] \
    || { echo "installed APK version mismatch on $serial" >&2; return 1; }

  cat >"$OUTPUT/$serial/apk-verification.txt" <<EOF
apk_sha256=$installed_sha256
signer_sha256=$APK_SIGNER_SHA256
version_code=$installed_version_code
version_name=$installed_version_name
rem_sha=$REM_SHA
lxmf_sha=$LXMF_SHA
EOF
}

latest_json() {
  local serial="$1" action="$2" prefix="$3" line
  shift 3
  broadcast "$serial" "$action" "$@"
  sleep 3
  line="$(logcat_raw "$serial" | grep "^$prefix " | tail -n 1 || true)"
  [[ -n "$line" ]] || { echo "missing $prefix result on $serial" >&2; return 1; }
  printf '%s\n' "${line#"$prefix "}"
}

run_result_action() {
  local serial="$1" action="$2" label="$3" before after latest attempt
  shift 3
  before="$(logcat_raw "$serial" | grep -c "^$label .*outcome=" || true)"
  broadcast "$serial" "$action" "$@"
  for ((attempt = 1; attempt <= 90; attempt++)); do
    sleep 1
    after="$(logcat_raw "$serial" | grep -c "^$label .*outcome=" || true)"
    if ((after > before)); then
      latest="$(logcat_raw "$serial" | grep "^$label .*outcome=" | tail -n 1)"
      [[ "$latest" == *"outcome=ok"* ]] || { echo "$latest" >&2; return 1; }
      return 0
    fi
  done
  echo "timed out waiting for $label result on $serial" >&2
  return 1
}

reset_rnode() {
  local serial="$1" before after latest attempt
  before="$(logcat_raw "$serial" | grep -c '^rnodeReset outcome=' || true)"
  broadcast "$serial" ADB_RESET_RNODE
  for ((attempt = 1; attempt <= 15; attempt++)); do
    sleep 1
    after="$(logcat_raw "$serial" | grep -c '^rnodeReset outcome=' || true)"
    if ((after > before)); then
      latest="$(logcat_raw "$serial" | grep '^rnodeReset outcome=' | tail -n 1)"
      [[ "$latest" == *"outcome=ok"* ]] || { echo "$latest" >&2; return 1; }
      return 0
    fi
  done
  echo "timed out resetting RNode on $serial" >&2
  return 1
}

snapshot() {
  local serial="$1" label="$2"
  latest_json "$serial" ADB_STATUS status >/dev/null
  adb_for "$serial" logcat -d -v threadtime \
    ReticulumAdbTest:I RNodeAndroidTransport:I ReticulumNodeService:I ReticulumNode:I '*:S' \
    >"$OUTPUT/$serial/$label-logcat.txt"
}

capture_failure() {
  local exit_code=$?
  if ((exit_code != 0)); then
    set +e
    if ((CAPTURE_RADIO_FAILURE)); then
      broadcast "$PHONE_A" ADB_RNODE_STATS
      broadcast "$PHONE_B" ADB_RNODE_STATS
      sleep 2
      snapshot "$PHONE_A" failure
      snapshot "$PHONE_B" failure
    fi
    echo "FAIL: acceptance stopped with exit code $exit_code" >"$RESULT_FILE"
  fi
  return "$exit_code"
}
trap capture_failure EXIT

wait_for_ready() {
  local serial="$1" expected_app="$2" expected_lxmf="$3" attempt status transport
  for ((attempt = 1; attempt <= STARTUP_ATTEMPTS; attempt++)); do
    status="$(latest_json "$serial" ADB_STATUS status)"
    transport="$(logcat_raw "$serial" | grep '^rnodeTransport ' | tail -n 1)"
    transport="${transport#rnodeTransport }"
    if jq -e --arg app "$expected_app" --arg lxmf "$expected_lxmf" '
      .running == true and .readiness.state == "Ready"
      and .appDestinationHex == $app and .lxmfDestinationHex == $lxmf
      and any(.interfaces[]; .kind == "rnode_ble" and .state == "connected")
      and any(.readiness.interfaces[]; .id == "rnode" and .state == "Ready")
      and any(.readiness.interfaces[]; .id == "tcp" and .state == "Disabled")
    ' <<<"$status" >/dev/null \
      && jq -e '.installed == true and .session.closed == false and .session.kind == "ble"
        and .session.negotiatedMtu >= 23 and .session.lastError == null' \
        <<<"$transport" >/dev/null; then
      return 0
    fi
    sleep 2
  done
  snapshot "$serial" startup-failed || true
  echo "RNode runtime failed strict readiness on $serial" >&2
  return 1
}

assert_matching_radio_profiles() {
  local settings_a settings_b evidence_a evidence_b
  settings_a="$(latest_json "$PHONE_A" ADB_APP_SETTINGS appSettings)"
  settings_b="$(latest_json "$PHONE_B" ADB_APP_SETTINGS appSettings)"
  jq -e '.tcpClients == [] and .rnode.enabled == true and .rnode.connectionMode == "ble"' \
    <<<"$settings_a" >/dev/null
  jq -e '.tcpClients == [] and .rnode.enabled == true and .rnode.connectionMode == "ble"' \
    <<<"$settings_b" >/dev/null
  [[ "$(jq -c '.rnode | {profile,region,frequencyHz}' <<<"$settings_a")" \
    == "$(jq -c '.rnode | {profile,region,frequencyHz}' <<<"$settings_b")" ]]
  [[ "$(jq -r '.rnode.peripheralId' <<<"$settings_a")" \
    != "$(jq -r '.rnode.peripheralId' <<<"$settings_b")" ]]
  jq -e --arg peer "$LXMF_DESTINATION_B" '
    .teams.activeTeamUid as $active
    | any(.teams.localTeams[]; .teamUid == $active and (.memberDestinations | index($peer) != null))
  ' <<<"$settings_a" >/dev/null
  jq -e --arg peer "$LXMF_DESTINATION_A" '
    .teams.activeTeamUid as $active
    | any(.teams.localTeams[]; .teamUid == $active and (.memberDestinations | index($peer) != null))
  ' <<<"$settings_b" >/dev/null

  evidence_a="$(service_logcat_raw "$PHONE_A" \
    | grep 'rnode_ble: startup evidence .* evidence=' | tail -n 1 || true)"
  evidence_b="$(service_logcat_raw "$PHONE_B" \
    | grep 'rnode_ble: startup evidence .* evidence=' | tail -n 1 || true)"
  [[ -n "$evidence_a" && -n "$evidence_b" ]] \
    || { echo "missing live RNode startup evidence" >&2; return 1; }
  evidence_a="${evidence_a#* evidence=}"
  evidence_b="${evidence_b#* evidence=}"
  for evidence in "$evidence_a" "$evidence_b"; do
    jq -e '
      .startup_validated == true
      and .probe.detected == true
      and (.probe.firmware_version | type == "string" and length > 0)
      and .configured.frequency_hz == 914625000
      and .configured.bandwidth_hz == 250000
      and .configured.spreading_factor == 11
      and .configured.coding_rate == 5
      and .configured.tx_power_dbm == 17
      and ((.configured.frequency_hz - .reported.frequency_hz) as $frequency_delta
        | ($frequency_delta >= -100 and $frequency_delta <= 100))
      and .configured.bandwidth_hz == .reported.bandwidth_hz
      and .configured.spreading_factor == .reported.spreading_factor
      and .configured.coding_rate == .reported.coding_rate
      and .configured.tx_power_dbm == .reported.tx_power_dbm
      and (.reported.radio_state == 1 or .startup_compatibility_warning != null)
    ' <<<"$evidence" >/dev/null
  done
  [[ "$(jq -c '.configured | {frequency_hz,bandwidth_hz,spreading_factor,coding_rate,tx_power_dbm}' \
    <<<"$evidence_a")" == \
    "$(jq -c '.configured | {frequency_hz,bandwidth_hz,spreading_factor,coding_rate,tx_power_dbm}' \
    <<<"$evidence_b")" ]]
  printf '%s\n' "$evidence_a" | jq . >"$OUTPUT/$PHONE_A/rnode-startup-evidence.json"
  printf '%s\n' "$evidence_b" | jq . >"$OUTPUT/$PHONE_B/rnode-startup-evidence.json"
}

wait_for_assertion() {
  local serial="$1" action="$2" label="$3" payload="$4" attempt latest
  for ((attempt = 1; attempt <= 12; attempt++)); do
    broadcast "$serial" "$action" --es payloadBase64 "$(printf '%s' "$payload" | b64)"
    sleep 2
    latest="$(logcat_raw "$serial" | grep "^$label outcome=" | tail -n 1 || true)"
    if [[ "$latest" == *"outcome=ok"* ]]; then
      return 0
    fi
    sleep 8
  done
  echo "acceptance assertion failed on $serial: $label" >&2
  return 1
}

send_lxmf() {
  local serial="$1" destination="$2" body="$3" payload
  payload="$(jq -cn --arg destination "$destination" --arg body "$body" \
    '{destinationHex:$destination,bodyUtf8:$body,sendMode:"Direct",usePropagationNode:false}')"
  run_result_action "$serial" ADB_SEND_LXMF sendLxmf \
    --es payloadBase64 "$(printf '%s' "$payload" | b64)"
}

event_payload() {
  local direction="$1" uid
  uid="lr-$direction-$TIMESTAMP"
  jq -cn --arg uid "$uid" --arg timestamp "$RFC3339_TIMESTAMP" \
    '{uid:$uid,commandId:$uid,sourceIdentity:"hil",sourceDisplayName:null,timestamp:$timestamp,commandType:"event-upsert",missionUid:"lora-regression",content:"HIL",callsign:"HIL",serverTime:null,clientTime:$timestamp,keywords:[],contentHashes:[],updatedAt:(now*1000|floor),deletedAt:null,correlationId:null,topics:[]}'
}

cat >"$OUTPUT/metadata.txt" <<EOF
timestamp=$TIMESTAMP
phone_a=$PHONE_A
phone_b=$PHONE_B
app_destination_a=$APP_DESTINATION_A
app_destination_b=$APP_DESTINATION_B
lxmf_destination_a=$LXMF_DESTINATION_A
lxmf_destination_b=$LXMF_DESTINATION_B
rem_sha=$REM_SHA
lxmf_sha=$LXMF_SHA
rem_tree=$REM_TREE
lxmf_tree=$LXMF_TREE
apk_sha256=$APK_SHA256
apk_signer_sha256=$APK_SIGNER_SHA256
apk_version_code=$APK_VERSION_CODE
apk_version_name=$APK_VERSION_NAME
EOF

for serial in "$PHONE_A" "$PHONE_B"; do
  adb_for "$serial" get-state | grep -qx device
  verify_installed_apk "$serial"
done
CAPTURE_RADIO_FAILURE=1
for serial in "$PHONE_A" "$PHONE_B"; do
  adb_for "$serial" logcat -c
  adb_for "$serial" shell am force-stop "$PACKAGE"
  adb_for "$serial" shell monkey -p "$PACKAGE" -c android.intent.category.LAUNCHER 1 >/dev/null
done

wait_for_ready "$PHONE_A" "$APP_DESTINATION_A" "$LXMF_DESTINATION_A"
wait_for_ready "$PHONE_B" "$APP_DESTINATION_B" "$LXMF_DESTINATION_B"

# A firmware transmit queue survives BLE disconnects. Reset both RNodes before
# collecting acceptance evidence so a previous failed run cannot inject stale
# link proofs or consume this run's half-duplex airtime budget.
reset_rnode "$PHONE_A"
reset_rnode "$PHONE_B"
sleep "$RNODE_RESET_WAIT_SECONDS"
for serial in "$PHONE_A" "$PHONE_B"; do
  adb_for "$serial" shell am force-stop "$PACKAGE"
  adb_for "$serial" logcat -c
  adb_for "$serial" shell monkey -p "$PACKAGE" -c android.intent.category.LAUNCHER 1 >/dev/null
done
RUN_START_MS="$(( $(date +%s) * 1000 ))"
wait_for_ready "$PHONE_A" "$APP_DESTINATION_A" "$LXMF_DESTINATION_A"
wait_for_ready "$PHONE_B" "$APP_DESTINATION_B" "$LXMF_DESTINATION_B"
assert_matching_radio_profiles
snapshot "$PHONE_A" startup
snapshot "$PHONE_B" startup

# Half-duplex acceptance is deliberately serialized: announce, connect, payload class, direction.
run_result_action "$PHONE_A" ADB_ANNOUNCE announce
sleep "$RADIO_WAIT_SECONDS"
wait_for_assertion "$PHONE_B" ADB_ASSERT_ANNOUNCE "assertAnnounce id=announce-a-to-b" \
  "$(jq -cn --arg id announce-a-to-b --arg destination "$LXMF_DESTINATION_A" \
    --argjson receivedAfterMs "$RUN_START_MS" \
    '{assertionId:$id,destinationHex:$destination,receivedAfterMs:$receivedAfterMs}')"
run_result_action "$PHONE_B" ADB_ANNOUNCE announce
sleep "$RADIO_WAIT_SECONDS"
wait_for_assertion "$PHONE_A" ADB_ASSERT_ANNOUNCE "assertAnnounce id=announce-b-to-a" \
  "$(jq -cn --arg id announce-b-to-a --arg destination "$LXMF_DESTINATION_B" \
    --argjson receivedAfterMs "$RUN_START_MS" \
    '{assertionId:$id,destinationHex:$destination,receivedAfterMs:$receivedAfterMs}')"

run_result_action "$PHONE_A" ADB_CONNECT_PEER "connect destination=$APP_DESTINATION_B" \
  --es destinationHex "$APP_DESTINATION_B"
sleep "$RADIO_WAIT_SECONDS"
run_result_action "$PHONE_B" ADB_CONNECT_PEER "connect destination=$APP_DESTINATION_A" \
  --es destinationHex "$APP_DESTINATION_A"

TOKEN_A="LORA_SMALL_A_TO_B_$TIMESTAMP"
TOKEN_B="LORA_SMALL_B_TO_A_$TIMESTAMP"
RESOURCE_TOKEN_A="LORA_RESOURCE_A_TO_B_$TIMESTAMP"
RESOURCE_TOKEN_B="LORA_RESOURCE_B_TO_A_$TIMESTAMP"
RESOURCE_A="$RESOURCE_TOKEN_A-$(printf 'A%.0s' {1..1024})"
RESOURCE_B="$RESOURCE_TOKEN_B-$(printf 'B%.0s' {1..1024})"

send_lxmf "$PHONE_A" "$LXMF_DESTINATION_B" "$TOKEN_A"
sleep "$RADIO_WAIT_SECONDS"
wait_for_assertion "$PHONE_B" ADB_ASSERT_MESSAGE "assertMessage id=small-a-to-b" \
  "$(jq -cn --arg id small-a-to-b --arg body "$TOKEN_A" \
    '{assertionId:$id,expectedBody:$body,prefix:false}')"
send_lxmf "$PHONE_B" "$LXMF_DESTINATION_A" "$TOKEN_B"
sleep "$RADIO_WAIT_SECONDS"
wait_for_assertion "$PHONE_A" ADB_ASSERT_MESSAGE "assertMessage id=small-b-to-a" \
  "$(jq -cn --arg id small-b-to-a --arg body "$TOKEN_B" \
    '{assertionId:$id,expectedBody:$body,prefix:false}')"

send_lxmf "$PHONE_A" "$LXMF_DESTINATION_B" "$RESOURCE_A"
sleep "$RADIO_WAIT_SECONDS"
wait_for_assertion "$PHONE_B" ADB_ASSERT_MESSAGE "assertMessage id=resource-a-to-b" \
  "$(jq -cn --arg id resource-a-to-b --arg body "$RESOURCE_TOKEN_A" \
    '{assertionId:$id,expectedBody:$body,prefix:true}')"
send_lxmf "$PHONE_B" "$LXMF_DESTINATION_A" "$RESOURCE_B"
sleep "$RADIO_WAIT_SECONDS"
wait_for_assertion "$PHONE_A" ADB_ASSERT_MESSAGE "assertMessage id=resource-b-to-a" \
  "$(jq -cn --arg id resource-b-to-a --arg body "$RESOURCE_TOKEN_B" \
    '{assertionId:$id,expectedBody:$body,prefix:true}')"

EVENT_A="lr-A_TO_B-$TIMESTAMP"
EVENT_B="lr-B_TO_A-$TIMESTAMP"
run_result_action "$PHONE_A" ADB_UPSERT_EVENT_TO_DESTINATION upsertEventToDestination \
  --es destinationHex "$APP_DESTINATION_B" \
  --es payloadBase64 "$(event_payload A_TO_B | b64)"
sleep "$RADIO_WAIT_SECONDS"
wait_for_assertion "$PHONE_B" ADB_ASSERT_EVENT "assertEvent id=event-a-to-b" \
  "$(jq -cn --arg id event-a-to-b --arg uid "$EVENT_A" \
    '{assertionId:$id,eventUid:$uid}')"
run_result_action "$PHONE_B" ADB_UPSERT_EVENT_TO_DESTINATION upsertEventToDestination \
  --es destinationHex "$APP_DESTINATION_A" \
  --es payloadBase64 "$(event_payload B_TO_A | b64)"
sleep "$RADIO_WAIT_SECONDS"
wait_for_assertion "$PHONE_A" ADB_ASSERT_EVENT "assertEvent id=event-b-to-a" \
  "$(jq -cn --arg id event-b-to-a --arg uid "$EVENT_B" \
    '{assertionId:$id,eventUid:$uid}')"

broadcast "$PHONE_A" ADB_RNODE_STATS
broadcast "$PHONE_B" ADB_RNODE_STATS
sleep 2
snapshot "$PHONE_A" final
snapshot "$PHONE_B" final
cat >>"$OUTPUT/metadata.txt" <<EOF
small_a_to_b=$TOKEN_A
small_b_to_a=$TOKEN_B
resource_a_to_b=$RESOURCE_TOKEN_A
resource_b_to_a=$RESOURCE_TOKEN_B
event_a_to_b=$EVENT_A
event_b_to_a=$EVENT_B
EOF
echo "PASS: bidirectional announce, connect, small LXMF, Resource-sized LXMF, and event replication" >"$RESULT_FILE"
echo "HIL PASS; evidence written to $OUTPUT"
