import { expect, test } from "@playwright/test";

import { defaultSettings, seedAppStorage } from "./support/app";

test("can mock the current REM splash screen", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await page.goto("/dashboard?mock=splash-screen");

  const splash = page.getByTestId("splash-screen");
  await expect(splash).toBeVisible();
  await expect(splash.getByRole("img", { name: "R.E.M. logo" })).toBeVisible();
  await expect(splash.getByText("Version 1.2.5")).toBeVisible();
});

test("can mock interface loading on the current splash screen", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      rnode: {
        ...defaultSettings.rnode,
        enabled: true,
        peripheralId: "00:11:22:33:44:55",
        displayName: "RNode REM",
      },
    },
  });

  await page.goto("/dashboard?mock=splash-interface-loading");

  const splash = page.getByTestId("splash-screen");
  await expect(splash).toBeVisible();
  await expect(splash.getByRole("img", { name: "R.E.M. logo" })).toBeVisible();
  await expect(splash.getByText("Reticulum Mobile Emergency Management")).toBeVisible();

  const interfaceLoading = page.getByTestId("splash-interface-loading");
  await expect(interfaceLoading).toBeVisible();
  await expect(interfaceLoading.getByText("Interfaces are loading")).toBeVisible();
  await expect(page.getByTestId("splash-loading-animation")).toBeVisible();
  await expect(interfaceLoading.getByText("Waiting for active links to report traffic.")).toBeVisible();
  await expect(page.getByTestId("splash-interface-rnode")).toContainText("LoRa");
  await expect(page.getByTestId("splash-interface-rnode")).toContainText("loading");
  await expect(page.getByTestId("splash-interface-tcp")).toContainText("TCP community");
  await expect(page.getByTestId("splash-interface-tcp")).toContainText("loading");
  await expect(page.getByTestId("splash-interface-local")).toContainText("Reticulum Net");
  await expect(page.getByTestId("splash-interface-local")).toContainText("loading");
});
