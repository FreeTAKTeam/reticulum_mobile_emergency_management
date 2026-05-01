import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

const BLANK_MAP_STYLE = {
  version: 8,
  sources: {},
  layers: [],
};

test("telemetry map shows live and stale markers while filtering expired fixes", async ({ page }) => {
  const now = Date.now();

  await page.route("https://tiles.openfreemap.org/styles/liberty*", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify(BLANK_MAP_STYLE),
    });
  });

  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      telemetry: {
        enabled: false,
        publishIntervalSeconds: 10,
        staleAfterMinutes: 5,
        expireAfterMinutes: 10,
      },
    },
    telemetry: [
      {
        callsign: "Rescue-1",
        lat: 44.6488,
        lon: -63.5752,
        speed: 12.5,
        updatedAt: now - 45_000,
      },
      {
        callsign: "Relay-3",
        lat: 44.6713,
        lon: -63.6117,
        updatedAt: now - 6 * 60_000,
      },
      {
        callsign: "Expired-9",
        lat: 44.69,
        lon: -63.58,
        updatedAt: now - 11 * 60_000,
      },
    ],
  });

  await gotoApp(page, "/dashboard");
  await page.getByRole("link", { name: "Map" }).click();

  await expect(page).toHaveURL(/\/telemetry$/);
  await expect(page.getByRole("heading", { name: "Map" })).toBeVisible();
  await expect(page.getByText("1 Live")).toBeVisible();
  await expect(page.getByText("Stale: 1")).toBeVisible();
  await expect(page.locator(".map-container .maplibregl-canvas")).toBeVisible();

  await expect(page.locator(".telemetry-marker")).toHaveCount(2);
  await expect(page.locator('.telemetry-marker.is-live[title="Rescue-1"]')).toBeVisible();
  await expect(page.locator('.telemetry-marker.is-stale[title="Relay-3"]')).toBeVisible();
  await expect(page.locator('.telemetry-marker[title="Expired-9"]')).toHaveCount(0);

  await page.locator('.telemetry-marker[title="Rescue-1"]').click();
  await expect(page.locator(".maplibregl-popup")).toContainText("Rescue-1");
  await expect(page.locator(".maplibregl-popup")).toContainText("Speed 12.5");
});
