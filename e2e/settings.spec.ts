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

test("fresh installs default to the first TCP community server", async ({ page }) => {
  await seedAppStorage(page, {});

  await gotoApp(page, "/settings");

  const runtimePanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Node Config" }),
  });

  await runtimePanel.locator("summary").click();

  const firstServer = page
    .locator("label.server-option")
    .filter({ hasText: DEFAULT_TCP_COMMUNITY_ENDPOINT });

  await expect(firstServer.getByRole("checkbox")).toBeChecked();
  await expect(runtimePanel.getByLabel("Transport node forwarding")).toBeChecked();
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();
});

test("rmap TCP selection is preserved as a community server", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      tcpClients: ["rmap.world:4242"],
    },
  });

  await gotoApp(page, "/settings");

  const runtimePanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Node Config" }),
  });

  await runtimePanel.locator("summary").click();

  const rmapServer = page.locator("label.server-option").filter({ hasText: "rmap.world:4242" });

  await expect(rmapServer.getByRole("checkbox")).toBeChecked();
  await expect(page.getByText(DEFAULT_TCP_COMMUNITY_ENDPOINT)).toBeVisible();
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();

  const runtimeSettings = await page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    return mod.useNodeStore().settings;
  });

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(runtimeSettings.tcpClients).toEqual(["rmap.world:4242"]);
  expect(storedSettings.tcpClients).toEqual(["rmap.world:4242"]);
});

test("operators can update runtime settings and persist TCP endpoints", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      displayName: "Atlas-1",
    },
  });

  await gotoApp(page, "/settings");

  const runtimePanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Node Config" }),
  });

  await runtimePanel.locator("summary").click();
  await runtimePanel.getByLabel("Call Sign").fill("Atlas-7");
  await runtimePanel.getByLabel("Transport node forwarding").uncheck();
  await runtimePanel
    .getByPlaceholder("Add custom endpoint (host:port or tcp://host:port)")
    .fill("mesh.example.org:5151");
  await runtimePanel.getByRole("button", { name: "Add" }).click();

  await expect(runtimePanel.getByText("mesh.example.org:5151")).toBeVisible();

  await page.getByRole("button", { name: "Save" }).click();

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(storedSettings.displayName).toBe("Atlas-7");
  expect(storedSettings.transportNodeEnabled).toBe(false);
  expect(storedSettings.tcpClients).toContain("mesh.example.org:5151");
});

test("operators can add Reticulum-style TCP URLs as custom endpoints", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await gotoApp(page, "/settings");

  const runtimePanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Node Config" }),
  });

  await runtimePanel.locator("summary").click();
  await runtimePanel
    .getByPlaceholder("Add custom endpoint (host:port or tcp://host:port)")
    .fill("tcp://mesh.example.org:5151");
  await runtimePanel.getByRole("button", { name: "Add" }).click();

  await expect(runtimePanel.getByText("mesh.example.org:5151")).toBeVisible();
  await page.getByRole("button", { name: "Save" }).click();

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(storedSettings.tcpClients).toContain("mesh.example.org:5151");
});

test("operators can save manual RNode LoRa configuration", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await gotoApp(page, "/settings");

  const runtimePanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Node Config" }),
  });

  await runtimePanel.locator("summary").click();
  await runtimePanel.getByLabel("Enable RNode Bluetooth LoRa").check();
  await runtimePanel.getByLabel("RNode device id").fill("AA:BB:CC:DD:EE:FF");
  await runtimePanel.getByLabel("RNode display name").fill("Field RNode");
  await runtimePanel.getByLabel("Region").selectOption("EU868");
  await runtimePanel.getByLabel("REM LoRa profile").selectOption("REM-LM-EXTREME-v1");

  await expect(page.getByRole("button", { name: "Save" })).toBeEnabled();
  await page.getByRole("button", { name: "Save" }).click();

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(storedSettings.rnode).toEqual({
    enabled: true,
    connectionMode: "ble",
    peripheralId: "AA:BB:CC:DD:EE:FF",
    displayName: "Field RNode",
    region: "EU868",
    profile: "REM-LM-EXTREME-v1",
  });
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

  await gotoApp(page, "/settings");

  const telemetryPanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Telemetry" }),
  });

  await telemetryPanel.locator("summary").click();
  await telemetryPanel.getByLabel("Telemetry publish interval (seconds)").fill("120");

  await expect(page.getByRole("button", { name: "Save" })).toBeEnabled();
  await page.getByRole("button", { name: "Save" }).click();

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(storedSettings.telemetry.publishIntervalSeconds).toBe(120);
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

  await gotoApp(page, "/settings");
  const hubPanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "RCH Hub Directory" }),
  });
  await hubPanel.locator("summary").click();

  await expect(page.getByText("Interfaces are loading")).toHaveCount(0);
  await expect(hubPanel.getByLabel("Mode")).toBeEnabled();
  await expect(hubPanel.getByLabel("Mode")).toHaveValue("SemiAutonomous");
  await expect(hubPanel.getByLabel("Hub identity hash")).toHaveValue(
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
  await gotoApp(page, "/settings");

  const nodeControlPanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Node Control" }),
  });
  await nodeControlPanel.locator("summary").click();
  await expect(nodeControlPanel).toContainText("Node is running");

  await nodeControlPanel.getByRole("button", { name: "Stop", exact: true }).click();

  await expect(page.getByTestId("splash-interface-loading")).toHaveCount(0);
  await expect(nodeControlPanel).toContainText("Node is stopped");
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

  await gotoApp(page, "/settings");
  const hubPanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "RCH Hub Directory" }),
  });
  await hubPanel.locator("summary").click();
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

  await expect(hubPanel.getByLabel("Mode")).toBeEnabled();
  await expect(hubPanel.getByLabel("Mode")).toHaveValue("SemiAutonomous");
  await expect(hubPanel).toContainText("1 cached peers");
  await expect(hubPanel).toContainText("server forcing connected routing");
  await expect(hubPanel).not.toContainText("outbound blocked");
});

test("plugin management is present without storing plugin configuration in the web app", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings");

  const pluginPanel = page.getByTestId("plugin-settings-panel");
  await pluginPanel.locator("summary").click();

  await expect(pluginPanel.locator("summary .summary-icon-svg")).toBeVisible();
  await expect(pluginPanel.getByRole("button", { name: "Refresh installed plugins" })).toBeVisible();
  await expect(pluginPanel).toContainText("No plugin APKs were discovered");
  const pluginSettingsKeys = await page.evaluate(() =>
    Object.keys(window.localStorage).filter((key) => key.toLowerCase().includes("plugin")),
  );
  expect(pluginSettingsKeys).toEqual([]);
});

test("Settings replaces Manage Peers with the Manage Teams page", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings");

  const teamEntry = page.locator(".settings-team-entry");
  await expect(teamEntry.getByRole("heading", { name: "Manage Teams" })).toBeVisible();
  await expect(page.getByText("Manage Peers")).toHaveCount(0);
  await teamEntry.getByRole("button", { name: "Open" }).click();

  await expect(page).toHaveURL(/\/settings\/teams$/);
  await expect(page.getByRole("heading", { name: "Manage Teams" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Add team", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Scan QR", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Back to settings" })).toBeVisible();
});
