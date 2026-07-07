import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./startupInterfaces.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { buildStartupInterfaceItems, statusHasReceivingInterface } = await import(moduleUrl);

const configuredSettings = {
  tcpClients: ["rns.example.net:4242"],
  rnode: {
    enabled: true,
    connectionMode: "ble",
    peripheralId: "00:11:22:33:44:55",
  },
};

function itemById(items, id) {
  const item = items.find((entry) => entry.id === id);
  assert.ok(item, `missing startup interface item ${id}`);
  return item;
}

test("disabled comes from interface configuration", () => {
  const items = buildStartupInterfaceItems(
    {
      running: true,
      interfaces: [],
    },
    {
      tcpClients: [],
      rnode: {
        enabled: false,
        connectionMode: "ble",
        peripheralId: "",
      },
    },
  );

  assert.equal(itemById(items, "rnode").state, "disabled");
  assert.equal(itemById(items, "tcp").state, "disabled");
  assert.equal(itemById(items, "local").state, "loading");
});

test("configured interfaces load before backend interface records arrive", () => {
  const items = buildStartupInterfaceItems(
    {
      running: true,
      interfaces: [],
    },
    configuredSettings,
  );

  assert.equal(itemById(items, "rnode").state, "loading");
  assert.equal(itemById(items, "tcp").state, "loading");
  assert.equal(itemById(items, "local").state, "loading");
});

test("connected interfaces wait until RX activity is reported", () => {
  const items = buildStartupInterfaceItems(
    {
      running: true,
      interfaces: [
        {
          interfaceHex: "01",
          label: "rnode-ble:RNode",
          kind: "rnode_ble",
          state: "connected",
          rxPackets: 0,
          rxBytes: 0,
          lastActivityMs: 0,
        },
        {
          interfaceHex: "02",
          label: "rns.example.net:4242",
          kind: "tcp_client",
          state: "connected",
          rxPackets: 0,
          rxBytes: 0,
          lastActivityMs: 0,
        },
      ],
    },
    configuredSettings,
  );

  assert.equal(itemById(items, "rnode").state, "waiting");
  assert.equal(itemById(items, "tcp").state, "waiting");
  assert.equal(itemById(items, "local").state, "waiting");
});

test("RX activity marks the receiving interface and Reticulum Net ready", () => {
  const items = buildStartupInterfaceItems(
    {
      running: true,
      interfaces: [
        {
          interfaceHex: "01",
          label: "rnode-ble:RNode",
          kind: "rnode_ble",
          state: "connected",
          rxPackets: 0,
          rxBytes: 0,
          lastActivityMs: 0,
        },
        {
          interfaceHex: "02",
          label: "rns.example.net:4242",
          kind: "tcp_client",
          state: "connected",
          rxPackets: 4,
          rxBytes: 512,
          lastActivityMs: 1000,
        },
      ],
    },
    configuredSettings,
  );

  assert.equal(itemById(items, "rnode").state, "waiting");
  assert.equal(itemById(items, "tcp").state, "ready");
  assert.equal(itemById(items, "local").state, "ready");
});

test("readiness requires a connected interface with RX activity", () => {
  assert.equal(
    statusHasReceivingInterface({
      interfaces: [
        {
          interfaceHex: "01",
          label: "rnode-ble:RNode",
          kind: "rnode_ble",
          state: "disconnected",
          rxPackets: 4,
          rxBytes: 512,
          lastActivityMs: 1000,
        },
      ],
    }),
    false,
  );

  assert.equal(
    statusHasReceivingInterface({
      interfaces: [
        {
          interfaceHex: "02",
          label: "rnode-ble:RNode",
          kind: "rnode_ble",
          state: "connected",
          rxPackets: 1,
          rxBytes: 48,
          lastActivityMs: 1000,
        },
      ],
    }),
    true,
  );
});
