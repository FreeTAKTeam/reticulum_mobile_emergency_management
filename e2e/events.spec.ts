import { expect, test } from "@playwright/test";

import {
  decodeMecpMessage,
  encodeMecpMessage,
  parseMecpMessage,
} from "../apps/mobile/src/utils/mecp";
import { gotoApp, seedAppStorage } from "./support/app";

const GREEK_CALLSIGN_PATTERN = /^(Alpha|Beta|Gamma|Delta|Epsilon|Zeta|Eta|Theta|Iota|Kappa|Lambda|Mu|Nu|Xi|Omicron|Pi|Rho|Sigma|Tau|Upsilon|Phi|Chi|Psi|Omega)\d{3}$/;

test("MECP utilities encode and parse compact event bodies", () => {
  const message = encodeMecpMessage({
    severity: 2,
    codes: ["T01"],
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
  expect(parseMecpMessage("MECP/2/bridge closed")).toMatchObject({
    valid: false,
    codes: [],
    category: null,
  });
  expect(parseMecpMessage("MECP/2/")).toMatchObject({
    valid: false,
    codes: [],
    category: null,
  });
});

test("MECP utilities encode and decode structured protocol details", () => {
  const message = encodeMecpMessage({
    severity: 1,
    codes: ["R03", "T99"],
    details: "north gate",
    extras: {
      callsign: "EAGLE-1",
      coordinates: { latitude: 45.5017, longitude: -73.5673 },
      etaMinutes: 15,
      language: "EN",
      pax: 4,
      references: ["A1"],
      timestamp: "0930",
    },
  });

  expect(message).toBe("MECP/1/R03 T99 4pax 45.5017,-73.5673 #A1 15 @en north gate");
  expect(message).not.toContain("EAGLE-1");
  expect(message).not.toContain("@0930");

  const decoded = decodeMecpMessage("MECP/1/R03 T99 4pax 45.5017,-73.5673 #A1 15 @en @0930 ~EAGLE-1 north gate");
  expect(decoded).toMatchObject({
    valid: true,
    severity: 1,
    category: "R",
    codes: ["R03", "T99"],
    details: "4pax 45.5017,-73.5673 #A1 15 @en @0930 ~EAGLE-1 north gate",
    extras: {
      callsign: "EAGLE-1",
      etaMinutes: 15,
      language: "en",
      pax: 4,
      references: ["#A1"],
      timestamp: "0930",
    },
  });
  expect(decoded.extras.coordinates).toEqual({ latitude: 45.5017, longitude: -73.5673 });
  expect(decoded.warnings).toContain('Unknown MECP event code "T99".');
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
  await expect(createForm.getByRole("button", { name: /Position \/ Movement/ })).toBeVisible();
  await expect(createForm.getByRole("button", { name: /P01 Stranded \/ stuck/ })).toBeVisible();
  await expect(createForm.getByText("MECP/2/P01")).toBeVisible();
  await createForm.getByLabel("MECP reference").fill("A1");
  await createForm.getByLabel("MECP GPS coordinates").fill("45.5017,-73.5673");
  await createForm.getByLabel("Optional details").fill("north gate");

  await createForm.getByRole("button", { name: "Add event" }).click();

  const timelineEvent = page.getByRole("article").filter({ hasText: "MECP/2/P01 45.5017,-73.5673 #A1 north gate" });
  await expect(timelineEvent.getByRole("heading", { name: "stranded / stuck" })).toBeVisible();
  await expect(timelineEvent.getByText("MECP/2/P01 45.5017,-73.5673 #A1 north gate")).toBeVisible();
  await expect(timelineEvent.getByText("45.50170, -73.56730")).toBeVisible();
  await expect(timelineEvent.getByText("Position / Movement")).toBeVisible();
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
        summary: "MECP/2/T01 C04 #BRAVO 2pax",
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

  await expect(page.getByRole("heading", { name: "road blocked + confirm received" })).toBeVisible();
  await expect(page.getByText("#BRAVO", { exact: true })).toBeVisible();
  await expect(page.getByText("2 pax")).toBeVisible();
  await expect(page.getByRole("heading", { name: "storm approaching" })).toBeVisible();

  await page.getByRole("button", { name: "Event filter status" }).click();
  await page.getByLabel("Filter by severity").selectOption("Mayday");
  await expect(page.getByRole("heading", { name: "storm approaching" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "road blocked + confirm received" })).toBeHidden();

  await page.getByLabel("Filter by severity").selectOption("All");
  await page.getByLabel("Filter by category").selectOption("T");
  await expect(page.getByRole("heading", { name: "road blocked + confirm received" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "storm approaching" })).toBeHidden();

  await page.getByRole("button", { name: "Reset" }).click();
  await expect(page.getByRole("heading", { name: "road blocked + confirm received" })).toBeVisible();
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
