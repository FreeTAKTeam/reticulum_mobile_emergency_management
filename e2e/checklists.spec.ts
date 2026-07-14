import { expect, test } from "@playwright/test";

import { gotoApp, seedAppStorage } from "./support/app";

test("operators can create a checklist and open its task detail", async ({ page }) => {
  await seedAppStorage(page);
  await gotoApp(page, "/checklists");

  await page.getByRole("button", { name: "Create checklist" }).click();
  await page.getByLabel("Checklist title").fill("Field readiness");
  await page.getByLabel("Checklist template").selectOption({ index: 1 });
  await page.locator(".create-submit").click();

  const openChecklist = page.getByRole("button", { name: "Open Field readiness" }).first();
  await expect(openChecklist).toBeVisible();
  await openChecklist.click();

  await expect(page).toHaveURL(/\/checklists\/chk-web-/);
  await expect(page.getByRole("heading", { name: "Field readiness" })).toBeVisible();
  await expect(page.locator(".task-card")).not.toHaveCount(0);
});
