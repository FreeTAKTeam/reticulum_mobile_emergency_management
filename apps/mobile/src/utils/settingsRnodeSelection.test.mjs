import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

// Load useSettingsRnode with its imports replaced by stubs; the Bluetooth stub is
// driven through globalThis.__rnodeStub so each test controls the pairing result.
function dataModule(source) {
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`;
}

const STUBS = {
  vue: "export const ref = (value) => ({ value });",
  "../../services/rnodeBluetooth": `
    export const normalizeRnodeBluetoothMode = (mode) => (mode === "bluetooth_classic" ? mode : "ble");
    export const rnodeBluetoothModeLabel = (mode) => mode;
    export const rnodeBluetoothDeviceDetail = () => "";
    export const rnodeUsbDeviceDetail = () => "";
    export const listPairedRnodeBluetoothDevices = async () => [];
    export const scanRnodeBluetoothDevices = async () => [];
    export const listRnodeUsbDevices = async () => [];
    export const requestRnodeUsbPermission = async (deviceId) => ({ deviceId, granted: false });
    export const startRnodeUsbBluetoothPairing = async () => ({});
    export const pairRnodeBluetoothDevice = (id, mode) => globalThis.__rnodeStub.pair(id, mode);
  `,
  "../../services/setupPermissions": 'export const requestRnodeBluetoothPermission = async () => "granted";',
  "../../utils/rnodeProfiles": "export const RNODE_PROFILE_SPECS = []; export const rnodeProfileSummary = () => \"\";",
  "../../utils/rnodeUsbPairing": "export const selectUsbBondedRnodeCandidate = () => undefined;",
};

const source = await readFile(new URL("../components/settings/useSettingsRnode.ts", import.meta.url), "utf8");
let transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
for (const [specifier, stub] of Object.entries(STUBS)) {
  transpiled = transpiled.replaceAll(`"${specifier}"`, `"${dataModule(stub)}"`);
}
const { useSettingsRnode } = await import(dataModule(transpiled));

function configuredForm() {
  return {
    rnodeEnabled: true,
    rnodeConnectionMode: "ble",
    rnodePeripheralId: "OLD-RNODE",
    rnodeDisplayName: "Old RNode",
    rnodeRegion: "EU868",
    rnodeProfile: "REM-MF-URBAN-v1",
  };
}

const newDevice = { id: "NEW-RNODE", address: "NEW-RNODE", name: "New RNode", paired: false, supportedModes: ["ble"] };

test("a failed pairing reports failure and keeps the configured RNode", async () => {
  globalThis.__rnodeStub = { pair: async () => { throw new Error("Bonding failed"); } };
  const form = configuredForm();
  const rnode = useSettingsRnode(form);

  assert.equal(await rnode.selectRnodeDevice(newDevice), false);
  assert.equal(form.rnodePeripheralId, "OLD-RNODE");
  assert.equal(rnode.scanFeedback.value, "Bonding failed");
});

test("pairing that only started reports failure", async () => {
  globalThis.__rnodeStub = {
    pair: async (id) => ({ id, address: id, paired: false, bondingStarted: true, bondState: "bonding" }),
  };
  const rnode = useSettingsRnode(configuredForm());

  assert.equal(await rnode.selectRnodeDevice(newDevice), false);
});

test("a successful pairing selects the requested device", async () => {
  globalThis.__rnodeStub = {
    pair: async (id) => ({ id, address: id, paired: true, bondingStarted: false, bondState: "bonded" }),
  };
  const form = configuredForm();
  const rnode = useSettingsRnode(form);

  assert.equal(await rnode.selectRnodeDevice(newDevice), true);
  assert.equal(form.rnodePeripheralId, "NEW-RNODE");
  assert.equal(form.rnodeDisplayName, "New RNode");
});

test("an already paired device is selected without pairing again", async () => {
  globalThis.__rnodeStub = { pair: async () => { throw new Error("should not pair"); } };
  const form = configuredForm();
  const rnode = useSettingsRnode(form);

  assert.equal(await rnode.selectRnodeDevice({ ...newDevice, paired: true }), true);
  assert.equal(form.rnodePeripheralId, "NEW-RNODE");
});
