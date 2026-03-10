import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

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
