import { expect, test } from "@playwright/test";

import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

const BLANK_MAP_STYLE = {
  version: 8,
  sources: {},
  layers: [],
};

test("telemetry map shows live and stale markers while filtering expired fixes", async ({ page }) => {
  const now = Date.now();

  await page.route("https://tiles.openfreemap.org/styles/liberty*", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify(BLANK_MAP_STYLE),
    });
  });

  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      telemetry: {
        enabled: false,
        publishIntervalSeconds: 10,
        staleAfterMinutes: 5,
        expireAfterMinutes: 10,
      },
    },
    telemetry: [
      {
        callsign: "Rescue-1",
        lat: 44.6488,
        lon: -63.5752,
        speed: 12.5,
        updatedAt: now - 45_000,
      },
      {
        callsign: "Relay-3",
        lat: 44.6488,
        lon: -63.5752,
        updatedAt: now - 6 * 60_000,
      },
      {
        callsign: "Expired-9",
        lat: 44.69,
        lon: -63.58,
        updatedAt: now - 11 * 60_000,
      },
    ],
    messages: [
      {
        callsign: "Rescue-1",
        groupName: "YELLOW",
        securityStatus: "Green",
        capabilityStatus: "Green",
        preparednessStatus: "Green",
        medicalStatus: "Yellow",
        mobilityStatus: "Yellow",
        commsStatus: "Yellow",
        updatedAt: now - 30_000,
      },
    ],
  });

  await gotoApp(page, "/dashboard");
  await page.getByRole("link", { name: "Map" }).click();

  await expect(page).toHaveURL(/\/telemetry$/);
  await expect(page.getByRole("heading", { name: "Map" })).toBeVisible();
  await expect(page.locator('[aria-label="Live telemetry: 1"]')).toBeVisible();
  await expect(page.locator('[aria-label="Stale telemetry: 1"]')).toBeVisible();
  await expect(page.locator('[aria-label="SOS alerts: 0"]')).toBeVisible();
  await expect(page.getByText("1 Live")).toHaveCount(0);
  await expect(page.getByText("Stale: 1")).toHaveCount(0);
  await expect(page.getByText("SOS: 0")).toHaveCount(0);
  await expect(page.getByText("Base Map")).toHaveCount(0);
  await expect(page.locator(".map-container .maplibregl-canvas")).toBeVisible();
  const bottomGap = await page.locator(".telemetry-view").evaluate((view) => {
    const content = view.closest("main");
    if (!content) {
      return Number.POSITIVE_INFINITY;
    }
    const contentRect = content.getBoundingClientRect();
    const viewRect = view.getBoundingClientRect();
    return Math.abs(contentRect.bottom - viewRect.bottom);
  });
  expect(bottomGap).toBeLessThanOrEqual(2);

  const layerButton = page.getByRole("button", { name: "Map layer: Base" });
  await expect(layerButton).toHaveAttribute("data-map-layer", "base");
  await layerButton.click();
  await expect(page.getByRole("menuitemradio", { name: "Base" })).toBeVisible();
  await page.getByRole("menuitemradio", { name: "Satellite" }).click();
  await expect(page.getByRole("button", { name: "Map layer: Satellite" })).toHaveAttribute(
    "data-map-layer",
    "satellite",
  );

  await expect(page.locator(".telemetry-marker")).toHaveCount(2);
  await expect(page.locator('.telemetry-marker.is-live[title="Rescue-1"]')).toBeVisible();
  await expect(page.locator('.telemetry-marker.is-stale[title="Relay-3"]')).toBeVisible();
  await expect(page.locator('.telemetry-marker.is-overlapped[data-overlap-count="2"]')).toHaveCount(2);
  await expect(page.locator(".telemetry-marker-label", { hasText: "Rescue-1" })).toBeVisible();
  await expect(page.locator(".telemetry-marker-label", { hasText: "Relay-3" })).toBeVisible();
  await expect(page.locator('.telemetry-marker[title="Expired-9"]')).toHaveCount(0);

  await page.locator('.telemetry-marker[title="Rescue-1"]').click();
  await expect(page.locator(".maplibregl-popup")).toContainText("Rescue-1");
  await expect(page.locator(".maplibregl-popup")).toContainText("Speed 12.5");
  await expect(page.locator(".popup-eam-pie")).toHaveText("75%");
  await page.evaluate(async ({ timestamp }) => {
    const mod = await import("/src/stores/telemetryStore.ts");
    const store = mod.useTelemetryStore();
    await store.upsertLocalPosition({
      callsign: "Rescue-1",
      lat: 44.6489,
      lon: -63.5753,
      speed: 13.5,
      updatedAt: timestamp,
    });
  }, { timestamp: now });
  await expect(page.locator(".maplibregl-popup")).toBeVisible();
  await expect(page.locator(".maplibregl-popup")).toContainText("Speed 13.5");
  await page.getByRole("button", { name: "Details" }).click();
  await expect(page).toHaveURL(/\/messages\?callsign=Rescue-1$/);
  await expect(page.getByRole("heading", { name: "Rescue-1" })).toBeVisible();
});

