#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
android="$root/apps/mobile/android"
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK}"
: "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to NDK 27.0.12077973}"
: "${ATN_SOURCE:?Set ATN_SOURCE to the ATN git repository}"
: "${ATN_COMMIT:?Set ATN_COMMIT to the full ATN source commit}"
: "${ATN_GRADLE:?Set ATN_GRADLE to Gradle 8.11.1}"
: "${ATN_KEYSTORE:?Set ATN_KEYSTORE to the ATN signing keystore}"
: "${ATN_KEY_ALIAS:?Set ATN_KEY_ALIAS}"
: "${ATN_STORE_PASSWORD:?Set ATN_STORE_PASSWORD in the environment}"
: "${ATN_KEY_PASSWORD:?Set ATN_KEY_PASSWORD in the environment}"
: "${REM_VERSION:?Set REM_VERSION}"
: "${REM_VERSION_CODE:?Set REM_VERSION_CODE}"
[[ "$ATN_COMMIT" =~ ^[a-f0-9]{40}$ ]] || { echo 'ATN_COMMIT must be a full commit SHA' >&2; exit 1; }
[[ "$REM_VERSION" =~ ^[0-9]+(\.[0-9]+)+([A-Za-z0-9.-]*)?$ && "$REM_VERSION_CODE" =~ ^[0-9]+$ ]] || { echo 'Invalid REM version' >&2; exit 1; }
[[ -z "$(git -C "$root" status --porcelain)" ]] || { echo 'REM checkout must be clean before packaging' >&2; exit 1; }
lxmf="$root/../LXMF-rs"
lxmf_pin="$(awk '/^  LXMF_RS_REF:/ {print $2; exit}' "$root/.github/workflows/plugins.yml")"
[[ "$lxmf_pin" =~ ^[a-f0-9]{40}$ && "$(git -C "$lxmf" rev-parse HEAD)" == "$lxmf_pin" && -z "$(git -C "$lxmf" status --porcelain)" ]] || { echo 'LXMF-rs must be clean and match the plugin workflow pin' >&2; exit 1; }
for file in keystore.properties ble-heart-rate-keystore.properties watch-status-keystore.properties; do
    [[ -f "$android/$file" ]] || { echo "Missing Android signing configuration: $file" >&2; exit 1; }
done
apksigner="$ANDROID_HOME/build-tools/35.0.0/apksigner"
aapt="$ANDROID_HOME/build-tools/35.0.0/aapt"
[[ -x "$apksigner" && -x "$aapt" ]] || { echo 'Android build tools 35.0.0 are required' >&2; exit 1; }
release="$root/output/offline-release-$REM_VERSION"
[[ ! -e "$release" ]] || { echo "Release output already exists: $release" >&2; exit 1; }
mkdir -p "$root/.release-work" "$root/output"
work="$(mktemp -d "$root/.release-work/offline.XXXXXXXX")"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/atn" "$work/assets/rem-plugins" "$work/release"
git -C "$ATN_SOURCE" archive "$ATN_COMMIT" | tar -x -C "$work/atn"

