import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

test("renders dashboard readiness metrics from stored action messages", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
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
    events: [
      {
        uid: "event-1",
        entryUid: "event-1",
        missionUid: "mission",
        callsign: "Alpha-1",
        summary: "Checkpoint updated",
        content: "Checkpoint updated",
        updatedAt: 1_710_000_000_800,
      },
    ],
    inboxMessages: [
      {
        messageIdHex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        conversationId: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        direction: "Inbound",
        destinationHex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        sourceHex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        bodyUtf8: "Inbound chat",
        method: "Direct",
        state: "Received",
        receivedAtMs: 1_710_000_000_900,
        updatedAtMs: 1_710_000_000_900,
      },
    ],
    notificationActivities: [
      {
        id: 701,
        title: "Inbound chat from Alpha-1",
        body: "Inbound chat",
        at: 1_710_000_001_000,
        route: "/inbox",
        conversationId: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        messageIdHex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      },
      {
        id: 702,
        title: "Checklist updated",
        body: "Medical supply checklist changed",
        at: 1_710_000_000_700,
        route: "/checklists/checklist-1",
      },
    ],
  });

  await gotoApp(page, "/dashboard");

  await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible();
  await expect(page.locator(".announce-count-chip")).toHaveCount(0);
  await expect(page.locator(".kpi-grid")).toHaveCount(0);
  await expect(page.locator(".header-actions").getByRole("button", { name: "Announce" })).toBeVisible();

  const panelHeadings = await page.locator(".panel h2").evaluateAll((headings) =>
    headings.map((heading) => heading.textContent?.trim() ?? ""),
  );
  expect(panelHeadings.indexOf("Activity")).toBeLessThan(panelHeadings.indexOf("Checklists"));
  await expect(page.getByRole("link", { name: "Open Threads" })).toHaveCount(0);

  await expect(page.locator(".activity-grid")).toContainText("2");
  await expect(page.locator(".activity-grid")).toContainText("EAM");
  await expect(page.locator(".activity-grid")).toContainText("1");
  await expect(page.locator(".activity-grid")).toContainText("Threads");
  await expect(page.getByRole("link", { name: /Inbound chat from Alpha-1/ })).toHaveAttribute(
    "href",
    "/inbox?conversation=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb&message=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  );
  await expect(page.getByRole("link", { name: /Checklist updated/ })).toHaveAttribute(
    "href",
    "/checklists/checklist-1",
  );

  const securityCard = page.locator(".ring-card").filter({ hasText: "Security" });
  await expect(securityCard).toHaveAttribute("href", "/messages");
  await expect(securityCard).toContainText("63%");
  await expect(securityCard).not.toContainText("Yellow");

  const capabilityCard = page.locator(".ring-card").filter({ hasText: "Capability" });
  await expect(capabilityCard).toContainText("75%");
  await expect(capabilityCard).not.toContainText("Green");

  const commsCard = page.locator(".ring-card").filter({ hasText: "Comms" });
  await expect(commsCard).toContainText("38%");
  await expect(commsCard).not.toContainText("Orange");
});
