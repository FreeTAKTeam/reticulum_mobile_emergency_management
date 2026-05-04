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
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();
});

test("legacy placeholder TCP selection normalizes to the first community server", async ({ page }) => {
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

  const firstServer = page
    .locator("label.server-option")
    .filter({ hasText: DEFAULT_TCP_COMMUNITY_ENDPOINT });

  await expect(firstServer.getByRole("checkbox")).toBeChecked();
  await expect(page.getByText("rmap.world:4242")).toHaveCount(0);

  await page.getByRole("button", { name: "Save" }).click();

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(storedSettings.tcpClients).toEqual([DEFAULT_TCP_COMMUNITY_ENDPOINT]);
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
  await runtimePanel.getByPlaceholder("Add custom endpoint (host:port)").fill("mesh.example.org:5151");
  await runtimePanel.getByRole("button", { name: "Add" }).click();

  await expect(runtimePanel.getByText("mesh.example.org:5151")).toBeVisible();

  await page.getByRole("button", { name: "Save" }).click();

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(storedSettings.displayName).toBe("Atlas-7");
  expect(storedSettings.tcpClients).toContain("mesh.example.org:5151");
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

test("RCH hub directory is disabled and coerces persisted mode to autonomous", async ({ page }) => {
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

  await expect(hubPanel.getByLabel("Mode")).toBeDisabled();
  await expect(hubPanel.getByLabel("Mode")).toHaveValue("Autonomous");
  await expect(hubPanel.getByLabel("Hub from announces (RCH servers)")).toBeDisabled();
  await expect(hubPanel.getByLabel("Hub identity hash")).toBeDisabled();
  await expect(hubPanel.getByLabel("Refresh interval seconds")).toBeDisabled();
  await expect(hubPanel.getByRole("button", { name: "Refresh Now" })).toBeDisabled();
  await expect(hubPanel.getByRole("button", { name: "Register Team Member" })).toBeDisabled();
  await expect(hubPanel.getByRole("button", { name: "Clear Registration" })).toBeDisabled();

  const runtimeHub = await page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    return mod.useNodeStore().settings.hub;
  });
  expect(runtimeHub.mode).toBe("Autonomous");
});

test("disabled RCH hub directory does not expose connected routing states", async ({ page }) => {
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

  await expect(hubPanel.getByLabel("Mode")).toBeDisabled();
  await expect(hubPanel.getByLabel("Mode")).toHaveValue("Autonomous");
  await expect(hubPanel).toContainText("Autonomous");
  await expect(hubPanel).not.toContainText("server forcing connected routing");
  await expect(hubPanel).not.toContainText("outbound blocked");
});

test("plugin settings sections can register after Settings is mounted", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await gotoApp(page, "/settings");

  await page.evaluate(async () => {
    const mod = await import("/src/plugins/pluginSettings.ts");
    mod.registerPluginSettingsSection({
      pluginId: "rem.plugin.example_status",
      name: "Example Status Plugin",
      version: "0.1.0",
      state: "Enabled",
      description: "Host-rendered settings for the example plug-in.",
      fields: [
        {
          id: "destinationHex",
          label: "Destination",
          type: "text",
          defaultValue: "",
          placeholder: "Reticulum destination",
        },
        {
          id: "statusMessage",
          label: "Status message",
          type: "text",
          defaultValue: "Status test from example plug-in",
        },
      ],
      actions: [],
    });
  });

  const pluginPanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "Plugin" }),
  });
  await pluginPanel.locator("summary").click();

  await expect(pluginPanel).toContainText("1 configurable");
  await expect(pluginPanel.getByRole("heading", { name: "Example Status Plugin" })).toBeVisible();
  await expect(pluginPanel.getByText("No plug-ins are installed or enabled")).toHaveCount(0);

  await pluginPanel.getByLabel("Destination").fill("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
  await pluginPanel.getByLabel("Status message").fill("Field report ready");
  await pluginPanel.getByRole("button", { name: "Save Plug-in" }).click();

  const saved = await page.evaluate(() =>
    JSON.parse(
      window.localStorage.getItem(
        "reticulum.mobile.pluginSettings.v1.rem.plugin.example_status",
      ) ?? "{}",
    ),
  );

  expect(saved).toEqual({
    destinationHex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    statusMessage: "Field report ready",
  });
});
