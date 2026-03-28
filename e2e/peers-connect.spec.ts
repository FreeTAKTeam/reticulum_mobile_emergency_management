import { expect, test, type Page } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

const TEST_PEER_DESTINATION = "1234567890abcdef1234567890abcdef";

async function savePeerViaStore(page: Page, destination: string): Promise<void> {
  await page.evaluate(async (peerDestination) => {
    const mod = await import("/src/stores/nodeStore.ts");
    const store = mod.useNodeStore();
    await store.savePeer(peerDestination);
  }, destination);
}

async function stopNodeViaStore(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    const store = mod.useNodeStore();
    await store.stopNode();
  });
}

test("connect all dispatches for saved peers without an active link", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      autoConnectSaved: false,
    },
  });

  await gotoApp(page, "/peers");
  await savePeerViaStore(page, TEST_PEER_DESTINATION);

  const savedItem = page.locator(".saved-item").filter({ hasText: TEST_PEER_DESTINATION });
  await expect(savedItem).toContainText("Disconnected");

  await page.getByRole("button", { name: "Connect all", exact: true }).click();
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

  const savedItem = page.locator(".saved-item").filter({ hasText: TEST_PEER_DESTINATION });
  await expect(savedItem.getByRole("button", { name: "Connect" })).toBeVisible();
  await expect(savedItem).toContainText("Disconnected");

  await savedItem.getByRole("button", { name: "Connect" }).click();
  await expect(savedItem).toContainText("Connected", { timeout: 5_000 });
  await expect(savedItem.getByRole("button", { name: "Disconnect" })).toBeVisible();

  await stopNodeViaStore(page);
  await expect(savedItem).toContainText("Disconnected", { timeout: 5_000 });

  await savedItem.getByRole("button", { name: "Connect" }).click();
  await expect(page.locator(".feedback").last()).toContainText("Start node before connecting to a peer.");
  await expect(savedItem.getByRole("button", { name: "Connect" })).toBeVisible();
});
