import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./settingsRestartReasons.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { changedRestartFields, restartBannerText } = await import(moduleUrl);

const base = {
  displayName: "Atlas-1",
  announceCapabilities: "R3AKT",
  announceIntervalSeconds: 1800,
  tcpClients: ["rmap.world:4242"],
  broadcast: true,
  transportNodeEnabled: true,
  telemetry: { staleAfterMinutes: 30 },
  hub: { mode: "Autonomous", identityHash: "", apiBaseUrl: "", apiKey: "", refreshIntervalSeconds: 3600 },
  rnode: { enabled: false, region: "US915" },
};

test("unchanged settings need no restart", () => {
  assert.deepEqual(changedRestartFields(base, structuredClone(base)), []);
});

test("names each changed node configuration field in plain language", () => {
  const after = {
    ...structuredClone(base),
    displayName: "Atlas-2",
    tcpClients: ["rmap.world:4242", "mesh.example:5151"],
    hub: { ...base.hub, mode: "Connected" },
  };
  assert.deepEqual(changedRestartFields(base, after), ["call sign", "TCP interfaces", "hub routing"]);
});

test("LoRa and stale-time changes are reported", () => {
  const after = {
    ...structuredClone(base),
    rnode: { enabled: true, region: "US915" },
    telemetry: { staleAfterMinutes: 45 },
  };
  assert.deepEqual(changedRestartFields(base, after), ["LoRa interface", "stale-after time"]);
});

test("banner text falls back when the reasons were lost on relaunch", () => {
  assert.equal(restartBannerText(["call sign", "TCP interfaces"]), "Restart node to apply: call sign, TCP interfaces");
  assert.equal(restartBannerText([]), "Restart node to apply saved changes");
});
