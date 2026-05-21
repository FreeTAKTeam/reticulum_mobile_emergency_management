import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf8"),
) as { version?: string };

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
    "import.meta.env.VITE_APP_VERSION": JSON.stringify(packageJson.version ?? "0.0.0"),
  },
  plugins: [vue()],
});
