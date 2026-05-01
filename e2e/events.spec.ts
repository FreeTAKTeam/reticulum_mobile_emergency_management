import { expect, test } from "@playwright/test";

import { gotoApp, seedAppStorage } from "./support/app";

test("operators can create and remove event timeline entries", async ({ page }) => {
  await seedAppStorage(page);
  await gotoApp(page, "/events");

  await page.getByRole("button", { name: "Add event", exact: true }).click();

  const createForm = page.locator("form.create-form");
  await expect(createForm.getByLabel("Configured call sign")).toHaveValue("emergency-ops-mobile");
  await createForm.getByLabel("Type").fill("Logistics");
  await createForm.getByLabel("Event summary").fill("Bridge closed near rally point");
  await createForm.getByRole("button", { name: "Add event" }).click();

  await expect(page.getByRole("heading", { name: "Bridge closed near rally point" })).toBeVisible();
  await expect(page.getByText("Logistics")).toBeVisible();
  await expect(page.getByText(/emergency-ops-mobile \|/)).toBeVisible();

  await page.getByRole("button", { name: "Delete emergency-ops-mobile" }).click();
  await expect(page.getByText("No events yet. Add one locally or wait for a peer snapshot.")).toBeVisible();
});

test("header shows the connected peer count", async ({ page }) => {
  await seedAppStorage(page, {
    savedPeers: [
      {
        destination: "c3d4f7a6e01944ef8e620f5c5a146f1a",
        label: "Relay Alpha",
        savedAt: Date.now(),
      },
    ],
  });
  await gotoApp(page, "/peers");

  const connectedPeerCount = page.getByTestId("connected-peer-count");

  await expect(page.getByRole("heading", { name: "Peers" })).toBeVisible();
  await expect(page.locator(".rows .row").first()).toBeVisible();
  await expect(connectedPeerCount).toHaveText("1/0");

  await page.locator(".rows .row").first().getByRole("button", { name: "Connect" }).click();
  await expect(connectedPeerCount).toHaveText("1/1");

  await page.locator(".rows .row").first().getByRole("button", { name: "Disconnect" }).click();
  await expect(connectedPeerCount).toHaveText("1/0");
});
