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
const { normalizeRnodeConnectionMode, normalizeRnodeSettings } = await import(moduleUrl);

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
