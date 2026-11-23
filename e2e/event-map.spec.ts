import { expect, test } from "@playwright/test";
import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

const style = { version: 8, glyphs: "https://demotiles.maplibre.org/font/{fontstack}/{range}.pbf", sources: {}, layers: [{ id: "background", type: "background", paint: { "background-color": "#202a30" } }] };

test("events with a position are drawn on the map layer", async ({ page }, info) => {
  await page.route("https://tiles.openfreemap.org/styles/liberty*", (r) => r.fulfill({ json: style }));
  const now = Date.now();
  await seedAppStorage(page, {
    settings: defaultSettings,
    events: [
      // Hub position wins over the MECP coordinates in the text.
      { uid: "evt-hub", callsign: "R4", updatedAt: now,
        args: { content: "MECP/0/M01 40.000000,-75.000000", latitude: 44, longitude: -63 } },
      { uid: "evt-mecp", callsign: "R5", summary: "MECP/2/T01 41.000000,-74.000000", updatedAt: now },
      { uid: "evt-none", callsign: "R6", summary: "Field report", updatedAt: now },
      { uid: "evt-deleted", callsign: "R7", summary: "MECP/1/S02 42.000000,-73.000000", updatedAt: now, deleted_at: now },
    ],
  });
  await gotoApp(page, "/dashboard");
  await page.getByRole("link", { name: "Map", exact: true }).click();
  const markers = page.locator(".maplibregl-marker.event-map-marker");
  await expect(markers).toHaveCount(2);
  // Mayday is red, Safety is green; each carries its category icon, not a dot.
  const hub = page.locator('[data-event-uid="evt-hub"]');
  await expect(hub).toHaveClass(/is-red/);
  await expect(page.locator('[data-event-uid="evt-mecp"]')).toHaveClass(/is-green/);
  await expect(hub.locator("svg")).toBeVisible();
  const positions = await page.getByTestId("event-map-layer").evaluate((el: any) => {
    const map = el.__vueParentComponent.props.map;
    map.jumpTo({ center: [-63, 44], zoom: 8 });
    const at = (uid: string) => document.querySelector(`[data-event-uid="${uid}"]`)!.getBoundingClientRect();
    const box = map.getCanvas().getBoundingClientRect();
    const centre = (r: DOMRect) => map.unproject([r.x + r.width / 2 - box.left, r.y + r.height / 2 - box.top]);
    return { hub: centre(at("evt-hub")), mecp: centre(at("evt-mecp")) };
  });
  expect(positions.hub.lng).toBeCloseTo(-63, 1);
  expect(positions.hub.lat).toBeCloseTo(44, 1);
  expect(positions.mecp.lng).toBeCloseTo(-74, 1);
  expect(positions.mecp.lat).toBeCloseTo(41, 1);
  await hub.click();
  const popup = page.locator(".event-map-popup");
  await expect(popup).toContainText("injury");
  await expect(popup).toContainText("Mayday");
  await expect(popup).toContainText("R4");
  await page.screenshot({ path: info.outputPath("rem-event-map.png") });
  await popup.getByRole("button", { name: "Open event" }).click();
  await expect(page).toHaveURL(/\/events\?event=evt-hub/);
  await expect(page.locator('[data-event-uid="evt-hub"].event-focused')).toBeVisible();
});
