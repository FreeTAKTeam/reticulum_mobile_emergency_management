"use strict";

let port;
let initialized = false;
let pending = false;
const byId = (id) => document.getElementById(id);

function send(message) {
  if (port) port.postMessage(JSON.stringify(message));
}

function render(state) {
  byId("device").textContent = `${state.deviceName || "Not paired"} ${state.address || ""}`;
  byId("connection").textContent = state.status || "Disconnected";
  byId("status").textContent = state.error || state.status || "Ready";
  if (!initialized) {
    byId("gid").value = state.gid || "";
    byId("nickname").value = state.nickname || "REM";
    byId("relay").checked = state.relayEnabled === true;
    initialized = true;
  }
  for (const id of ["connect", "sendText", "sendPosition", "pair"]) {
    byId(id).disabled = state.busy === true || ((id === "sendText" || id === "sendPosition") && !state.connected);
  }
  byId("disconnect").disabled = !state.connected && !state.busy;

}

window.addEventListener("message", (event) => {
  if (event.data !== "rem-plugin-config-v1" || !event.ports[0] || port) return;
  port = event.ports[0];
  port.onmessage = (message) => {
    pending = false;
    try {
      const response = JSON.parse(message.data);
      if (response.type === "state") render(response);
      if (response.type === "validationError") byId("status").textContent = response.message || "Configuration rejected";
      if (response.type === "actionResult") byId("status").textContent = "Pairing opened. Return here and connect after selecting your radio.";
    } catch {
      byId("status").textContent = "REM returned an invalid configuration response.";
    }
  };
  send({ type: "ready" });
  send({ type: "getState" });
});

byId("settings").addEventListener("submit", (event) => {
  event.preventDefault();
  send({ type: "update", gid: byId("gid").value, nickname: byId("nickname").value,
    relayEnabled: byId("relay").checked });
});

for (const action of ["pair", "connect", "disconnect", "sendText", "sendPosition"]) {
  byId(action).addEventListener("click", () => {
    const text = byId("text").value || (action === "sendPosition" ? "Position" : "");
    if (action.startsWith("send") && (new TextEncoder().encode(text).length > 180 || !text)) {
      byId("status").textContent = "Enter 1–180 UTF-8 bytes of text.";
      return;
    }
    const destinationHex = byId("destination").value.trim().toLowerCase();
    if (action === "sendText" && !/^[0-9a-f]{32}$/.test(destinationHex)) {
      byId("status").textContent = "Enter this person’s 32-digit LXMF destination hash.";
      return;
    }
    send({ type: "action", action, text, destinationHex });
  });
}

window.setInterval(() => {
  if (port && !document.hidden && !pending) {
    pending = true;
    send({ type: "getState" });
  }
}, 2000);
