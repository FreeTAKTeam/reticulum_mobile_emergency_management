import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { router } from "./router";
import "./styles.css";

type ConsoleMethod = "debug" | "info" | "warn" | "error" | "log";

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

suppressUndefinedConsoleNoise();

const app = createApp(App);
app.use(createPinia());
app.use(router);

void router.isReady().finally(() => {
  app.mount("#app");
});
