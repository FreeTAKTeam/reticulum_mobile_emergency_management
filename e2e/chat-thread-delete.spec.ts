import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

test("operators can delete a chat thread from the conversation list", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await gotoApp(page, "/inbox?mockChat=1");
  await expect(page.getByRole("heading", { name: "Chat" })).toBeVisible();
  await expect(page.getByText("3 Threads")).toBeVisible();

  page.on("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Delete conversation with ALPHA-1" }).click();

  await expect(page.getByText("2 Threads")).toBeVisible();
  await expect(page.getByRole("button", { name: /ALPHA-1/ })).toHaveCount(0);
  await expect(page.getByRole("button", { name: /^TRIAGE-2 / })).toBeVisible();
});
