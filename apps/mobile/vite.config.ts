import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf8"),
) as { version?: string };

function firstNonEmpty(...values: Array<string | undefined>): string | undefined {
  return values.map((value) => value?.trim()).find((value): value is string => Boolean(value));
}

function readGradleProperty(name: string): string | undefined {
  try {
    const properties = readFileSync(new URL("./android/gradle.properties", import.meta.url), "utf8");
    const line = properties
      .split(/\r?\n/g)
      .map((entry) => entry.trim())
      .find((entry) => entry.startsWith(`${name}=`));
    return line?.slice(name.length + 1).trim();
  } catch {
    return undefined;
  }
}

const appVersion = firstNonEmpty(
  process.env.VITE_APP_VERSION,
  process.env.ORG_GRADLE_PROJECT_appVersionName,
  readGradleProperty("appVersionName"),
  packageJson.version,
) ?? "0.0.0";

const appBuildVersion = firstNonEmpty(
  process.env.VITE_APP_BUILD_VERSION,
  process.env.ORG_GRADLE_PROJECT_appVersionCode,
  readGradleProperty("appVersionCode"),
) ?? "local";

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

export default defineConfig({
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
    "import.meta.env.VITE_APP_BUILD_VERSION": JSON.stringify(appBuildVersion),
  },
  plugins: [vue()],
});
