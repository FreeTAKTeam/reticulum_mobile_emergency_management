import { expect, test } from "@playwright/test";

import { defaultSettings, seedAppStorage } from "./support/app";

test("shows the REM logo and version during startup", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await page.goto("/dashboard");

  const splash = page.getByTestId("splash-screen");
  await expect(splash).toBeVisible();
  await expect(splash.getByRole("img", { name: "R.E.M. logo" })).toBeVisible();
  await expect(splash.getByText("Version 1.0.10")).toBeVisible();

  await expect(splash).toBeHidden({ timeout: 5000 });
  await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible();
});
