import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

test.beforeEach(async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/dashboard");
});

test("More tools keeps navigation, active links and close controls", async ({ page }) => {
  const more = page.getByRole("button", { name: "More", exact: true });
  const menu = page.getByRole("complementary", { name: "More tools" });
  await more.click();
  await expect(menu.getByRole("link")).toHaveCount(7);
  await menu.getByRole("button", { name: "Close more tools" }).focus();
  await page.keyboard.press("Enter");
  await expect(menu).toBeHidden();
  await more.click();
  await menu.getByRole("link", { name: "Checklists", exact: true }).click();
  await expect(page).toHaveURL(/\/checklists$/);
  await expect(menu).toBeHidden();
  await more.click();
  await expect(menu.getByRole("link", { name: "Checklists", exact: true })).toHaveClass(/active/);
  await page.locator(".menu-backdrop").click({ position: { x: 5, y: 5 } });
  await expect(menu).toBeHidden();
});

test("cancelling shutdown keeps REM running", async ({ page }) => {
  await page.getByRole("button", { name: "More", exact: true }).click();
  page.once("dialog", async (dialog) => {
    expect(dialog.message()).toContain("background service will stop");
    await dialog.dismiss();
  });
  await page.getByRole("button", { name: "Quit", exact: true }).click();
  await expect(page.getByRole("complementary", { name: "More tools" })).toBeVisible();
  await expect.poll(() => page.evaluate(async () => {
    const { useNodeStore } = await import("/src/stores/nodeStore.ts");
    return useNodeStore().status.running;
  })).toBe(true);
});

test("shutdown stops REM and closes the menu", async ({ page }) => {
  await page.getByRole("button", { name: "More", exact: true }).click();
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Quit", exact: true }).focus();
  await page.keyboard.press("Enter");
  await expect(page.locator(".running")).toHaveText("Stopped");
  await expect(page.getByRole("complementary", { name: "More tools" })).toBeHidden();
  await expect(page.getByTestId("splash-interface-loading")).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Dashboard", exact: true })).toBeVisible();
});

test("failed shutdown stays open and offers retry", async ({ page }) => {
  await page.evaluate(async () => {
    const { useNodeStore } = await import("/src/stores/nodeStore.ts");
    useNodeStore().stopNode = async () => { throw new Error("Native stop failed"); };
  });
  await page.getByRole("button", { name: "More", exact: true }).click();
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Quit", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Native stop failed");
  await expect(page.getByRole("button", { name: "Quit", exact: true })).toBeEnabled();
  await expect.poll(() => page.evaluate(async () => {
    const { useNodeStore } = await import("/src/stores/nodeStore.ts");
    return useNodeStore().status.running;
  })).toBe(true);
});

test("pending shutdown cannot be submitted twice after reopening the menu", async ({ page }) => {
  await page.evaluate(async () => {
    const { useNodeStore } = await import("/src/stores/nodeStore.ts");
    const store = useNodeStore();
    const stop = store.stopNode;
    Reflect.set(window, "shutdownCalls", 0);
    store.stopNode = async () => {
      Reflect.set(window, "shutdownCalls", Reflect.get(window, "shutdownCalls") + 1);
      await new Promise<void>((resolve) => { Reflect.set(window, "finishShutdown", resolve); });
      await stop();
    };
  });
  const more = page.getByRole("button", { name: "More", exact: true });
  await more.click();
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Quit", exact: true }).click();
  await expect(page.getByRole("button", { name: "Quitting…", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "Close more tools" }).click();
  await more.click();
  await expect(page.getByRole("button", { name: "Quitting…", exact: true })).toBeDisabled();
  await page.evaluate(() => {
    const finish = Reflect.get(window, "finishShutdown");
    if (typeof finish === "function") finish();
  });
  await expect(page.locator(".running")).toHaveText("Stopped");
  expect(await page.evaluate(() => Reflect.get(window, "shutdownCalls"))).toBe(1);
});
