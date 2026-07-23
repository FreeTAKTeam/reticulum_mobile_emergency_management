import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../stores/messagingModel.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  chatSendModeForDestination,
  hasInboundReplyHistory,
  isLocalChatMessageId,
  isRetryableChatMessage,
} = await import(moduleUrl);

const lxmfDestination = "77777777777777777777777777777777";
const appDestination = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

function nodeStore() {
  return {
    bestPropagationNodeHex: undefined,
    discoveredByDestination: {},
    savedByDestination: {},
  };
}

function message(direction, overrides = {}) {
  return {
    messageIdHex: direction === "Inbound" ? "inbound-1" : "outbound-1",
    conversationId: lxmfDestination,
    direction,
    destinationHex: lxmfDestination,
    sourceHex: direction === "Inbound" ? lxmfDestination : undefined,
    bodyUtf8: "hello",
    method: "Direct",
    state: direction === "Inbound" ? "Received" : "Failed",
    transportState: direction === "Inbound" ? "TransportDelivered" : "Failed",
    applicationAckState: direction === "Inbound" ? "NotRequired" : "Failed",
    updatedAtMs: 1,
    ...overrides,
  };
}

test("persisted inbound history authorizes a standard LXMF reply", () => {
  const store = nodeStore();
  assert.equal(
    hasInboundReplyHistory(lxmfDestination, [message("Inbound")], store),
    true,
  );
  assert.equal(chatSendModeForDestination(lxmfDestination, store, true), "Auto");
});

test("outbound-only history does not authorize an unsaved destination", () => {
  const store = nodeStore();
  assert.equal(
    hasInboundReplyHistory(lxmfDestination, [message("Outbound")], store),
    false,
  );
  assert.equal(chatSendModeForDestination(lxmfDestination, store, false), "DirectOnly");
});

test("inbound LXMF and application destination aliases share reply authorization", () => {
  const store = nodeStore();
  store.discoveredByDestination[appDestination] = {
    destination: appDestination,
    lxmfDestinationHex: lxmfDestination,
  };

  assert.equal(
    hasInboundReplyHistory(appDestination, [message("Inbound")], store),
    true,
  );
});

test("retry eligibility is limited to failed outbound messages", () => {
  assert.equal(isLocalChatMessageId("local-123"), true);
  assert.equal(isLocalChatMessageId("native-123"), false);
  assert.equal(isRetryableChatMessage(message("Outbound")), true);
  assert.equal(isRetryableChatMessage(message("Inbound", { state: "Failed" })), false);
  assert.equal(isRetryableChatMessage(message("Outbound", { state: "Queued" })), false);
});
