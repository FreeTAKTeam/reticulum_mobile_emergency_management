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
  const layer = page.getByTestId("event-map-layer");
  const features = () => layer.evaluate((el: any) =>
    (el.__vueParentComponent.props.map.getStyle().sources["rem-events"] as any)?.data?.features ?? []);
  await expect.poll(async () => (await features()).length).toBe(2);
  const byCallsign = Object.fromEntries((await features()).map((f: any) => [f.properties.callsign, f]));
  expect(byCallsign.R4.geometry.coordinates).toEqual([-63, 44]);
  expect(byCallsign.R4.properties.severity).toBe("red");
  expect(byCallsign.R5.geometry.coordinates).toEqual([-74, 41]);
  expect(byCallsign.R5.properties.severity).toBe("green");
  const point = await layer.evaluate((el: any) => {
    const map = el.__vueParentComponent.props.map;
    map.jumpTo({ center: [-63, 44], zoom: 8 });
    const { x, y } = map.project([-63, 44]);
    const box = map.getCanvas().getBoundingClientRect();
    return { x: box.left + x, y: box.top + y };
  });
  await expect.poll(() => layer.evaluate((el: any) =>
    el.__vueParentComponent.props.map.queryRenderedFeatures({ layers: ["rem-events-circle"] }).length)).toBeGreaterThan(0);
  await page.mouse.click(point.x, point.y);
  const popup = page.locator(".event-map-popup");
  await expect(popup).toContainText("Medical");
  await expect(popup).toContainText("R4");
  await expect(popup).toContainText("injury");
  await page.screenshot({ path: info.outputPath("rem-event-map.png") });
});
