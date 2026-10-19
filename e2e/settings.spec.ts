import { expect, test } from "@playwright/test";

import { DEFAULT_TCP_COMMUNITY_ENDPOINT } from "../apps/mobile/src/utils/tcpCommunityServers";
import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

async function seedHubDirectorySnapshot(
  page: import("@playwright/test").Page,
  snapshot: {
    effectiveConnectedMode: boolean;
    receivedAtMs: number;
    items: Array<{
      identity: string;
      destinationHash: string;
      displayName?: string;
      announceCapabilities: string[];
      clientType?: string;
      registeredMode?: string;
      lastSeen?: string;
      status?: string;
    }>;
  },
): Promise<void> {
  await page.evaluate(async (nextSnapshot) => {
    const mod = await import("/src/stores/nodeStore.ts");
    const store = mod.useNodeStore();
    store.hubDirectorySnapshot = nextSnapshot;
  }, snapshot);
}

async function storedSettings(page: import("@playwright/test").Page): Promise<Record<string, any>> {
  return page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );
}

async function openAdvanced(page: import("@playwright/test").Page): Promise<void> {
  await page.locator("details.settings-advanced > summary", { hasText: "Advanced" }).click();
}

test("settings index groups pages by goal and remembers folded sections", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings");

  for (const group of ["You", "Connectivity", "Sharing", "Emergency", "Device"]) {
    await expect(page.getByRole("button", { name: new RegExp(`^${group}`) })).toBeVisible();
  }
  await expect(page.getByRole("link", { name: /Network interfaces/ })).toContainText("TCP · LoRa off");
  await expect(page.getByRole("button", { name: "Save" })).toHaveCount(0);

  const sharing = page.getByRole("button", { name: /^Sharing/ });
  await sharing.click();
  await expect(sharing).toHaveAttribute("aria-expanded", "false");
  await expect(sharing).toContainText("Base map");
  await expect(page.getByRole("link", { name: /Location sharing/ })).toBeHidden();

  await page.reload();
  await expect(page.getByRole("button", { name: /^Sharing/ })).toHaveAttribute("aria-expanded", "false");
  await page.getByRole("button", { name: /^Sharing/ }).click();
  await expect(page.getByRole("link", { name: /Location sharing/ })).toBeVisible();
});

test("fresh installs default to the first TCP community server", async ({ page }) => {
  await seedAppStorage(page, {});

  await gotoApp(page, "/settings/network");

  const firstServer = page
    .locator("label.server-option")
    .filter({ hasText: DEFAULT_TCP_COMMUNITY_ENDPOINT });

  await expect(firstServer.getByRole("checkbox")).toBeChecked();
  await openAdvanced(page);
  await expect(page.getByLabel("Transport node forwarding")).toBeChecked();
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();
});

test("rmap TCP selection is preserved as a community server", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      tcpClients: ["rmap.world:4242"],
    },
  });

  await gotoApp(page, "/settings/network");

  const rmapServer = page.locator("label.server-option").filter({ hasText: "rmap.world:4242" });

  await expect(rmapServer.getByRole("checkbox")).toBeChecked();
  await expect(page.getByText(DEFAULT_TCP_COMMUNITY_ENDPOINT)).toBeVisible();
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();

  const runtimeSettings = await page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    return mod.useNodeStore().settings;
  });

  expect(runtimeSettings.tcpClients).toEqual(["rmap.world:4242"]);
  expect((await storedSettings(page)).tcpClients).toEqual(["rmap.world:4242"]);
});

test("operators can update runtime settings and persist TCP endpoints", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      displayName: "Atlas-1",
    },
  });

  await gotoApp(page, "/settings/identity");
  await page.getByLabel("Call sign").fill("Atlas-7");
  await page.getByRole("button", { name: "Save" }).click();
  await expect.poll(async () => (await storedSettings(page)).displayName).toBe("Atlas-7");

  await gotoApp(page, "/settings/network");
  await openAdvanced(page);
  await page.getByLabel("Transport node forwarding").uncheck();
  await page
    .getByPlaceholder("Add custom endpoint (host:port or tcp://host:port)")
    .fill("mesh.example.org:5151");
  await page.getByRole("button", { name: "Add" }).click();

  await expect(page.locator(".active-endpoint").getByText("mesh.example.org:5151")).toBeVisible();

  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();

  const stored = await storedSettings(page);
  expect(stored.displayName).toBe("Atlas-7");
  expect(stored.transportNodeEnabled).toBe(false);
  expect(stored.tcpClients).toContain("mesh.example.org:5151");
});

