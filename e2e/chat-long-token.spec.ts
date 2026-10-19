import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

test("a long unbroken message does not push sent bubbles off screen", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/inbox?mockChat=1");

  const composer = page.getByPlaceholder("Write an LXMF message");
  if (!await composer.isVisible()) {
    await page.getByRole("button", { name: /^ALPHA-1 / }).click();
  }

  await composer.fill(`LORA_RESOURCE_${"B".repeat(4_000)}`);
  await page.getByRole("button", { name: "Send message" }).click();
  await composer.fill("short reply");
  await page.getByRole("button", { name: "Send message" }).click();

  const threadBody = page.locator(".thread-body");
  const reply = page.locator(".bubble.outbound", { hasText: "short reply" });
  await expect(reply).toBeVisible();
  const [bodyBox, replyBox] = await Promise.all([threadBody.boundingBox(), reply.boundingBox()]);
  if (!bodyBox || !replyBox) throw new Error("thread or reply bubble is not laid out");
  expect(replyBox.x + replyBox.width).toBeLessThanOrEqual(bodyBox.x + bodyBox.width + 1);
  expect(await threadBody.evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
});