cd "$root"
npm run node-client:build
VITE_APP_VERSION="$REM_VERSION" npm run mobile:build
(cd apps/mobile && npx cap sync android)
bash tools/codegen/generate-uniffi-bindings.sh kotlin
common=("-PappVersionName=$REM_VERSION" "-PappVersionCode=$REM_VERSION_CODE" '-PabiFilters=arm64-v8a,armeabi-v7a,x86_64')
(cd "$android" && bash ./gradlew "${common[@]}" -PapkClassifierSuffix=light :app:assembleRelease :rem-plugin-sdk:assembleRelease)
light="$(find "$android/app/build/outputs/apk/release" -maxdepth 1 -name "*-v${REM_VERSION}-light-release.apk" -print -quit)"
[[ -f "$light" ]] || { echo 'Light APK missing' >&2; exit 1; }
"$apksigner" verify "$light"
badging="$("$aapt" dump badging "$light")"
[[ "$badging" == *"package: name='network.reticulum.emergency' versionCode='$REM_VERSION_CODE' versionName='$REM_VERSION'"* ]] || { echo 'Incorrect light APK identity or version' >&2; exit 1; }
host_fingerprint="$("$apksigner" verify --print-certs "$light" | awk '/Signer #1 certificate SHA-256 digest:/{print tolower($NF)}')"
[[ "$host_fingerprint" =~ ^[0-9a-f]{64}$ ]] || { echo 'Cannot determine REM signing certificate' >&2; exit 1; }
(cd "$android" && bash ./gradlew "-PremHostFingerprints=$host_fingerprint" :ble-heart-rate-plugin:assembleRelease :watch-status-plugin:assembleRelease)
cp "$root/plugins/ble-heart-rate/android/build/outputs/apk/release/ble-heart-rate-plugin-release.apk" "$work/release/ble-heart-rate.apk"
cp "$root/plugins/watch-status/android/build/outputs/apk/release/watch-status-plugin-release.apk" "$work/release/watch-status.apk"
sdk="$root/packages/android-plugin-sdk/build/outputs/aar/rem-plugin-sdk-release.aar"
export REM_SDK_AAR="$sdk" REM_SDK_SHA256="$(sha256sum "$sdk" | cut -d ' ' -f 1)"
export REM_SDK_COMMIT="$(git rev-parse HEAD)" REM_HOST_PACKAGE=network.reticulum.emergency REM_HOST_FINGERPRINTS="$host_fingerprint"
export ATN_ANDROID_ABIS=arm64-v8a,armeabi-v7a,x86_64
bash "$work/atn/scripts/build-android.sh" --release
"$ANDROID_HOME/build-tools/35.0.0/zipalign" -P 16 -f 4 "$work/atn/android/app/build/outputs/apk/release/app-release-unsigned.apk" "$work/atn-aligned.apk"
"$apksigner" sign --ks "$ATN_KEYSTORE" --ks-key-alias "$ATN_KEY_ALIAS" --ks-pass env:ATN_STORE_PASSWORD --key-pass env:ATN_KEY_PASSWORD --out "$work/release/atn-rangefinder.apk" "$work/atn-aligned.apk"
for plugin in ble-heart-rate watch-status atn-rangefinder; do
    apk="$work/release/$plugin.apk"
    "$apksigner" verify "$apk"
    case "$plugin" in
        ble-heart-rate) package=org.freetakteam.rem.plugin.bleheartrate ;;
        watch-status) package=org.freetakteam.rem.plugin.watchstatus ;;
        atn-rangefinder) package=org.freetak.atn ;;
    esac
    badging="$("$aapt" dump badging "$apk")"
    [[ "$badging" == *"package: name='$package'"* ]] || { echo "Incorrect package: $plugin" >&2; exit 1; }
    hash="$(sha256sum "$apk" | cut -d ' ' -f 1)"
    cp "$apk" "$work/assets/rem-plugins/$plugin-$hash.apk"
done
cp "$light" "$work/release/rem-light.apk"
(cd "$android" && bash ./gradlew "${common[@]}" -PapkClassifierSuffix=complete "-PbundledPluginsDir=$work/assets" :app:assembleRelease)
complete="$(find "$android/app/build/outputs/apk/release" -maxdepth 1 -name "*-v${REM_VERSION}-complete-release.apk" -print -quit)"
[[ -f "$complete" ]] || { echo 'Complete APK missing' >&2; exit 1; }
"$apksigner" verify "$complete"
badging="$("$aapt" dump badging "$complete")"
[[ "$badging" == *"package: name='network.reticulum.emergency' versionCode='$REM_VERSION_CODE' versionName='$REM_VERSION'"* ]] || { echo 'Incorrect complete APK identity or version' >&2; exit 1; }
complete_fingerprint="$("$apksigner" verify --print-certs "$complete" | awk '/Signer #1 certificate SHA-256 digest:/{print tolower($NF)}')"
[[ "$host_fingerprint" == "$complete_fingerprint" ]] || { echo 'REM signing certificates differ' >&2; exit 1; }
cp "$complete" "$work/release/rem-complete.apk"
python3 - "$work/release/rem-complete.apk" "$work/release/rem-light.apk" "$work/assets/rem-plugins" <<'PY'
import pathlib, sys, zipfile
complete, light, assets = sys.argv[1:]
expected = {"assets/rem-plugins/" + p.name: p.read_bytes() for p in pathlib.Path(assets).glob("*.apk")}
if len(expected) != 3:
    raise SystemExit("Expected exactly three plugin APKs")
for path, bundled in ((complete, True), (light, False)):
    with zipfile.ZipFile(path) as apk:
        names = {name for name in apk.namelist() if name.startswith("assets/rem-plugins/")}
        if names != (set(expected) if bundled else set()):
            raise SystemExit(f"Incorrect plugin assets in {path}")
        if bundled and any(apk.read(name) != data for name, data in expected.items()):
            raise SystemExit("Bundled plugin bytes differ from standalone APKs")
PY
(cd "$work/release" && sha256sum *.apk > SHA256SUMS)
printf 'REM commit: %s\nLXMF commit: %s\nATN commit: %s\nREM signer SHA-256: %s\n' "$(git rev-parse HEAD)" "$lxmf_pin" "$ATN_COMMIT" "$host_fingerprint" > "$work/release/BUILD-INFO.txt"
mv "$work/release" "$release"
printf 'Offline release ready at %s\n' "$release"
