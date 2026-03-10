import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

test("operators can create, edit, view help, and delete an action message", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      displayName: "Raven-6",
    },
  });

  await gotoApp(page, "/messages");

  await page.getByRole("button", { name: "Add message", exact: true }).click();

  const createForm = page.locator("form.create-form");
  await expect(createForm.getByLabel("Call Sign")).toHaveValue("Raven-6");
  await createForm.getByLabel("Call Sign").fill("Bravo-2");
  await createForm.getByLabel("Team color").selectOption("RED");
  await createForm.getByRole("button", { name: "Add message" }).click();

  const messageCard = page.locator("article.item:visible").filter({ hasText: "Bravo-2" });
  await expect(messageCard.getByRole("heading", { name: "Bravo-2" })).toBeVisible();
  await expect(messageCard).toContainText("Team: Red");

  page.once("dialog", (dialog) => dialog.accept("BLUE"));
  await messageCard.getByRole("button", { name: "Edit Bravo-2" }).click();
  await expect(messageCard).toContainText("Team: Blue");

  await page.getByRole("button", { name: "Open status color help" }).click();
  await expect(page).toHaveURL(/\/messages\/help$/);
  await expect(page.getByRole("heading", { name: "Help - Status Color Indicators" })).toBeVisible();

  await page.getByRole("link", { name: "Back to Messages" }).click();
  await messageCard.getByRole("button", { name: "Delete Bravo-2" }).click();
  await expect(page.getByRole("heading", { name: "Bravo-2" })).toHaveCount(0);
});
