import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

test("addressed goTenna chat uses normal conversations with honest provenance and status", async ({ page }) => {
  const peer = "77777777777777777777777777777777";
  const now = Date.now();
  await seedAppStorage(page, {
    settings: defaultSettings,
    savedPeers: [{ destination: peer, savedAt: now, circleTier: "inner" }],
    inboxMessages: [
      { messageIdHex: "mesh-in", conversationId: peer, direction: "Inbound", destinationHex: "a".repeat(32),
        sourceHex: peer, bodyUtf8: "Addressed Mesh chat", method: "Direct", state: "Received",
        transportState: "TransportDelivered", applicationAckState: "NotRequired", receivedAtMs: now - 1000,
        updatedAtMs: now - 1000, detail: "external-relay:gotenna:001122334455:unverified" },
      { messageIdHex: "mesh-out", conversationId: peer, direction: "Outbound", destinationHex: peer,
        bodyUtf8: "Addressed radio reply", method: "Direct", state: "SentDirect", transportState: "SentDirect",
        applicationAckState: "NotRequired", sentAtMs: now, updatedAtMs: now,
        detail: "external-relay:gotenna:001122334455:radio-accepted-unconfirmed" },
      { messageIdHex: "mesh-pending", conversationId: peer, direction: "Outbound", destinationHex: peer,
        bodyUtf8: "Attempt awaiting a radio result", method: "Direct", state: "Queued", transportState: "Queued",
        applicationAckState: "NotRequired", sentAtMs: now + 1, updatedAtMs: now + 1,
        detail: "external-relay:gotenna:001122334455:radio-attempt-unconfirmed" },
    ],
  });
  await gotoApp(page, "/inbox");
  if (!await page.getByText("Addressed Mesh chat", { exact: true }).isVisible()) {
    await page.getByRole("button", { name: new RegExp(`^${peer}`) }).click();
  }
  await expect(page.getByText("goTenna · unverified sender", { exact: true })).toBeVisible();
  await expect(page.getByText("Radio accepted · delivery unconfirmed", { exact: true })).toBeVisible();
  await expect(page.getByText("Radio attempt · outcome unconfirmed", { exact: true })).toBeVisible();
});

test("goTenna MECP codes render in the native event timeline", async ({ page }) => {
  await seedAppStorage(page, {
    events: [{ uid: "gotenna-native-event", type: "Incident", summary: "MECP/2/P01", callsign: "goTenna 001122334455", updatedAt: Date.now() }],
  });
  await gotoApp(page, "/events");
  const event = page.getByRole("article").filter({ hasText: "goTenna 001122334455" });
  await expect(event).toContainText("MECP/2/P01");
  await expect(event.getByText(/^Safety$/i)).toBeVisible();
});

test("plugin configuration has no inbox and requires a recipient for every chat", async ({ page }) => {
  const root = resolve(process.cwd(), "plugins/gotenna/android/src/main/assets/rem-plugin-config");
  for (const [name, contentType] of [["index.html", "text/html"], ["config.js", "text/javascript"], ["config.css", "text/css"]]) {
    await page.route(`**/gotenna-plugin/${name}`, (route) => route.fulfill({ body: readFileSync(resolve(root, name)), contentType }));
  }
  await page.goto("/gotenna-plugin/index.html");
  await page.evaluate(() => {
    const channel = new MessageChannel();
    const requests: Record<string, unknown>[] = [];
    Object.assign(window, { meshRequests: requests });
    channel.port2.onmessage = (event) => {
      requests.push(JSON.parse(event.data));
      channel.port2.postMessage(JSON.stringify({ type: "state", status: "Connected", connected: true,
        deviceName: "Mesh", gid: "001122334455", nickname: "REM", relayEnabled: false,
        inbox: [{ message: { text: "This must not be a plugin inbox" } }] }));
    };
    window.postMessage("rem-plugin-config-v1", "*", [channel.port1]);
  });
  await expect(page.getByRole("button", { name: "Send chat", exact: true })).toBeEnabled();
  await expect(page.getByRole("heading", { name: /Mesh inbox/ })).toHaveCount(0);
  await expect(page.getByText("This must not be a plugin inbox")).toHaveCount(0);
  await page.getByLabel("Message Up to 180 UTF-8 bytes").fill("Hello this person");
  await page.getByRole("button", { name: "Send chat", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("destination hash");
  await page.getByLabel("Recipient LXMF destination", { exact: false }).fill("b".repeat(32));
  await page.getByRole("button", { name: "Send chat", exact: true }).click();
  await expect.poll(() => page.evaluate(() =>
    (window as Window & { meshRequests: Record<string, unknown>[] }).meshRequests.find((item) => item.action === "sendText")?.destinationHex,
  )).toBe("b".repeat(32));
});
