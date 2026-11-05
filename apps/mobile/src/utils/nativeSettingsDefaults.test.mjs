import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./nativeSettingsDefaults.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { withNativeSettingsDefaults } = await import(moduleUrl);
const teams = { localTeams: [], localTeamsInitialized: true };

test("blank household fields accept the defaults the runtime filled in", () => {
  const requested = { displayName: "Pixelcorvo", teams, community: { householdId: " ", householdName: "", adults: 1 } };
  const persisted = { teams, community: { householdId: "6521979f1165965b", householdName: "Household Pixelcorvo" } };
  assert.deepEqual(withNativeSettingsDefaults(requested, persisted), {
    displayName: "Pixelcorvo",
    teams,
    community: { householdId: "6521979f1165965b", householdName: "Household Pixelcorvo", adults: 1 },
  });
});

test("operator-provided household fields are never replaced by persisted values", () => {
  const requested = { teams, community: { householdId: "0123456789abcdef", householdName: "Harbour House" } };
  const persisted = { teams, community: { householdId: "ffffffffffffffff", householdName: "Household X" } };
  assert.deepEqual(withNativeSettingsDefaults(requested, persisted).community, requested.community);
});

test("first-save verification accepts native team initialization and migrated members", () => {
  const requested = { community: { householdId: "id", householdName: "House" },
    teams: { activeTeamUid: "yellow", aliases: [], localTeams: [], localTeamsInitialized: false } };
  const persisted = { ...requested, teams: { ...requested.teams, localTeamsInitialized: true,
    localTeams: [{ teamUid: "yellow", memberDestinations: ["a".repeat(32)] }] } };
  assert.deepEqual(withNativeSettingsDefaults(requested, persisted), persisted);
  assert.equal(requested.teams.localTeamsInitialized, false);
  assert.deepEqual(requested.teams.localTeams, []);
});

test("configured teams and nonempty uninitialized teams remain strictly compared", () => {
  for (const initialized of [true, false]) {
    const requested = { community: { householdId: "id", householdName: "House" },
      teams: { localTeamsInitialized: initialized, localTeams: [{ teamUid: "yellow", memberDestinations: [] }] } };
    const persisted = { ...requested, teams: { localTeamsInitialized: true,
      localTeams: [{ teamUid: "yellow", memberDestinations: ["b".repeat(32)] }] } };
    assert.deepEqual(withNativeSettingsDefaults(requested, persisted), requested);
    assert.notDeepEqual(withNativeSettingsDefaults(requested, persisted), persisted);
  }
});

test("initialization does not mask changed active team, aliases or unrelated settings", () => {
  const requested = { displayName: "Requested", community: { householdId: "id", householdName: "House" },
    teams: { activeTeamUid: "yellow", aliases: [], localTeams: [], localTeamsInitialized: false } };
  const persisted = { ...requested, displayName: "Different", teams: { activeTeamUid: "blue",
    aliases: [{ teamUid: "blue", alias: "Different" }], localTeams: [], localTeamsInitialized: true } };
  const expected = withNativeSettingsDefaults(requested, persisted);
  assert.equal(expected.displayName, requested.displayName);
  assert.equal(expected.teams.activeTeamUid, requested.teams.activeTeamUid);
  assert.deepEqual(expected.teams.aliases, requested.teams.aliases);
  assert.notDeepEqual(expected, persisted);
});

test("an uninitialized native response cannot silently fill team contents", () => {
  const requested = { community: { householdId: "id", householdName: "House" },
    teams: { localTeams: [], localTeamsInitialized: false } };
  const persisted = { ...requested, teams: { localTeamsInitialized: false,
    localTeams: [{ teamUid: "yellow", memberDestinations: [] }] } };
  assert.deepEqual(withNativeSettingsDefaults(requested, persisted), requested);
});
