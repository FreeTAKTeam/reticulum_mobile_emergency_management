import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./announceEvidence.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  announceHasEmergencyCapabilities,
  peerHasRemAnnounceEvidence,
} = await import(moduleUrl);

test("REM app announce capabilities are visible in discovery", () => {
  assert.equal(
    announceHasEmergencyCapabilities("R3AKT,EMergencyMessages,Telemetry;name=Pixel"),
    true,
  );
});

test("LXMF delivery enriches already REM-compatible peers", () => {
  assert.equal(
    peerHasRemAnnounceEvidence({
      appData: "R3AKT,EMergencyMessages,Telemetry;name=Poco",
      latestAnnounceClass: "LxmfDelivery",
      latestAnnounceKind: "lxmf_delivery",
    }),
    true,
  );
});

test("LXMF delivery alone is stored but not visible as a REM peer", () => {
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
