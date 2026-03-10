import { expect, test } from "@playwright/test";

import { gotoApp, seedAppStorage } from "./support/app";

test("operators can create and remove event timeline entries", async ({ page }) => {
  await seedAppStorage(page);
  await gotoApp(page, "/events");

  await page.getByRole("button", { name: "Add event", exact: true }).click();

  const createForm = page.locator("form.create-form");
  await createForm.getByLabel("Call Sign").fill("Echo-1");
  await createForm.getByLabel("Type").fill("Logistics");
  await createForm.getByLabel("Event summary").fill("Bridge closed near rally point");
  await createForm.getByRole("button", { name: "Add event" }).click();

  await expect(page.getByRole("heading", { name: "Bridge closed near rally point" })).toBeVisible();
  await expect(page.getByText("Logistics")).toBeVisible();

  await page.getByRole("button", { name: "Delete Echo-1" }).click();
  await expect(page.getByText("No events yet. Add one locally or wait for a peer snapshot.")).toBeVisible();
});
