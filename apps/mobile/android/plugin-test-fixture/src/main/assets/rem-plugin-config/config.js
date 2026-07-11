"use strict";

let port;
const label = document.getElementById("label");
const status = document.getElementById("status");

window.addEventListener("message", (event) => {
  if (event.data !== "rem-plugin-config-v1" || !event.ports[0]) return;
  port = event.ports[0];
  port.onmessage = (message) => {
    const payload = JSON.parse(message.data);
    if (payload.type === "state") {
      label.value = payload.label || "";
      status.textContent = "Configuration loaded";
    } else if (payload.type === "validationError") {
      status.textContent = payload.message || "Configuration failed";
    }
  };
  port.postMessage(JSON.stringify({ type: "ready" }));
});

document.getElementById("save").addEventListener("click", () => {
  if (!port) return;
  port.postMessage(JSON.stringify({ type: "update", label: label.value }));
});
