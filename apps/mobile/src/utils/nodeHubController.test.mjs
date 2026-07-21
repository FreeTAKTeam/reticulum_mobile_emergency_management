import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

function dataModule(source) {
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`;
}

const hubRegistryMock = dataModule(`
  export function buildHubRegistryBootstrapProfile(options) {
    return { ...options, teamColor: "yellow" };
  }
  export function saveHubRegistryLinkage() {}
`);
const nodeClientMock = dataModule(`
  export const YELLOW_TEAM_UID = "d6b6e188b910d6bdd24d04b7a7ec5444";
`);
const nodeSettingsMock = dataModule(`
  export function hasSelectedHubIdentity(identityHash) {
    return typeof identityHash === "string" && identityHash.trim().length > 0;
  }
  export function hubModeUsesRch(mode) {
    return mode === "Connected" || mode === "SemiAutonomous";
  }
`);
const nodeStoreCoreMock = dataModule(`
  export const EMPTY_BYTES = new Uint8Array(0);
  export function asTrimmedString(value) {
    return typeof value === "string" ? value.trim() : "";
  }
  export function nowMs() { return 123; }
`);

const source = await readFile(new URL("../stores/nodeHubController.ts", import.meta.url), "utf8");
let transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
for (const [specifier, replacement] of [
  ["@reticulum/node-client", nodeClientMock],
  ["../services/hubRegistryBootstrap", hubRegistryMock],
  ["./nodeSettingsModel", nodeSettingsMock],
  ["./nodeStoreCore", nodeStoreCoreMock],
]) {
  transpiled = transpiled.replaceAll(`"${specifier}"`, `"${replacement}"`);
}
const moduleUrl = dataModule(transpiled);
const { createNodeHubController } = await import(moduleUrl);

test("RCH directory refresh failures retain the hub-scoped cached roster", async () => {
  const nativeCalls = [];
  const hubRegistration = { status: "pending", lastError: "" };
  const cachedSnapshot = {
    schemaVersion: 2,
    hubIdentityHash: "7f0cc7b986c9967dfb6eefb43bc498c9",
    activeTeamUid: "d6b6e188b910d6bdd24d04b7a7ec5444",
    effectiveConnectedMode: false,
    teams: [{
      uid: "d6b6e188b910d6bdd24d04b7a7ec5444",
      color: "YELLOW",
      teamName: "YELLOW",
    }],
    callerMemberships: [{
      teamUid: "d6b6e188b910d6bdd24d04b7a7ec5444",
      teamMemberUid: "member-yellow",
    }],
    members: [],
    items: [],
    receivedAtMs: 1,
  };
  const hubDirectorySnapshot = { value: cachedSnapshot };
  const client = {
    value: {
      async refreshHubDirectory() {
        nativeCalls.push("refresh");
        throw new Error("network error");
      },
    },
  };
  const controller = createNodeHubController({
    appendLog() {},
    client,
    errorMessage(error) {
      return error instanceof Error ? error.message : String(error);
    },
    hubDirectorySnapshot,
    hubRegistration,
    settings: {
      displayName: "Pixel",
      hub: {
        mode: "SemiAutonomous",
        identityHash: "7f0cc7b986c9967dfb6eefb43bc498c9",
      },
      teams: {
        activeTeamUid: "d6b6e188b910d6bdd24d04b7a7ec5444",
        aliases: [],
      },
    },
    status: {
      value: {
        running: true,
        identityHex: "pixel-identity",
        appDestinationHex: "pixel-destination",
      },
    },
  });

  await assert.rejects(controller.bootstrapHubRegistration(true), /network error/);

  assert.deepEqual(nativeCalls, ["refresh"]);
  assert.equal(hubDirectorySnapshot.value, cachedSnapshot);
  assert.equal(hubRegistration.status, "error");
  assert.equal(hubRegistration.lastError, "network error");
});
