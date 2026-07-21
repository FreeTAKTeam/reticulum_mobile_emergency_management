import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const YELLOW = "d6b6e188b910d6bdd24d04b7a7ec5444";
const BLUE = "43341e5c822d99857fa6e8641f2ca9c0";
const nodeClientMock = `data:text/javascript;base64,${Buffer.from(`
  export const YELLOW_TEAM_UID = "${YELLOW}";
  export const CANONICAL_TEAM_UIDS = new Set(["${YELLOW}", "${BLUE}"]);
`).toString("base64")}`;
const source = await readFile(new URL("./teamSettings.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText.replaceAll('"@reticulum/node-client"', `"${nodeClientMock}"`);
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { normalizeTeamPreferences } = await import(moduleUrl);

test("team preferences retain canonical selection and bounded local aliases", () => {
  assert.deepEqual(normalizeTeamPreferences({
    activeTeamUid: BLUE.toUpperCase(),
    aliases: [
      { teamUid: BLUE, alias: `  ${"M".repeat(60)}  ` },
      { teamUid: BLUE, alias: "duplicate" },
      { teamUid: "custom", alias: "ignored" },
    ],
  }), {
    activeTeamUid: BLUE,
    aliases: [{ teamUid: BLUE, alias: "M".repeat(48) }],
    localTeams: [],
    localTeamsInitialized: false,
  });
});

test("missing or unsupported team preferences migrate to Yellow", () => {
  assert.deepEqual(normalizeTeamPreferences({ activeTeamUid: "custom", aliases: [] }), {
    activeTeamUid: YELLOW,
    aliases: [],
    localTeams: [],
    localTeamsInitialized: false,
  });
});

test("local teams retain canonical multi-membership and ensure Yellow exists", () => {
  const peer = "aa".repeat(16);
  assert.deepEqual(normalizeTeamPreferences({
    activeTeamUid: BLUE,
    aliases: [],
    localTeamsInitialized: true,
    localTeams: [
      { teamUid: BLUE, memberDestinations: [peer, peer, "invalid"] },
    ],
  }), {
    activeTeamUid: BLUE,
    aliases: [],
    localTeams: [
      { teamUid: YELLOW, memberDestinations: [] },
      { teamUid: BLUE, memberDestinations: [peer] },
    ],
    localTeamsInitialized: true,
  });
});
