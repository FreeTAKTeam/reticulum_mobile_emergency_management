import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./rnodeProfiles.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  isRnodeFrequencyHz,
  inferRnodeRegionFromCoordinates,
  inferRnodeRegionFromTimezone,
  normalizeRnodeConnectionMode,
  normalizeRnodeFrequencyHz,
  normalizeRnodeProfile,
  normalizeRnodeRegion,
  normalizeRnodeSettings,
  resolveRnodeFrequencyForRegionChange,
} = await import(moduleUrl);

test("normalizes legacy RNode settings to BLE connection mode", () => {
  assert.deepEqual(
    normalizeRnodeSettings({
      enabled: true,
      peripheralId: " 48:CA:43:38:BC:E1 ",
      displayName: " Field RNode ",
      region: "EU868",
      profile: "REM-MF-URBAN-v1",
    }),
    {
      enabled: true,
      connectionMode: "ble",
      peripheralId: "48:CA:43:38:BC:E1",
      displayName: "Field RNode",
      region: "EU868",
      profile: "REM-MF-URBAN-v1",
      frequencyHz: 868_000_000,
    },
  );
});

test("normalizes RNode Bluetooth Classic aliases to explicit mode", () => {
  assert.equal(normalizeRnodeConnectionMode("classic"), "bluetooth_classic");
  assert.equal(normalizeRnodeConnectionMode("bluetooth"), "bluetooth_classic");
  assert.equal(normalizeRnodeConnectionMode("BluetoothClassic"), "bluetooth_classic");
});

test("normalizes RNode USB and TCP modes without changing selected device", () => {
  assert.equal(normalizeRnodeSettings({ enabled: true, connectionMode: "usb", peripheralId: "USB:123" }).connectionMode, "usb");
  assert.equal(normalizeRnodeSettings({ enabled: true, connectionMode: "tcp", peripheralId: "rnode.local" }).connectionMode, "tcp");
});

test("rejects an explicitly unknown RNode connection mode", () => {
  assert.throws(
    () => normalizeRnodeConnectionMode("carrier-pigeon"),
    /Unsupported RNode connection mode/,
  );
});

test("preserves fallback RNode connection mode when normalizing partial settings", () => {
  assert.deepEqual(
    normalizeRnodeSettings(
      {
        enabled: true,
        peripheralId: " 48:CA:43:38:BC:E1 ",
        displayName: " Field RNode ",
        region: "EU868",
        profile: "REM-MF-URBAN-v1",
      },
      {
        enabled: true,
        connectionMode: "bluetooth_classic",
        peripheralId: "stale",
        displayName: "stale",
        region: "US915",
        profile: "REM-LF-RURAL-v1",
      },
    ),
    {
      enabled: true,
      connectionMode: "bluetooth_classic",
      peripheralId: "48:CA:43:38:BC:E1",
      displayName: "Field RNode",
      region: "EU868",
      profile: "REM-MF-URBAN-v1",
      frequencyHz: 868_000_000,
    },
  );
});

test("updates regional defaults without overwriting explicit frequencies", () => {
  assert.equal(
    resolveRnodeFrequencyForRegionChange("US915", "EU868", 915_000_000),
    868_000_000,
  );
  assert.equal(
    resolveRnodeFrequencyForRegionChange("US915", "EU868", 433_000_000),
    433_000_000,
  );
});

test("rejects explicit unknown regions and profiles instead of silently retuning", () => {
  assert.throws(() => normalizeRnodeRegion("XX000"), /Unsupported RNode LoRa region/);
  assert.throws(() => normalizeRnodeProfile("REM-TYPO-v1"), /Unsupported RNode LoRa profile/);
  assert.equal(normalizeRnodeRegion(""), "US915");
  assert.equal(normalizeRnodeProfile(""), "REM-LF-RURAL-v1");
});

test("falls back to the selected region default for frequencies outside the RNode range", () => {
  assert.equal(isRnodeFrequencyHz(137_000_000), true);
  assert.equal(isRnodeFrequencyHz(3_000_000_000), true);
  assert.equal(isRnodeFrequencyHz(1), false);
  assert.equal(normalizeRnodeFrequencyHz(136_999_999, "EU868"), 868_000_000);
  assert.equal(normalizeRnodeFrequencyHz(3_000_000_001, "US915"), 915_000_000);
  assert.equal(normalizeRnodeFrequencyHz(433_000_000.4, "EU868"), 433_000_000);
});

test("infers only supported LoRa regions with a confident time zone match", () => {
  assert.equal(inferRnodeRegionFromTimezone("America/Halifax"), "US915");
  assert.equal(inferRnodeRegionFromTimezone("Europe/Paris"), "EU868");
  assert.equal(inferRnodeRegionFromTimezone("Australia/Sydney"), "AU915");
  assert.equal(inferRnodeRegionFromTimezone("Asia/Tokyo"), "AS923");
  assert.equal(inferRnodeRegionFromTimezone("Asia/Kolkata"), "IN865");
  assert.equal(inferRnodeRegionFromTimezone("Asia/Seoul"), "KR920");
  assert.equal(inferRnodeRegionFromTimezone("Europe/Moscow"), "RU864");
  assert.equal(inferRnodeRegionFromTimezone("America/Sao_Paulo"), undefined);
});

test("coordinate inference leaves ambiguous locations for manual selection", () => {
  assert.equal(inferRnodeRegionFromCoordinates(-33.87, 151.21), "AU915");
  assert.equal(inferRnodeRegionFromCoordinates(21.15, 79.09), "IN865");
  assert.equal(inferRnodeRegionFromCoordinates(37.56, 126.97), "KR920");
  assert.equal(inferRnodeRegionFromCoordinates(28.61, 77.21), undefined);
  assert.equal(inferRnodeRegionFromCoordinates(23.81, 90.41), undefined);
  assert.equal(inferRnodeRegionFromCoordinates(27.72, 85.32), undefined);
  assert.equal(inferRnodeRegionFromCoordinates(23.5, 74), undefined);
  assert.equal(inferRnodeRegionFromCoordinates(48.86, 2.35), undefined);
  assert.equal(inferRnodeRegionFromCoordinates(-23.55, -46.63), undefined);
  assert.equal(inferRnodeRegionFromCoordinates(Number.NaN, 0), undefined);
});
