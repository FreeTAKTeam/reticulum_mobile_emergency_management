"use strict";

let port;
const fields = {
  alias: document.getElementById("alias"),
  operator: document.getElementById("operator"),
  stale: document.getElementById("stale"),
  sharing: document.getElementById("sharing"),
  destination: document.getElementById("destination"),
  interval: document.getElementById("interval"),
};
const device = document.getElementById("device");
const connection = document.getElementById("connection");
const status = document.getElementById("status");

function send(message) {
  if (port) port.postMessage(JSON.stringify(message));
}

function render(state) {
  device.textContent = state.deviceName
    ? `${state.deviceName} (${state.selectedDevice || "unknown"})`
    : state.selectedDevice || "Not paired";
  connection.textContent = state.connectionStatus || "Unknown";
  fields.alias.value = state.alias || "Heart rate";
  fields.operator.value = state.operatorRnsIdentity || "";
  fields.stale.value = state.staleTimeoutSeconds || 30;
  fields.sharing.checked = state.sharingEnabled === true;
  fields.destination.value = state.destination || "";
  fields.interval.value = state.sendIntervalSeconds || 30;
  status.textContent = "Configuration loaded";
}

window.addEventListener("message", (event) => {
  if (event.data !== "rem-plugin-config-v1" || !event.ports[0]) return;
  port = event.ports[0];
  port.onmessage = (message) => {
    const response = JSON.parse(message.data);
    if (response.type === "state") render(response);
    if (response.type === "validationError") status.textContent = response.message || "Configuration rejected";
    if (response.type === "actionResult") {
      status.textContent = "Pairing opened in the plugin";
      window.setTimeout(() => send({ type: "getState" }), 500);
    }
  };
  send({ type: "ready" });
  send({ type: "getState" });
});

document.getElementById("pair").addEventListener("click", () => {
  status.textContent = "Opening pairing…";
  send({ type: "action", action: "permissions.pair" });
});

document.getElementById("save").addEventListener("click", () => {
  status.textContent = "Saving…";
  send({
    type: "update",
    alias: fields.alias.value,
    operatorRnsIdentity: fields.operator.value,
    staleTimeoutSeconds: Number(fields.stale.value),
    sharingEnabled: fields.sharing.checked,
    destination: fields.destination.value,
    sendIntervalSeconds: Number(fields.interval.value),
  });
});
