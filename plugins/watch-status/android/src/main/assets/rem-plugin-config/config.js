"use strict";
let port;
const enabled = document.getElementById("enabled");
const portInput = document.getElementById("port");
const url = document.getElementById("url");
const server = document.getElementById("server");
const snapshot = document.getElementById("snapshot");
const status = document.getElementById("status");
const radarEnabled = document.getElementById("radarEnabled");
const radarToken = document.getElementById("radarToken");
const renewRadar = document.getElementById("renewRadar");
const forgetRadar = document.getElementById("forgetRadar");
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
  radarEnabled.checked = state.radarEnabled === true;
  radarToken.value = state.radarToken || "";
  document.getElementById("radarUrl").textContent = state.radarUrl || "Unavailable";
  document.getElementById("radarSession").textContent = state.radarPairingPersistent
    ? (state.radarSessionActive ? "Saved · does not expire" : "Saved · reconnects when server starts")
    : (state.radarEnabled ? "Not paired · replace pairing code to pair" : "Off · enable radar to pair");
  renewRadar.disabled = !state.running || !state.radarEnabled;
  forgetRadar.disabled = !state.radarPairingPersistent;
  status.textContent = "Configuration loaded";
}
window.addEventListener("message", (event) => {
  if (event.data !== "rem-plugin-config-v1" || !event.ports[0]) return;
  port = event.ports[0];
  if (typeof port.start === "function") port.start();
  port.onmessage = (message) => {
    try { handleResponse(JSON.parse(message.data)); }
    catch { status.textContent = "Unable to read configuration response"; }
  };
  send({ type: "ready" });
  send({ type: "getState" });
});
document.getElementById("save").addEventListener("click", () => {
  status.textContent = "Saving…";
  send({ type: "update", enabled: enabled.checked, port: Number(portInput.value), radarEnabled: radarEnabled.checked });
});
renewRadar.addEventListener("click", () => {
  status.textContent = "Replacing pairing code…";
  send({ type: "action", action: "radar.renew" });
});
forgetRadar.addEventListener("click", () => {
  status.textContent = "Forgetting pairing…";
  send({ type: "action", action: "radar.forget" });
});
document.getElementById("revealRadarToken").addEventListener("change", (event) => {
  radarToken.type = event.target.checked ? "text" : "password";
});
