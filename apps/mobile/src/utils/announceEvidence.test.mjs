import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import { pack } from "msgpackr";
import ts from "typescript";

const compilerOptions = {
  module: ts.ModuleKind.ES2022,
  target: ts.ScriptTarget.ES2022,
};
const cacheRoot = new URL("../../../../node_modules/.cache/", import.meta.url);
await mkdir(cacheRoot, { recursive: true });
const tempDir = await mkdtemp(new URL("announce-evidence-", cacheRoot));

const peersSource = await readFile(new URL("./peers.ts", import.meta.url), "utf8");
const peersTranspiled = ts.transpileModule(peersSource, {
  compilerOptions,
}).outputText;
const peersPath = `${tempDir}/peers.mjs`;
await writeFile(peersPath, peersTranspiled, "utf8");

const evidenceSource = await readFile(new URL("./announceEvidence.ts", import.meta.url), "utf8");
const evidenceTranspiled = ts.transpileModule(evidenceSource, {
  compilerOptions,
}).outputText.replace('from "./peers"', 'from "./peers.mjs"');
const evidencePath = `${tempDir}/announceEvidence.mjs`;
await writeFile(evidencePath, evidenceTranspiled, "utf8");

const evidenceModule = await import(pathToFileURL(evidencePath).href);
const peersModule = await import(pathToFileURL(peersPath).href);
await rm(tempDir, { recursive: true, force: true });

const {
  announceHasEmergencyCapabilities,
  peerHasRemAnnounceEvidence,
} = evidenceModule;
const { hasCapability } = peersModule;

function structuredAppDataHex(capabilities) {
  const metadata = pack({
    app: "rch",
    schema: 1,
    caps: capabilities,
  });
  return Buffer.from(pack([Buffer.from("Pixel"), null, metadata])).toString("hex");
}

function legacyStructuredAppDataHex(capabilities) {
  return Buffer.from(pack([
    "Legacy REM",
    { caps: capabilities },
  ])).toString("hex");
}

test("REM capability text is recognized", () => {
  assert.equal(
    announceHasEmergencyCapabilities("R3AKT,EMergencyMessages,Telemetry;name=Pixel"),
    true,
  );
});

test("standard LXMF metadata capabilities are recognized", () => {
  const appData = structuredAppDataHex([
    "R3AKT",
    "EMergencyMessages",
    "Telemetry",
    "rem.standard_lxmf_receipts.v1",
  ]);

  assert.equal(announceHasEmergencyCapabilities(appData), true);
  assert.equal(hasCapability(appData, "Telemetry"), true);
});

test("legacy structured REM metadata capabilities remain recognized", () => {
  const appData = legacyStructuredAppDataHex([
    "R3AKT",
    "EMergencyMessages",
  ]);

  assert.equal(announceHasEmergencyCapabilities(appData), true);
});

test("standard LXMF metadata without REM capabilities is not REM evidence", () => {
  const appData = Buffer.from(pack([Buffer.from("Sideband"), null])).toString("hex");

  assert.equal(announceHasEmergencyCapabilities(appData), false);
});

test("REM LXMF delivery capabilities are visible in discovery", () => {
  assert.equal(
    peerHasRemAnnounceEvidence({
      appData: "R3AKT,EMergencyMessages,Telemetry;name=Poco",
      latestAnnounceClass: "LxmfDelivery",
      latestAnnounceKind: "lxmf_delivery",
    }),
    true,
  );
});

test("app-only REM capabilities are not discovery evidence", () => {
  assert.equal(
    peerHasRemAnnounceEvidence({
      appData: "R3AKT,EMergencyMessages,Telemetry;name=Poco",
      latestAnnounceClass: "PeerApp",
      latestAnnounceKind: "app",
    }),
    false,
  );
});

test("non-REM LXMF delivery is not visible as a REM peer", () => {
  assert.equal(
    peerHasRemAnnounceEvidence({
      appData: "92c404506f636fc0",
      latestAnnounceClass: "LxmfDelivery",
      latestAnnounceKind: "lxmf_delivery",
    }),
    false,
  );
});

test("non-REM announce evidence is not visible in discovery", () => {
  assert.equal(
    peerHasRemAnnounceEvidence({
      appData: "Node propagation",
      latestAnnounceClass: "PropagationNode",
      latestAnnounceKind: "lxmf_propagation",
    }),
    false,
  );
});
