import { expect, test, type Page } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

const TEST_PEER_DESTINATION = "1234567890abcdef1234567890abcdef";
const RED_TEAM_UID = "65ce79a3a3e4b51ec0ec52d1d3d2b0b9";

async function savePeerViaStore(page: Page, destination: string): Promise<void> {
  await page.evaluate(async (peerDestination) => {
    const mod = await import("/src/stores/nodeStore.ts");
    const store = mod.useNodeStore();
    await store.savePeer(peerDestination);
  }, destination);
  await page.locator(".peer-tabs").getByRole("button", { name: /Peers/ }).click();
}

async function stopNodeViaStore(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    const store = mod.useNodeStore();
    await store.stopNode();
  });
}

test("active-team roster connects saved peers without an active link", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      autoConnectSaved: false,
    },
  });

  await gotoApp(page, "/peers");
  await savePeerViaStore(page, TEST_PEER_DESTINATION);

  const savedItem = page.locator(".roster-row").filter({ hasText: TEST_PEER_DESTINATION.slice(0, 8) });
  await expect(savedItem).toContainText("Offline");

  await savedItem.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(savedItem).toContainText("Connected", { timeout: 5_000 });
  await expect(savedItem.getByRole("button", { name: "Disconnect" })).toBeVisible();
});

test("manual connect uses the saved-peer button and surfaces node-not-running errors", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      autoConnectSaved: false,
    },
  });

  await gotoApp(page, "/peers");
  await savePeerViaStore(page, TEST_PEER_DESTINATION);

  const savedItem = page.locator(".roster-row").filter({ hasText: TEST_PEER_DESTINATION.slice(0, 8) });
  await expect(savedItem.getByRole("button", { name: "Connect" })).toBeVisible();
  await expect(savedItem).toContainText("Offline");

  await savedItem.getByRole("button", { name: "Connect" }).click();
  await expect(savedItem).toContainText("Connected", { timeout: 5_000 });
  await expect(savedItem.getByRole("button", { name: "Disconnect" })).toBeVisible();

  await stopNodeViaStore(page);
  await expect(savedItem.getByRole("button", { name: "Connect" })).toBeVisible({ timeout: 5_000 });

  await savedItem.getByRole("button", { name: "Connect" }).click();
  await expect.poll(async () =>
    page.evaluate(async () => {
      const mod = await import("/src/stores/nodeStore.ts");
      return mod.useNodeStore().lastError;
    }),
  ).toContain("Start node before connecting to a peer.");
  await expect(savedItem.getByRole("button", { name: "Connect" })).toBeVisible();
});

test("active team switcher and Manage Teams preserve read-only RCH membership with local aliases", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/peers?mockPeers=1");
  const activeTeamMenu = page.locator("summary[aria-label='Active team']");
  await expect(activeTeamMenu).toContainText("Yellow");
  await activeTeamMenu.click();
  await page.getByRole("menuitemradio", { name: /Blue/ }).click();
  await expect(activeTeamMenu).toContainText("Blue");

  await page.getByRole("button", { name: "Manage teams" }).click();
  await expect(page).toHaveURL(/\/settings\/teams/);
  const blueRow = page.locator(".directory-row").filter({ hasText: "Blue" });
  await expect(blueRow).toContainText(/read only/i);
  await blueRow.locator(".team-row-main").click();

  const blueDialog = page.getByRole("dialog", { name: "Manage Blue" });
  const aliasInput = blueDialog.getByLabel("Local alias");
  await aliasInput.fill("Medical");
  await blueDialog.getByRole("button", { name: "Save", exact: true }).click();
  const medicalDialog = page.getByRole("dialog", { name: "Manage Medical" });
  await expect(medicalDialog).toContainText("Medical");
  await expect.poll(() => page.evaluate(() => {
    const raw = window.localStorage.getItem("reticulum.mobile.settings.v1");
    return raw ? JSON.parse(raw).teams : null;
  })).toMatchObject({
    activeTeamUid: "43341e5c822d99857fa6e8641f2ca9c0",
    aliases: [{ teamUid: "43341e5c822d99857fa6e8641f2ca9c0", alias: "Medical" }],
  });
  await medicalDialog.getByRole("button", { name: "Close team details" }).click();

  await page.reload();
  await expect.poll(() => page.evaluate(() => {
    const raw = window.localStorage.getItem("reticulum.mobile.settings.v1");
    return raw ? JSON.parse(raw).teams : null;
  })).toMatchObject({
    activeTeamUid: "43341e5c822d99857fa6e8641f2ca9c0",
    aliases: [{ teamUid: "43341e5c822d99857fa6e8641f2ca9c0", alias: "Medical" }],
  });
  await expect.poll(() => page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    return mod.useNodeStore().settings.teams;
  })).toMatchObject({
    activeTeamUid: "43341e5c822d99857fa6e8641f2ca9c0",
    aliases: [{ teamUid: "43341e5c822d99857fa6e8641f2ca9c0", alias: "Medical" }],
  });
  await page.goto("/peers?mockPeers=1");
  await expect(page.locator("summary[aria-label='Active team']")).toContainText("Medical");
});

test("creates a local color team and assigns a saved peer without removing Yellow membership", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/peers");
  await savePeerViaStore(page, TEST_PEER_DESTINATION);

  await page.getByRole("button", { name: "Manage teams" }).click();
  await page.getByRole("button", { name: "Add team", exact: true }).click();
  const addDialog = page.getByRole("dialog", { name: "Add local team" });
  await addDialog.getByLabel("New local team color").selectOption(RED_TEAM_UID);
  await addDialog.getByLabel("Local name").fill("Friends");
  await addDialog.getByRole("button", { name: "Create team" }).click();

  const redDialog = page.getByRole("dialog", { name: "Manage Friends" });
  await redDialog.getByLabel("Add saved peer to Friends").selectOption(TEST_PEER_DESTINATION);
  await redDialog.getByRole("button", { name: "Add", exact: true }).click();
  await expect(redDialog).toContainText(TEST_PEER_DESTINATION);
  await expect(redDialog).toContainText("LOCAL");

  await redDialog.getByRole("button", { name: "Share QR" }).click();
  const qrDialog = page.getByRole("dialog", { name: "Friends team QR code" });
  await expect(qrDialog.getByRole("img", { name: "Friends local team QR code" }))
    .toHaveAttribute("src", /^data:image\/png;base64,/);
  await expect(qrDialog).toContainText("Local aliases and peer labels are not included.");
  await qrDialog.getByRole("button", { name: "Close QR" }).click();
  await expect(qrDialog).toBeHidden();
  await page.getByRole("button", { name: "Close team details" }).click();
  await page.getByRole("button", { name: "Back to peers" }).click();
  const activeTeamMenu = page.locator("summary[aria-label='Active team']");
  await activeTeamMenu.click();
  await page.getByRole("menuitemradio", { name: /Red · Friends/ }).click();
  await expect(activeTeamMenu).toContainText("Red · Friends");
});
