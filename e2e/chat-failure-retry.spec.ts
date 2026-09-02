import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

const peerDestination = "77777777777777777777777777777777";

test("failed chat text remains visible and can be retried", async ({ page }) => {
  const now = Date.now();
  await seedAppStorage(page, {
    settings: defaultSettings,
    savedPeers: [{ destination: peerDestination, savedAt: now, circleTier: "inner" }],
    inboxMessages: [
      {
        messageIdHex: "inbound-77",
        conversationId: peerDestination,
        direction: "Inbound",
        destinationHex: peerDestination,
        sourceHex: peerDestination,
        bodyUtf8: "Can you receive me?",
        method: "Direct",
        state: "Received",
        transportState: "TransportDelivered",
        applicationAckState: "NotRequired",
        receivedAtMs: now - 2_000,
        updatedAtMs: now - 2_000,
      },
      {
        messageIdHex: "native-failed-77",
        conversationId: peerDestination,
        direction: "Outbound",
        destinationHex: peerDestination,
        bodyUtf8: "Reply text must not disappear",
        method: "Opportunistic",
        state: "Failed",
        transportState: "Failed",
        applicationAckState: "Failed",
        detail: "No current LXMF route is available.",
        sentAtMs: now - 1_000,
        updatedAtMs: now - 1_000,
      },
    ],
  });

  await gotoApp(page, "/inbox");
  const failedBody = page.getByText("Reply text must not disappear");
  if (!await failedBody.isVisible()) {
    await page.getByRole("button", { name: new RegExp(`^${peerDestination}`) }).click();
  }
  await expect(failedBody).toBeVisible();
  await expect(page.getByText("No current LXMF route is available.")).toBeVisible();

  const retry = page.getByRole("button", {
    name: "Retry message: Reply text must not disappear",
  });
  await expect(retry).toBeVisible();
  await page.evaluate(async (destination) => {
    const { useNodeStore } = await import("/src/stores/nodeStore.ts");
    await useNodeStore().setPeerTier(destination, "inner");
  }, peerDestination);
  await expect(retry).toBeEnabled();
  await retry.click();

  await expect(page.getByText("Reply text must not disappear")).toBeVisible();
  await expect(page.getByText("Queued", { exact: true })).toBeVisible();
  await expect(retry).toHaveCount(0);
});
