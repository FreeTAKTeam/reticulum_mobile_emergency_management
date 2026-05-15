import { expect, test } from "@playwright/test";

import {
  encodeMecpMessage,
  parseMecpMessage,
} from "../apps/mobile/src/utils/mecp";
import { gotoApp, seedAppStorage } from "./support/app";

const GREEK_CALLSIGN_PATTERN = /^(Alpha|Beta|Gamma|Delta|Epsilon|Zeta|Eta|Theta|Iota|Kappa|Lambda|Mu|Nu|Xi|Omicron|Pi|Rho|Sigma|Tau|Upsilon|Phi|Chi|Psi|Omega)\d{3}$/;

test("MECP utilities encode and parse compact event bodies", () => {
  const message = encodeMecpMessage({
    severity: 2,
    code: "T01",
    details: "#A1",
  });

  expect(message).toBe("MECP/2/T01 #A1");
  expect(parseMecpMessage(message)).toMatchObject({
    valid: true,
    severity: 2,
    category: "T",
    codes: ["T01"],
    details: "#A1",
  });
  expect(parseMecpMessage("Bridge closed near rally point")).toMatchObject({
    valid: false,
    severity: null,
    category: null,
  });
});

test("operators can create and remove MECP event timeline entries", async ({ page }) => {
  await seedAppStorage(page);
  await gotoApp(page, "/events");

  await page.getByRole("button", { name: "Add event", exact: true }).click();

  const createForm = page.locator("form.create-form");
  const callsignInput = createForm.getByLabel("Configured call sign");
  await expect(callsignInput).toHaveValue(GREEK_CALLSIGN_PATTERN);
  const callsign = await callsignInput.inputValue();

  await expect(createForm.getByRole("button", { name: /Severity/ })).toBeVisible();
  await createForm.getByRole("button", { name: /Severity/ }).click();
  await expect(createForm.getByRole("button", { name: /Mayday/ })).toBeVisible();
  await createForm.locator(".severity-menu").getByRole("button", { name: /Urgent/ }).click();
  await createForm.getByRole("button", { name: /Severity Urgent/ }).click();
  await createForm.locator(".severity-menu").getByRole("button", { name: /Safety/ }).click();
  await expect(createForm.getByRole("button", { name: /Terrain \/ Infrastructure/ })).toBeVisible();
  await expect(createForm.getByRole("button", { name: /T01 Road blocked/ })).toBeVisible();
  await createForm.getByLabel("Optional details").fill("#A1");

  await createForm.getByRole("button", { name: "Add event" }).click();

  const timelineEvent = page.getByRole("article").filter({ hasText: "MECP/2/T01 #A1" });
  await expect(timelineEvent.getByRole("heading", { name: "road blocked" })).toBeVisible();
  await expect(timelineEvent.getByText("MECP/2/T01 #A1")).toBeVisible();
  await expect(timelineEvent.getByText("Terrain / Infrastructure")).toBeVisible();
  await expect(page.getByText(new RegExp(`${callsign} \\|`))).toBeVisible();

  await page.getByRole("button", { name: `Delete ${callsign}` }).click();
  await expect(page.getByText("No events yet. Add one locally or wait for a peer snapshot.")).toBeVisible();
});

test("operators can filter MECP events by severity and category", async ({ page }) => {
  const now = Date.now();
  await seedAppStorage(page, {
    events: [
      {
        uid: "evt-safety-road",
        type: "T",
        summary: "MECP/2/T01",
        callsign: "Omega999",
        updatedAt: now,
      },
      {
        uid: "evt-mayday-weather",
        type: "W",
        summary: "MECP/0/W01",
        callsign: "Omega999",
        updatedAt: now + 1,
      },
    ],
  });
  await gotoApp(page, "/events");

  await expect(page.getByRole("heading", { name: "road blocked" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "storm approaching" })).toBeVisible();

  await page.getByRole("button", { name: "Event filter status" }).click();
  await page.getByLabel("Filter by severity").selectOption("Mayday");
  await expect(page.getByRole("heading", { name: "storm approaching" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "road blocked" })).toBeHidden();

  await page.getByLabel("Filter by severity").selectOption("All");
  await page.getByLabel("Filter by category").selectOption("T");
  await expect(page.getByRole("heading", { name: "road blocked" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "storm approaching" })).toBeHidden();

  await page.getByRole("button", { name: "Reset" }).click();
  await expect(page.getByRole("heading", { name: "road blocked" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "storm approaching" })).toBeVisible();
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
