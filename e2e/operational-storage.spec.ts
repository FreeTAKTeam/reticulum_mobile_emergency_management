import { expect, test } from "@playwright/test";
import { gotoApp, seedAppStorage } from "./support/app";

test("browser operational projections avoid durable caches and expire on reload", async ({ page }) => {
  await seedAppStorage(page);
  await gotoApp(page, "/events");
  const outcome = await page.evaluate(async () => {
    const path = "/src/services/operationalStorage.ts";
    const { operationalStorage } = await import(/* @vite-ignore */ path);
    const retired = "reticulum.mobile.events.v1";
    localStorage.setItem(retired, "retired-cache");
    const ignoredRetiredCache = operationalStorage.getItem(retired);
    for (const key of [retired, "reticulum.mobile.messages.v1", "reticulum.mobile.inbox.v1",
      "reticulum.mobile.telemetry.v1", "reticulum.mobile.notificationActivity.v1"]) {
      operationalStorage.setItem(key, "new sensitive value");
    }
    return { ignoredRetiredCache, retained: localStorage.getItem(retired),
      durableInbox: localStorage.getItem("reticulum.mobile.inbox.v1") };
  });
  expect(outcome).toEqual({ ignoredRetiredCache: null, retained: "retired-cache", durableInbox: null });
  await page.reload();
  expect(await page.evaluate(async () => {
    const path = "/src/services/operationalStorage.ts";
    const { operationalStorage } = await import(/* @vite-ignore */ path);
    return operationalStorage.getItem("reticulum.mobile.inbox.v1");
  })).toBeNull();
});
