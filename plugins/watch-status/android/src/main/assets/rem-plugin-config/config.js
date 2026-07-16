"use strict";
let port;
const enabled = document.getElementById("enabled");
const portInput = document.getElementById("port");
const url = document.getElementById("url");
const server = document.getElementById("server");
const snapshot = document.getElementById("snapshot");
const status = document.getElementById("status");
function send(message) { if (port) port.postMessage(JSON.stringify(message)); }
function handleResponse(response) {
  if (response.type === "state") render(response);
  if (response.type === "validationError") status.textContent = response.message || "Configuration rejected";
}
function render(state) {
  enabled.checked = state.enabled === true;
  portInput.value = state.port || 29863;
  url.textContent = state.url || "Unavailable";
  server.textContent = state.bindError || (state.running ? "Listening" : "Stopped");
  snapshot.textContent = state.snapshotAgeMs < 0 ? "Unavailable" : `${Math.round(state.snapshotAgeMs / 1000)}s old`;
  status.textContent = "Configuration loaded";
}
window.addEventListener("message", (event) => {
  if (event.data !== "rem-plugin-config-v1" || !event.ports[0]) return;
  port = event.ports[0];
  if (typeof port.start === "function") port.start();
  port.onmessage = (message) => handleResponse(JSON.parse(message.data));
  send({ type: "ready" });
  send({ type: "getState" });
});
document.getElementById("save").addEventListener("click", () => {
  status.textContent = "Saving…";
  send({ type: "update", enabled: enabled.checked, port: Number(portInput.value) });
});