test("operators can add Reticulum-style TCP URLs as custom endpoints", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await gotoApp(page, "/settings/network");
  await page
    .getByPlaceholder("Add custom endpoint (host:port or tcp://host:port)")
    .fill("tcp://mesh.example.org:5151");
  await page.getByRole("button", { name: "Add" }).click();

  await expect(page.locator(".active-endpoint").getByText("mesh.example.org:5151")).toBeVisible();
  await page.getByRole("button", { name: "Save" }).click();

  await expect.poll(async () => (await storedSettings(page)).tcpClients).toContain("mesh.example.org:5151");
});

test("operators can save manual RNode LoRa configuration", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await gotoApp(page, "/settings/network");
  await page.getByLabel("Enable RNode Bluetooth LoRa").check();
  await page.getByRole("link", { name: /Pair RNode device/ }).click();
  await expect(page).toHaveURL(/\/settings\/network\/rnode-pairing$/);
  await page.getByLabel("RNode device id").fill("AA:BB:CC:DD:EE:FF");
  await page.getByLabel("RNode display name").fill("Field RNode");
  await page.getByRole("button", { name: "Done" }).click();

  await expect(page).toHaveURL(/\/settings\/network$/);
  await expect(page.getByRole("link", { name: /Pair RNode device/ })).toContainText("Field RNode");
  await page.getByLabel("Region").selectOption("EU868");
  await page.getByLabel("REM LoRa profile").selectOption("REM-LM-EXTREME-v1");

  await expect(page.getByRole("button", { name: "Save" })).toBeEnabled();
  await page.getByRole("button", { name: "Save" }).click();

  await expect.poll(async () => (await storedSettings(page)).rnode).toEqual({
    enabled: true,
    connectionMode: "ble",
    peripheralId: "AA:BB:CC:DD:EE:FF",
    displayName: "Field RNode",
    region: "EU868",
    profile: "REM-LM-EXTREME-v1",
    frequencyHz: 868_000_000,
  });
});

test("out-of-range RNode LoRa frequency is not persisted", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await gotoApp(page, "/settings/network");
  const settingsBefore = await storedSettings(page);
  await page.getByLabel("Enable RNode Bluetooth LoRa").check();
  await page.getByLabel("Frequency (Hz)").fill("1");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText(
    "RNode LoRa frequency must be between 137000000 and 3000000000 Hz.",
  )).toBeVisible();
  expect(await storedSettings(page)).toEqual(settingsBefore);
});

test("telemetry publish interval above 60 seconds activates save and persists", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      telemetry: {
        ...defaultSettings.telemetry,
        publishIntervalSeconds: 10,
      },
    },
  });

  await gotoApp(page, "/settings/location");
  await page.getByLabel("Telemetry publish interval (seconds)").fill("120");

  await expect(page.getByRole("button", { name: "Save" })).toBeEnabled();
  await page.getByRole("button", { name: "Save" }).click();

  await expect.poll(async () => (await storedSettings(page)).telemetry.publishIntervalSeconds).toBe(120);
});

test("location sharing blocks a hide time shorter than the stale time", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings/location");

  await page.getByLabel("Telemetry goes stale after (minutes)").fill("60");
  await page.getByLabel("Telemetry disappears after (minutes)").fill("30");
  await expect(page.getByRole("alert")).toContainText("Hide time must be the same as or longer");
  await page.getByRole("button", { name: "Save" }).click();
  expect((await storedSettings(page)).telemetry?.staleAfterMinutes).not.toBe(60);
});

test("leaving a form with unsaved changes asks to discard or keep editing", async ({ page }) => {
  await seedAppStorage(page, { settings: { ...defaultSettings, displayName: "Atlas-1" } });
  await gotoApp(page, "/settings/identity");

  await page.getByLabel("Call sign").fill("Atlas-9");
  await page.getByRole("button", { name: "Back to settings" }).click();
  const dialog = page.getByRole("alertdialog", { name: "Discard unsaved changes?" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Keep editing" }).click();
  await expect(page).toHaveURL(/\/settings\/identity$/);
  await expect(page.getByLabel("Call sign")).toHaveValue("Atlas-9");

  await page.getByRole("button", { name: "Back to settings" }).click();
  await dialog.getByRole("button", { name: "Discard" }).click();
  await expect(page).toHaveURL(/\/settings$/);
  expect((await storedSettings(page)).displayName).toBe("Atlas-1");
});

test("saving a restart field shows the restart banner until the node restarts", async ({ page }) => {
  await seedAppStorage(page, { settings: { ...defaultSettings, displayName: "Atlas-1" } });
  await gotoApp(page, "/settings/identity");
  await expect.poll(() => page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    return mod.useNodeStore().status.running;
  })).toBe(true);

  await page.getByLabel("Call sign").fill("Atlas-2");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.locator(".settings-save-bar")).toContainText("Restart the node");
  await page.getByRole("button", { name: "Back to settings" }).click();

  const banner = page.locator(".restart-banner");
  await expect(banner).toContainText("Restart node to apply: call sign");
  await banner.getByRole("button", { name: "Restart" }).click();
  await expect(banner).toHaveCount(0);
});

