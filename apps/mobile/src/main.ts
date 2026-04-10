import { Capacitor } from "@capacitor/core";
import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { router } from "./router";
import "./styles.css";

type ConsoleMethod = "debug" | "info" | "warn" | "error" | "log";
type CapacitorLoggingBridge = {
  isLoggingEnabled?: boolean;
};

function suppressUndefinedConsoleNoise(): void {
  const methods: ConsoleMethod[] = ["debug", "info", "warn", "error", "log"];
  for (const method of methods) {
    const original = console[method].bind(console) as (...args: unknown[]) => void;
    console[method] = ((...args: unknown[]) => {
      if (args.length === 1 && args[0] === undefined) {
        return;
      }
      original(...args);
    }) as Console[typeof method];
  }
}

function disableCapacitorNativeBridgeResultLogging(): void {
  if (Capacitor.getPlatform() !== "android") {
    return;
  }
  const bridge = (globalThis as typeof globalThis & {
    Capacitor?: CapacitorLoggingBridge;
  }).Capacitor;
  if (!bridge) {
    return;
  }
  bridge.isLoggingEnabled = false;
}

suppressUndefinedConsoleNoise();
disableCapacitorNativeBridgeResultLogging();

const app = createApp(App);
app.use(createPinia());
app.use(router);

void router.isReady().finally(() => {
  app.mount("#app");
});
