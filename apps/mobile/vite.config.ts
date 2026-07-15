import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const packageJson = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf8"),
) as { version?: string };

function firstNonEmpty(...values: Array<string | undefined>): string | undefined {
  return values.map((value) => value?.trim()).find((value): value is string => Boolean(value));
}

const appVersion = firstNonEmpty(
  process.env.VITE_APP_VERSION,
  process.env.ORG_GRADLE_PROJECT_appVersionName,
  packageJson.version,
) ?? "0.0.0";

function manualChunks(id: string): string | undefined {
  if (!id.includes("node_modules")) {
    return undefined;
  }
  if (id.includes("maplibre-gl") || id.includes("@maplibre")) {
    return "maplibre";
  }
  if (
    id.includes("/vue/")
    || id.includes("\\vue\\")
    || id.includes("/vue-router/")
    || id.includes("\\vue-router\\")
    || id.includes("/pinia/")
    || id.includes("\\pinia\\")
  ) {
    return "vendor";
  }
  return "vendor";
}

export default defineConfig(({ mode }) => ({
  build: {
    // Route-level lazy loading leaves MapLibre as an intentional on-demand chunk.
    chunkSizeWarningLimit: 900,
    rollupOptions: {
      output: {
        manualChunks,
      },
    },
  },
  define: {
    "import.meta.env.VITE_APP_VERSION": JSON.stringify(appVersion),
  },
  resolve: {
    alias: mode === "web"
      ? [{
          find: "@reticulum/node-client",
          replacement: fileURLToPath(
            new URL("../../packages/node-client/src/web-entry.ts", import.meta.url),
          ),
        }]
      : [],
  },
  plugins: [vue()],
}));
