import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

test("chat message links render as styled external anchors", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await gotoApp(page, "/inbox?mockChat=1");
  await expect(page.getByRole("heading", { name: "Chat" })).toBeVisible();

  const message = "Use https://example.org/ops?id=42, then report.";
  await page.getByPlaceholder("Write an LXMF message").fill(message);
  await page.getByRole("button", { name: "Send message" }).click();

  const link = page.locator(".bubble-content .message-link", {
    hasText: "https://example.org/ops?id=42",
  });
  await expect(link).toBeVisible();
  const renderedLink = await link.evaluate((element) => ({
    href: element.getAttribute("href"),
    target: element.getAttribute("target"),
    rel: element.getAttribute("rel"),
    className: element.className,
    body: element.closest(".bubble-content")?.textContent?.replace(/\s+/g, " ").trim(),
  }));
  expect(renderedLink.href).toBe("https://example.org/ops?id=42");
  expect(renderedLink.target).toBe("_blank");
  expect(renderedLink.rel).toBe("noopener noreferrer");
  expect(renderedLink.className).toContain("sos-map-link");
  expect(renderedLink.body).toBe("Use https://example.org/ops?id=42, then report.");
});

test("chat peer status and coordinates link to EAM details and telemetry map", async ({ page }) => {
  const now = Date.now();

  await seedAppStorage(page, {
    settings: defaultSettings,
    messages: [
      {
        callsign: "ALPHA-1",
        groupName: "YELLOW",
        securityStatus: "Green",
        capabilityStatus: "Green",
        preparednessStatus: "Green",
        medicalStatus: "Green",
        mobilityStatus: "Green",
        commsStatus: "Green",
        updatedAt: now - 30_000,
      },
    ],
    telemetry: [
      {
        callsign: "ALPHA-1",
        lat: 44.6488,
        lon: -63.5752,
        updatedAt: now - 20_000,
      },
    ],
  });

  await gotoApp(page, "/inbox?mockChat=1");
  await expect(page.getByRole("heading", { name: "ALPHA-1" })).toBeVisible();

  const statusLink = page.getByRole("link", { name: "Open EAM details for ALPHA-1" });
  await expect(statusLink).toHaveClass(/sos-map-link/);
  await statusLink.click();
  await expect(page).toHaveURL(/\/messages\?callsign=ALPHA-1$/);
  await expect(page.locator(".item.selected", { hasText: "ALPHA-1" }).first()).toBeVisible();

  await gotoApp(page, "/inbox?mockChat=1");
  const coordinateLink = page.getByRole("link", { name: /Open 44\.65.+63\.58.+on telemetry map/ });
  await expect(coordinateLink).toHaveClass(/sos-map-link/);
  await coordinateLink.click();
  await expect(page).toHaveURL(/\/telemetry\?callsign=ALPHA-1&lat=44\.6488&lon=-63\.5752$/);
});