test("telemetry popup opens a chat thread for the matched peer", async ({ page }) => {
  const now = Date.now();
  const lxmfDestinationHex = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

  await page.route("https://tiles.openfreemap.org/styles/liberty*", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify(BLANK_MAP_STYLE),
    });
  });

  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      telemetry: {
        enabled: false,
        publishIntervalSeconds: 10,
        staleAfterMinutes: 5,
        expireAfterMinutes: 10,
      },
    },
    telemetry: [
      {
        callsign: "Rescue-Chat",
        lat: 44.6488,
        lon: -63.5752,
        updatedAt: now - 45_000,
      },
    ],
    savedPeers: [
      {
        destination: lxmfDestinationHex,
        label: "Rescue-Chat",
        savedAt: now - 60_000,
      },
    ],
  });

  await gotoApp(page, "/telemetry");
  await expect(page.locator(".map-container .maplibregl-canvas")).toBeVisible();

  await page.locator('.telemetry-marker[title="Rescue-Chat"]').click();
  await expect(page.getByRole("button", { name: "Chat" })).toBeEnabled();
  await page.getByRole("button", { name: "Chat" }).click();
  await expect(page).toHaveURL(/\/inbox\?conversation=draft(?::|%3A)bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb$/);
  await expect(page.getByRole("heading", { name: "Rescue-Chat" })).toBeVisible();
  await expect(page.getByText("No messages yet for this conversation.")).toBeVisible();
});

test("telemetry publishing reacts when runtime readiness changes", async ({ page, context }) => {
  await context.grantPermissions(["geolocation"]);
  await context.setGeolocation({ latitude: 44.6488, longitude: -63.5752 });
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      telemetry: {
        enabled: true,
        publishIntervalSeconds: 3600,
        staleAfterMinutes: 5,
        expireAfterMinutes: 10,
      },
    },
  });

  await gotoApp(page, "/dashboard");
  await expect.poll(async () => page.evaluate(async () => {
    const mod = await import("/src/stores/telemetryStore.ts");
    return mod.useTelemetryStore().loopStatus;
  })).toBe("running");

  const publishAttempts = await page.evaluate(async () => {
    const telemetryMod = await import("/src/services/telemetry.ts");
    const nodeMod = await import("/src/stores/nodeStore.ts");
    const original = telemetryMod.telemetryService.getCurrentPosition.bind(telemetryMod.telemetryService);
    let attempts = 0;
    telemetryMod.telemetryService.getCurrentPosition = async () => {
      attempts += 1;
      return original();
    };

    const nodeStore = nodeMod.useNodeStore();
    nodeStore.status = {
      ...nodeStore.status,
    };
    await new Promise((resolve) => window.setTimeout(resolve, 100));
    const attemptsAfterSameStateReplacement = attempts;

    nodeStore.status = {
      ...nodeStore.status,
      running: !nodeStore.status.running,
    };

    const deadline = Date.now() + 2_000;
    while (attempts === 0 && Date.now() < deadline) {
      await new Promise((resolve) => window.setTimeout(resolve, 20));
    }
    return {
      attemptsAfterReadinessTransition: attempts,
      attemptsAfterSameStateReplacement,
    };
  });

  expect(publishAttempts.attemptsAfterSameStateReplacement).toBe(0);
  expect(publishAttempts.attemptsAfterReadinessTransition).toBeGreaterThan(0);
});

