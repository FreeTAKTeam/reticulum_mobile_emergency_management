import { expect, test } from "@playwright/test";
import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

const destination = "77".repeat(16);
const logicalId = "11".repeat(32);
const wireId = "22".repeat(32);

test("chat projects native custody, delivery, and a visible eviction notice", async ({ page }) => {
  const now = Date.now();
  await seedAppStorage(page, {
    settings: defaultSettings,
    savedPeers: [{ destination, savedAt: now, circleTier: "inner" }],
    inboxMessages: [{
      messageIdHex: "44".repeat(32), conversationId: destination, direction: "Inbound",
      sourceHex: destination, destinationHex: destination, bodyUtf8: "Radio check",
      method: "Direct", state: "Received", transportState: "TransportDelivered",
      applicationAckState: "NotRequired", receivedAtMs: now - 1000, updatedAtMs: now - 1000,
    }, {
      messageIdHex: logicalId, lastWireMessageIdHex: wireId, conversationId: destination,
      direction: "Outbound", destinationHex: destination, bodyUtf8: "Offline field report",
      method: "Direct", state: "Queued", transportState: "Queued",
      applicationAckState: "NotRequired", updatedAtMs: now,
    }],
  });
  await gotoApp(page, "/inbox");
  const body = page.getByText("Offline field report", { exact: true });
  if (!await body.isVisible()) await page.getByRole("button", { name: new RegExp(`^${destination}`) }).click();
  await expect(body).toBeVisible();

  async function project(state: "StoredLocally" | "UploadedUnverified" | "Delivered" | "Evicted", uploaded = false): Promise<void> {
    await page.evaluate(async ({ state, uploaded, wireId, destination, now }) => {
      const { useMessagingStore } = await import("/src/stores/messagingStore.ts");
      // A native status fixture; the browser does not implement propagation.
      useMessagingStore().localPropagation = {
        [wireId]: { transientIdHex: "33".repeat(32), messageIdHex: wireId, destinationHex: destination,
          state, encryptedBytes: state === "Evicted" || state === "Delivered" ? 0 : 1024,
          admittedAtMs: now, expiresAtMs: now + 604800000, mayExistOnRelay: uploaded },
      };
    }, { state, uploaded, wireId, destination, now });
  }
  const bubble = page.locator(".bubble", { hasText: "Offline field report" });
  await project("StoredLocally");
  await expect(bubble.locator(".bubble-meta")).toContainText("Stored locally");
  await project("UploadedUnverified", true);
  await expect(bubble.locator(".bubble-meta")).toContainText("Uploaded (unverified)");
  await project("Delivered");
  await expect(bubble.locator(".bubble-meta")).toContainText("Delivered");
  await project("Evicted", true);
  await expect(bubble.locator(".bubble-meta")).toContainText("Evicted from local storage");
  await expect(bubble.getByRole("status")).toHaveText("A copy may remain on the relay after local expiry.");
  await expect(body).toBeVisible();
  await page.screenshot({ path: "test-results/local-propagation-chat.png", fullPage: true });
});
