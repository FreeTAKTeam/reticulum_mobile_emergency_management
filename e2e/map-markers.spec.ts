import { expect, test, type Page } from "@playwright/test";

import { gotoApp, seedAppStorage } from "./support/app";

const BLANK_MAP_STYLE = { version: 8, sources: {}, layers: [] };

async function openMap(page: Page): Promise<void> {
  await page.route("https://tiles.openfreemap.org/styles/liberty*", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify(BLANK_MAP_STYLE) });
  });
  await seedAppStorage(page);
  await gotoApp(page, "/dashboard");
  await page.getByRole("link", { name: "Map" }).click();
  await expect(page).toHaveURL(/\/telemetry$/);
  await expect(page.locator(".map-container .maplibregl-canvas")).toBeVisible();
}

async function panMap(page: Page): Promise<void> {
  const canvas = page.locator(".map-container .maplibregl-canvas");
  await canvas.focus();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowDown");
}

/** The marker count lives in the map menu; open it, check, and close it again. */
async function expectMarkerCount(page: Page, label: string): Promise<void> {
  await page.getByRole("button", { name: "Map menu" }).click();
  await expect(page.getByRole("menuitemcheckbox", { name: label })).toBeVisible();
  await page.getByRole("button", { name: "Map menu" }).click();
}

test("adds, edits and removes a shared-symbol marker from the map", async ({ page }) => {
  await openMap(page);
  await expectMarkerCount(page, "Markers: 0, shown");

  await page.getByRole("button", { name: "Add marker" }).click();
  const picker = page.getByRole("dialog", { name: "Choose a marker symbol" });
  await expect(picker).toBeVisible();
  await expect(picker.getByRole("heading", { name: "NATO" })).toBeVisible();

  await picker.getByLabel("Search marker symbols").fill("wildfire");
  await expect(picker.locator("[data-symbol]")).toHaveCount(1);
  await picker.locator('[data-symbol="fire"]').click();

  const aimBar = page.getByLabel("Aim marker");
  await expect(aimBar.getByRole("button", { name: "Place fire" })).toBeVisible();
  await aimBar.getByRole("button", { name: "Place fire" }).click();

  const details = page.getByRole("dialog", { name: "Marker Fire 1" });
  await expect(details).toBeVisible();
  await expect(details).toContainText("Incident");
  await expect(details).toContainText("Local");
  await expect(page.getByRole("button", { name: "Fire marker: Fire 1" })).toBeVisible();
  await expectMarkerCount(page, "Markers: 1, shown");

  await details.getByRole("button", { name: "Rename" }).click();
  await details.getByLabel("Marker name").fill("  ");
  await details.getByRole("button", { name: "Save name" }).click();
  await expect(details.getByRole("alert")).toHaveText("Enter a name");
  await details.getByLabel("Marker name").fill("Ridge fire");
  await details.getByRole("button", { name: "Save name" }).click();
  await expect(page.getByTestId("marker-detail-name")).toHaveText("Ridge fire");

  const before = await page.getByTestId("marker-detail-name").locator("xpath=..").innerText();
  await page.getByRole("dialog", { name: "Marker Ridge fire" }).getByRole("button", { name: "Move" }).click();
  await expect(page.getByRole("button", { name: "Fire marker: Ridge fire" })).toHaveCount(0);
  await panMap(page);
  await page.getByLabel("Aim marker").getByRole("button", { name: "Move here" }).click();
  const moved = page.getByRole("dialog", { name: "Marker Ridge fire" });
  await expect(moved).toBeVisible();
  await expect.poll(async () => page.getByTestId("marker-detail-name").locator("xpath=..").innerText())
    .not.toBe(before);

  await moved.getByRole("button", { name: "Delete" }).click();
  await moved.getByRole("button", { name: "Keep" }).click();
  await moved.getByRole("button", { name: "Delete" }).click();
  await moved.getByRole("button", { name: "Delete" }).click();
  await expectMarkerCount(page, "Markers: 0, shown");
  await expect(page.locator(".map-marker-pin")).toHaveCount(0);
});

