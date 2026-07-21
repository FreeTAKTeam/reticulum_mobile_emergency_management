import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const RED = "65ce79a3a3e4b51ec0ec52d1d3d2b0b9";
const nodeClientMock = `data:text/javascript;base64,${Buffer.from(`
  export const CANONICAL_TEAM_UIDS = new Set(["${RED}"]);
`).toString("base64")}`;
const source = await readFile(new URL("./localTeamExchange.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText.replaceAll('"@reticulum/node-client"', `"${nodeClientMock}"`);
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  MAX_LOCAL_TEAM_QR_MEMBERS,
  encodeLocalTeamExchange,
  encodeLocalTeamQrExchange,
  parseLocalTeamExchange,
} = await import(moduleUrl);

test("local team exchange round-trips canonical membership without a local alias", () => {
  const destination = "ab".repeat(16);
  const encoded = encodeLocalTeamExchange(RED, "RED", [
    { destination: destination.toUpperCase(), label: " Friend " },
  ]);
  assert.equal(encoded.includes("Friends"), false);
  assert.deepEqual(parseLocalTeamExchange(encoded), {
    teamUid: RED,
    members: [{ destination, label: "Friend" }],
  });
});

test("local team exchange rejects malformed and noncanonical input", () => {
  assert.throws(() => parseLocalTeamExchange("not-json"), /not valid JSON/);
  assert.throws(() => parseLocalTeamExchange(JSON.stringify({
    schemaVersion: 1,
    type: "rem.local-team",
    team: { uid: "custom", members: [] },
  })), /unsupported color/);
});

test("local team QR exchange is compact and excludes local names and labels", () => {
  const destination = "cd".repeat(16);
  const encoded = encodeLocalTeamQrExchange(RED, [destination]);
  assert.equal(encoded.includes("Friends"), false);
  assert.equal(encoded.includes("label"), false);
  assert.deepEqual(parseLocalTeamExchange(encoded), {
    teamUid: RED,
    members: [{ destination }],
  });
});

test("local team QR exchange rejects oversized rosters", () => {
  const members = Array.from(
    { length: MAX_LOCAL_TEAM_QR_MEMBERS + 1 },
    (_, index) => index.toString(16).padStart(32, "0"),
  );
  assert.throws(() => encodeLocalTeamQrExchange(RED, members), /at most 40/);
});