test("persisted RCH mode does not block local runtime startup", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      hub: {
        ...defaultSettings.hub,
        mode: "SemiAutonomous",
        identityHash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      },
    },
  });

  await gotoApp(page, "/settings/routing");

  await expect(page.getByText("Interfaces are loading")).toHaveCount(0);
  await expect(page.getByRole("radio", { name: /Semi-autonomous/ })).toBeChecked();
  await expect(page.getByRole("radio", { name: /Semi-autonomous/ })).toBeEnabled();
  await expect(page.getByLabel("Hub identity hash")).toHaveValue(
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  );

  const runtimeHub = await page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    return mod.useNodeStore().settings.hub;
  });
  expect(runtimeHub.mode).toBe("SemiAutonomous");
});

test("stopping the node returns to the regular screen and reports stopped", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings/node");

  const nodeControl = page.getByRole("region", { name: "Node Control" });
  await expect(nodeControl).toContainText("Node is running");

  await nodeControl.getByRole("button", { name: "Stop", exact: true }).click();

  await expect(page.getByTestId("splash-interface-loading")).toHaveCount(0);
  await expect(nodeControl).toContainText("Node is stopped");
  await expect(page.locator(".running")).toHaveText("Stopped");
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  await expect.poll(async () => page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    return mod.useNodeStore().status.running;
  })).toBe(false);
});

test("semi-autonomous RCH exposes effective connected routing state", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      hub: {
        ...defaultSettings.hub,
        mode: "SemiAutonomous",
        identityHash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      },
    },
  });

  await gotoApp(page, "/settings/routing");
  await seedHubDirectorySnapshot(page, {
    effectiveConnectedMode: true,
    receivedAtMs: Date.now(),
    items: [
      {
        identity: "11111111111111111111111111111111",
        destinationHash: "22222222222222222222222222222222",
        displayName: "Pixel",
        announceCapabilities: ["r3akt", "telemetry"],
        clientType: "rem",
        registeredMode: "connected",
        lastSeen: "2026-04-02T12:43:28Z",
        status: "active",
      },
    ],
  });

  const status = page.locator(".status-rows");
  await expect(page.getByRole("radio", { name: /Semi-autonomous/ })).toBeChecked();
  await expect(status).toContainText("1 cached peers");
  await expect(status).toContainText("server forcing connected routing");
  await expect(status).not.toContainText("outbound blocked");
});

test("SOS trigger asks for confirmation before sending", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings/sos");

  await page.getByRole("button", { name: "Trigger SOS" }).click();
  await expect(page.getByRole("button", { name: "Send SOS to peers" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("button", { name: "Send SOS to peers" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: /Deactivation PIN/ })).toBeVisible();
});

test("plugin management is present without storing plugin configuration in the web app", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings");
  await page.getByRole("link", { name: /Plugins/ }).click();

  const pluginPanel = page.getByTestId("plugin-settings-panel");
  await expect(pluginPanel.getByRole("button", { name: "Refresh installed plugins" })).toBeVisible();
  await expect(pluginPanel).toContainText("No plugin APKs were discovered");
  const pluginSettingsKeys = await page.evaluate(() =>
    Object.keys(window.localStorage).filter((key) => key.toLowerCase().includes("plugin")),
  );
  expect(pluginSettingsKeys).toEqual([]);
});

test("Settings links to the Manage Teams page", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings");

  await expect(page.getByText("Manage Peers")).toHaveCount(0);
  await page.getByRole("link", { name: /Teams/ }).click();

  await expect(page).toHaveURL(/\/settings\/teams$/);
  await expect(page.getByRole("heading", { name: "Manage Teams" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Add team", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Scan QR", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Back to settings" })).toBeVisible();
});