test("recent symbols, long-press placement and the visibility toggle", async ({ page }) => {
  await openMap(page);

  await page.getByRole("button", { name: "Add marker" }).click();
  await page.getByRole("dialog", { name: "Choose a marker symbol" }).locator('[data-symbol="medic"]').click();
  await page.getByLabel("Aim marker").getByRole("button", { name: "Place medic" }).click();
  await page.getByRole("dialog", { name: "Marker Medic 1" }).getByRole("button", { name: "Close" }).click();

  await page.getByRole("button", { name: "Add marker" }).click();
  const recent = page.getByTestId("marker-recent-symbols");
  await expect(recent.locator("[data-symbol]")).toHaveCount(1);
  await expect(recent.locator('[data-symbol="medic"]')).toBeVisible();
  await page.getByRole("dialog", { name: "Choose a marker symbol" })
    .getByRole("button", { name: "Close symbol picker" }).click();

  const canvas = page.locator(".map-container .maplibregl-canvas");
  await canvas.click({ button: "right", position: { x: 60, y: 160 } });
  await expect(page.getByRole("dialog", { name: "Marker Medic 2" })).toBeVisible();
  await page.getByRole("dialog", { name: "Marker Medic 2" }).getByRole("button", { name: "Close" }).click();
  await expect(page.locator(".map-marker-pin")).toHaveCount(2);

  await page.getByRole("button", { name: "Map menu" }).click();
  const mapMenu = page.getByRole("menu", { name: "Map menu" });
  await mapMenu.getByRole("menuitemcheckbox", { name: "Markers: 2, shown" }).click();
  await expect(page.locator(".map-marker-pin")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Add marker" })).toHaveCount(0);
  await mapMenu.getByRole("menuitemcheckbox", { name: "Markers: 2, hidden" }).click();
  await expect(page.locator(".map-marker-pin")).toHaveCount(2);
  await page.getByRole("button", { name: "Map menu" }).click();
  await expect(mapMenu).toHaveCount(0);
});

test("uses RCH colors through placement and selection without a frame", async ({ page }, testInfo) => {
  await openMap(page);
  const colors = [
    ["friendly", "rgb(107, 203, 236)"],
    ["hostile", "rgb(222, 103, 103)"],
    ["neutral", "rgb(155, 187, 89)"],
    ["unknown", "rgb(241, 224, 151)"],
    ["fire", "rgb(245, 158, 11)"],
  ];
  for (const [symbol, color] of colors) {
    await page.getByRole("button", { name: "Add marker" }).click();
    const option = page.getByRole("dialog", { name: "Choose a marker symbol" })
      .locator(`[data-symbol="${symbol}"]`).last();
    await expect(option.locator("svg")).toHaveCSS("color", color);
    await option.click();
    await expect(page.locator(".marker-crosshair-pin svg")).toHaveCSS("color", color);
    await page.getByLabel("Aim marker").getByRole("button", { name: `Place ${symbol}` }).click();
    const pin = page.locator(`.map-marker-pin[data-symbol="${symbol}"]`);
    const icon = pin.locator(".map-marker-pin-icon");
    await expect(pin).toHaveClass(/is-selected/);
    await expect(icon.locator("path")).toHaveCSS("fill", color);
    await expect(icon).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
    await expect(icon).toHaveCSS("border-top-width", "0px");
    await expect(page.locator(".marker-detail-icon")).toHaveCSS("color", color);
    await page.getByRole("button", { name: "Close marker details" }).click();
    await expect(pin).not.toHaveClass(/is-selected/);
    await expect(icon.locator("path")).toHaveCSS("fill", color);
    await expect(icon).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
    await expect(icon).toHaveCSS("border-top-width", "0px");
    await page.screenshot({ path: testInfo.outputPath(`${symbol}.png`) });
    await pin.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("button", { name: "Close marker details" })).toBeVisible();
    const details = page.getByRole("dialog");
    await details.getByRole("button", { name: "Delete", exact: true }).click();
    await details.getByRole("button", { name: "Delete", exact: true }).click();
  }
});
