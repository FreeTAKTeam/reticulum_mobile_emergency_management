import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";
const fixture = JSON.parse(readFileSync("e2e/fixtures/r3s-native.json", "utf8"));
const style = { version: 8, glyphs: "https://demotiles.maplibre.org/font/{fontstack}/{range}.pbf", sources: {}, layers: [{id:"background",type:"background",paint:{"background-color":"#202a30"}}] };

test("native origin sensors render, hide, survive style reset and suspend on failure", async ({ page }, info) => {
  await page.route("https://tiles.openfreemap.org/styles/liberty*", r => r.fulfill({json:style}));
  await seedAppStorage(page, {settings:defaultSettings});
  await gotoApp(page, "/dashboard");
  await page.evaluate(async (data) => {
    const { useNodeStore } = await import("/src/stores/nodeStore.ts");
    const client = await useNodeStore().sensorGeometryClient();
    Object.assign(window, {r3sFixture:data, r3sPhase:"fresh"});
    client.getSensorOrigins = async () => {
      const w = window as any;
      if (w.r3sPhase === "failed") throw new Error("Native projection unavailable");
      return w.r3sFixture[w.r3sPhase];
    };
  }, fixture);
  await page.getByRole("link", {name:"Map",exact:true}).click();
  const panel = page.getByTestId("sensor-geometry-panel");
  await panel.locator("summary").click();
  await expect(panel.getByText("Drone B", {exact:true})).toBeVisible();
  await expect(panel.locator("article")).toHaveCount(5);
  await panel.getByRole("button", {name:"Locate"}).last().click();
  const features = () => panel.evaluate((el:any) => {
    const map = el.__vueParentComponent.props.map;
    return (map.getStyle().sources["r3s-sensor-geometry"] as any)?.data?.features ?? [];
  });
  await expect.poll(async () => (await features()).filter((f:any)=>f.geometry.type==="MultiPolygon").length).toBe(5);
  await expect.poll(async () => page.locator(".maplibregl-canvas").count()).toBe(1);
  await panel.locator("section").last().getByRole("checkbox").first().uncheck();
  await expect.poll(async () => (await features()).filter((f:any)=>f.geometry.type==="MultiPolygon").length).toBe(1);
  await panel.locator("section").last().getByRole("checkbox").first().check();
  await panel.evaluate((el:any, nextStyle) => el.__vueParentComponent.props.map.setStyle(nextStyle), style);
  await expect.poll(async () => (await features()).filter((f:any)=>f.geometry.type==="MultiPolygon").length).toBe(5);
  await page.waitForTimeout(500);
  await page.screenshot({path:info.outputPath("rem-r3s-map.png")});
  await page.evaluate(() => { (window as any).r3sPhase = "stale"; });
  await expect(panel.getByText(/Stale ·/)).toHaveCount(5);
  await expect.poll(async () => (await features()).filter((f:any)=>f.geometry.type==="MultiPolygon").length).toBe(0);
  await page.evaluate(() => { (window as any).r3sPhase = "failed"; });
  await expect(panel.getByRole("alert")).toContainText("Native projection unavailable");
  await expect(panel.locator("article")).toHaveCount(0);
  await expect.poll(async () => (await features()).length).toBe(0);
});
