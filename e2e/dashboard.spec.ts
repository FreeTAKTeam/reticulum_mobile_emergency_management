import { expect, test } from "@playwright/test";

import { gotoApp, seedAppStorage } from "./support/app";

test("renders dashboard readiness metrics from stored action messages", async ({ page }) => {
  await seedAppStorage(page, {
    messages: [
      {
        callsign: "Alpha-1",
        groupName: "BLUE",
        securityStatus: "Green",
        capabilityStatus: "Yellow",
        preparednessStatus: "Red",
        medicalStatus: "Unknown",
        mobilityStatus: "Green",
        commsStatus: "Yellow",
        updatedAt: 1_710_000_000_000,
      },
      {
        callsign: "Bravo-2",
        groupName: "RED",
        securityStatus: "Red",
        capabilityStatus: "Green",
        preparednessStatus: "Green",
        medicalStatus: "Yellow",
        mobilityStatus: "Unknown",
        commsStatus: "Red",
        updatedAt: 1_710_000_000_500,
      },
    ],
  });

  await gotoApp(page, "/dashboard");

  await expect(page.getByRole("heading", { name: "Emergency Ops Dashboard" })).toBeVisible();
  await expect(page.getByText("# 2 MSG")).toBeVisible();

  const securityCard = page.locator(".ring-card").filter({ hasText: "Security" });
  await expect(securityCard).toContainText("63%");
  await expect(securityCard).toContainText("Yellow");

  const capabilityCard = page.locator(".ring-card").filter({ hasText: "Capability" });
  await expect(capabilityCard).toContainText("75%");
  await expect(capabilityCard).toContainText("Green");

  const commsCard = page.locator(".ring-card").filter({ hasText: "Comms" });
  await expect(commsCard).toContainText("38%");
  await expect(commsCard).toContainText("Orange");
});
