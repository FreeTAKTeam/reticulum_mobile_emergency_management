import { expect, test } from "@playwright/test";

import { DEFAULT_TCP_COMMUNITY_ENDPOINT } from "../apps/mobile/src/utils/tcpCommunityServers";
import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

test("fresh installs default to the first TCP community server", async ({ page }) => {
  await seedAppStorage(page, {});

  await gotoApp(page, "/settings");

  const runtimePanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Runtime" }),
  });

  await runtimePanel.locator("summary").click();

  const firstServer = page
    .locator("label.server-option")
    .filter({ hasText: DEFAULT_TCP_COMMUNITY_ENDPOINT });

  await expect(firstServer.getByRole("checkbox")).toBeChecked();

  await page.getByRole("button", { name: "Save" }).click();

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(storedSettings.tcpClients).toEqual([DEFAULT_TCP_COMMUNITY_ENDPOINT]);
});

test("legacy placeholder TCP selection normalizes to the first community server", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      tcpClients: ["rmap.world:4242"],
    },
  });

  await gotoApp(page, "/settings");

  const runtimePanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Runtime" }),
  });

  await runtimePanel.locator("summary").click();

  const firstServer = page
    .locator("label.server-option")
    .filter({ hasText: DEFAULT_TCP_COMMUNITY_ENDPOINT });

  await expect(firstServer.getByRole("checkbox")).toBeChecked();
  await expect(page.getByText("rmap.world:4242")).toHaveCount(0);

  await page.getByRole("button", { name: "Save" }).click();

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(storedSettings.tcpClients).toEqual([DEFAULT_TCP_COMMUNITY_ENDPOINT]);
});

test("operators can update runtime settings and persist TCP endpoints", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      displayName: "Atlas-1",
    },
  });

  await gotoApp(page, "/settings");

  const runtimePanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Runtime" }),
  });

  await runtimePanel.locator("summary").click();
  await runtimePanel.getByLabel("Call Sign").fill("Atlas-7");
  await runtimePanel.getByPlaceholder("Add custom endpoint (host:port)").fill("mesh.example.org:5151");
  await runtimePanel.getByRole("button", { name: "Add" }).click();

  await expect(runtimePanel.getByText("mesh.example.org:5151")).toBeVisible();

  await runtimePanel.getByRole("button", { name: "Save" }).click();

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(storedSettings.displayName).toBe("Atlas-7");
  expect(storedSettings.tcpClients).toContain("mesh.example.org:5151");
});
