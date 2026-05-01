import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

test("operators can create, edit statuses, cycle status, view help, and delete an action message", async ({ page }) => {
  const updatedAtFor = async (callsign: string): Promise<number> =>
    page.evaluate((targetCallsign) => {
      const raw = globalThis.localStorage.getItem("reticulum.mobile.messages.v1");
      if (!raw) {
        return 0;
      }
      const entries = JSON.parse(raw) as Record<string, { callsign?: string; updatedAt?: number }>;
      const match = Object.values(entries).find((entry) => entry.callsign === targetCallsign);
      return typeof match?.updatedAt === "number" ? match.updatedAt : 0;
    }, callsign);

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
  const createdUpdatedAt = await updatedAtFor("Bravo-2");
  expect(createdUpdatedAt).toBeGreaterThan(0);

  await messageCard.getByRole("button", { name: "Edit Bravo-2" }).click();
  await expect(createForm.getByLabel("Call Sign")).toHaveValue("Bravo-2");
  await createForm.getByLabel("Team color").selectOption("BLUE");
  await createForm.getByLabel("Security status").selectOption("Yellow");
  await createForm.getByLabel("Comms status").selectOption("Red");
  await createForm.getByRole("button", { name: "Save message" }).click();
  await expect(messageCard).toContainText("Team: Blue");
  const editedUpdatedAt = await updatedAtFor("Bravo-2");
  expect(editedUpdatedAt).toBeGreaterThan(createdUpdatedAt);
  await messageCard.getByRole("button", { name: "Show statuses" }).click();
  await expect(messageCard.locator(".pill-button").filter({ hasText: "Security" })).toContainText("Yellow");
  await expect(messageCard.locator(".pill-button").filter({ hasText: "Comms" })).toContainText("Red");

  const securityStatusButton = messageCard.locator(".pill-button").filter({ hasText: "Security" });
  await securityStatusButton.click();
  await expect(securityStatusButton).toContainText("Red");
  const rotatedUpdatedAt = await updatedAtFor("Bravo-2");
  expect(rotatedUpdatedAt).toBeGreaterThan(editedUpdatedAt);

  await page.getByRole("link", { name: "Open status color help" }).click();
  await expect(page).toHaveURL(/\/messages\/help$/);
  await expect(page.getByRole("heading", { name: "Status Help" })).toBeVisible();

  await page.getByRole("link", { name: "Messages" }).click();
  await messageCard.getByRole("button", { name: "Delete Bravo-2" }).click();
  await expect(page.getByRole("heading", { name: "Bravo-2" })).toHaveCount(0);
});

test("synced inbound action messages are visible but read-only", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      displayName: "Poco",
    },
    messages: [
      {
        callsign: "Pixel",
        groupName: "YELLOW",
        securityStatus: "Red",
        capabilityStatus: "Red",
        preparednessStatus: "Red",
        medicalStatus: "Red",
        mobilityStatus: "Red",
        commsStatus: "Red",
        updatedAt: Date.UTC(2026, 2, 26, 13, 33, 0),
        reportedBy: "Pixel",
        teamMemberUid: "f7a7c54f4e0fb481b73b990b843277df",
        teamUid: "d6b6e188b910d6bdd24d04b7a7ec5444",
        source: {
          rns_identity: "84c51f6385d01217f56b6f36dae81e95",
          display_name: "Pixel",
        },
        syncState: "synced",
        lastSyncedAt: Date.UTC(2026, 2, 26, 13, 33, 9),
      },
    ],
  });

  await gotoApp(page, "/messages");

  const messageCard = page.locator("article.item:visible").filter({ hasText: "Pixel" });
  await expect(messageCard.getByRole("heading", { name: "Pixel" })).toBeVisible();
  await expect(messageCard).toContainText("Synced");
  await expect(messageCard).toContainText("Read only");
  await expect(messageCard).toContainText("Reported by Pixel");
  await expect(messageCard).toContainText("Updated");
  await expect(messageCard.getByRole("button", { name: "Edit Pixel" })).toHaveCount(0);
  await expect(messageCard.getByRole("button", { name: "Delete Pixel" })).toHaveCount(0);

  await messageCard.getByRole("button", { name: "Show statuses" }).click();
  const securityStatusButton = messageCard.locator(".pill-button").filter({ hasText: "Security" });
  await expect(securityStatusButton).toBeDisabled();
  await expect(securityStatusButton).toContainText("Red");
});
