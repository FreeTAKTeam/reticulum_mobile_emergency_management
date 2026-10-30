const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

test("persistent pairing replacement and forgetting use the trusted configuration channel", () => {
  const controls = new Map();
  const messages = [];
  let connect;
  const document = { getElementById(id) {
    if (!controls.has(id)) controls.set(id, { addEventListener(type, handler) { this[type] = handler; } });
    return controls.get(id);
  } };
  // PluginConfigurationActivity forwards only these documented message types.
  const acceptedTypes = new Set(["ready", "getState", "update", "action"]);
  const port = { start() {}, postMessage(raw) {
    const message = JSON.parse(raw);
    assert.ok(acceptedTypes.has(message.type), "the REM host must forward the real UI request");
    messages.push(message);
  } };
  const source = fs.readFileSync(path.join(__dirname, "../android/src/main/assets/rem-plugin-config/config.js"), "utf8");
  vm.runInNewContext(source, { document, window: { addEventListener(type, handler) { connect = handler; } } });
  connect({ data: "rem-plugin-config-v1", ports: [port] });
  controls.get("renewRadar").click();
  assert.deepEqual(messages.at(-1), { type: "action", action: "radar.renew" });
  assert.equal(controls.get("status").textContent, "Replacing pairing code…");
  port.onmessage({ data: JSON.stringify({ type: "state", running: true, radarEnabled: true,
    radarSessionActive: true, radarPairingPersistent: true, radarToken: "fixture-code", snapshotAgeMs: -1 }) });
  assert.equal(controls.get("radarSession").textContent, "Saved · does not expire");
  assert.equal(controls.get("status").textContent, "Configuration loaded");
  assert.equal(controls.get("forgetRadar").disabled, false);
  controls.get("forgetRadar").click();
  assert.deepEqual(messages.at(-1), { type: "action", action: "radar.forget" });
  port.onmessage({ data: JSON.stringify({ type: "state", running: true, radarEnabled: false,
    radarSessionActive: false, radarPairingPersistent: false, radarToken: "", snapshotAgeMs: -1 }) });
  assert.equal(controls.get("radarToken").value, "");
  assert.equal(controls.get("forgetRadar").disabled, true);
  assert.equal(controls.get("renewRadar").disabled, true);
});
