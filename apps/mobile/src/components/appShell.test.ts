import { expect, test, vi } from "vitest";
import { createSSRApp, h } from "vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { renderToString } from "vue/server-renderer";

import AppToolsMenu from "./AppToolsMenu.vue";
import { footerItems, iconPaths, menuItems } from "./appNavigation";

// router.ts builds a browser history at import time; node has no window.
vi.mock("vue-router", async (importOriginal) => {
  const actual = await importOriginal<typeof import("vue-router")>();
  return { ...actual, createWebHistory: () => actual.createMemoryHistory() };
});

test("every footer and menu entry resolves to a registered route", async () => {
  const { router } = await import("../router");
  for (const item of [...footerItems, ...menuItems]) {
    expect(router.resolve(item.path).matched.length, item.path).toBeGreaterThan(0);
  }
});

test("every navigation icon has drawable paths", () => {
  for (const item of [...footerItems, ...menuItems]) {
    expect(iconPaths[item.icon]?.length, item.icon).toBeGreaterThan(0);
  }
});

async function renderMenu(open: boolean, currentPath: string): Promise<string> {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: menuItems.map((item) => ({ path: item.path, component: { render: () => null } })),
  });
  const app = createSSRApp({
    render: () => h(AppToolsMenu, { open, currentPath, stopNode: async () => undefined }),
  });
  app.use(router);
  await router.push(currentPath);
  return renderToString(app);
}

test("tools menu renders one tile per menu entry and marks the active one", async () => {
  const html = await renderMenu(true, "/checklists/abc");
  for (const item of menuItems) {
    expect(html).toContain(`aria-label="${item.label}"`);
  }
  expect(html).toMatch(/class="tool-tile active"[^>]*aria-label="Checklists"|aria-label="Checklists"[^>]*class="tool-tile active"/);
  expect(html.match(/tool-tile active/g)).toHaveLength(1);
  expect(html).toContain("Quit");
});

test("closed tools menu renders nothing", async () => {
  const html = await renderMenu(false, "/dashboard");
  expect(html).not.toContain("tools-menu");
});