test("telemetry map hides locations for cancelled SOS emergencies", async ({ page }) => {
  const now = Date.now();

  await page.route("https://tiles.openfreemap.org/styles/liberty*", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify(BLANK_MAP_STYLE),
    });
  });

  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await gotoApp(page, "/telemetry");
  await expect(page.getByRole("heading", { name: "Map" })).toBeVisible();
  await expect(page.locator(".map-container .maplibregl-canvas")).toBeVisible();

  await page.evaluate(async ({ timestamp }) => {
    const mod = await import("/src/stores/sosStore.ts");
    const store = mod.useSosStore();
    const activeSource = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const cancelledSource = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    store.alerts = [
      {
        incidentId: "active-incident",
        sourceHex: activeSource,
        conversationId: activeSource,
        state: "Active",
        active: true,
        bodyUtf8: "Active SOS",
        lat: 44.6488,
        lon: -63.5752,
        messageIdHex: "11111111111111111111111111111111",
        receivedAtMs: timestamp - 60_000,
        updatedAtMs: timestamp - 60_000,
      },
      {
        incidentId: "cancelled-incident",
        sourceHex: cancelledSource,
        conversationId: cancelledSource,
        state: "Cancelled",
        active: false,
        bodyUtf8: "SOS Cancelled",
        lat: 44.6501,
        lon: -63.5771,
        messageIdHex: "22222222222222222222222222222222",
        receivedAtMs: timestamp - 45_000,
        updatedAtMs: timestamp - 45_000,
      },
    ];
    store.locations = [
      {
        incidentId: "active-incident",
        sourceHex: activeSource,
        lat: 44.6488,
        lon: -63.5752,
        recordedAtMs: timestamp - 55_000,
      },
      {
        incidentId: "cancelled-incident",
        sourceHex: cancelledSource,
        lat: 44.6501,
        lon: -63.5771,
        recordedAtMs: timestamp - 40_000,
      },
    ];
  }, { timestamp: now });

  await expect(page.locator('[aria-label="SOS alerts: 1"]')).toBeVisible();
  await expect(page.locator(".sos-trail-marker")).toHaveCount(1);
});

test("telemetry map clusters close positions into a count bubble when zoomed out", async ({ page }) => {
  const now = Date.now();

  await page.route("https://tiles.openfreemap.org/styles/liberty*", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify(BLANK_MAP_STYLE),
    });
  });

  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      telemetry: {
        enabled: false,
        publishIntervalSeconds: 10,
        staleAfterMinutes: 5,
        expireAfterMinutes: 10,
      },
    },
    telemetry: [
      {
        callsign: "Noemi",
        lat: 44.6488,
        lon: -63.5752,
        updatedAt: now - 45_000,
      },
      {
        callsign: "Poco",
        lat: 44.64892,
        lon: -63.57534,
        updatedAt: now - 65_000,
      },
      {
        callsign: "Relay",
        lat: 44.64904,
        lon: -63.57548,
        updatedAt: now - 85_000,
      },
    ],
  });

  await gotoApp(page, "/dashboard");
  await page.getByRole("link", { name: "Map" }).click();
  await expect(page).toHaveURL(/\/telemetry$/);
  await expect(page.getByRole("heading", { name: "Map" })).toBeVisible();
  await expect(page.locator(".map-container .maplibregl-canvas")).toBeVisible();
  await expect(page.locator(".telemetry-marker")).toHaveCount(3);

  await page.locator(".maplibregl-ctrl-zoom-out").evaluate((button) => {
    (button as HTMLButtonElement).click();
  });
  await page.locator(".maplibregl-ctrl-zoom-out").evaluate((button) => {
    (button as HTMLButtonElement).click();
  });
  await page.locator(".maplibregl-ctrl-zoom-out").evaluate((button) => {
    (button as HTMLButtonElement).click();
  });
  await expect(page.locator('.telemetry-cluster[data-count="3"]')).toBeVisible();
  await expect(page.locator(".telemetry-marker")).toHaveCount(0);

  await page.locator(".maplibregl-ctrl-zoom-in").evaluate((button) => {
    (button as HTMLButtonElement).click();
  });
  await page.locator(".maplibregl-ctrl-zoom-in").evaluate((button) => {
    (button as HTMLButtonElement).click();
  });
  await page.locator(".maplibregl-ctrl-zoom-in").evaluate((button) => {
    (button as HTMLButtonElement).click();
  });
  await expect(page.locator(".telemetry-cluster")).toHaveCount(0);
  await expect(page.locator(".telemetry-marker")).toHaveCount(3);
});
