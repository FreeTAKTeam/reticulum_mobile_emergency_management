import { expect, test } from "@playwright/test";

import { DEFAULT_TCP_COMMUNITY_ENDPOINT } from "../apps/mobile/src/utils/tcpCommunityServers";
import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

async function seedAnnouncedPeers(
  page: import("@playwright/test").Page,
  peers: Array<{
    destination: string;
    appData: string;
    label?: string;
    announcedName?: string;
  }>,
): Promise<void> {
  await page.evaluate(async (entries) => {
    const mod = await import("/src/stores/nodeStore.ts");
    const store = mod.useNodeStore();
    const now = Date.now();
    for (const entry of entries) {
      const isHub = entry.appData.toLowerCase().includes("hub");
      store.announceByDestination[entry.destination] = {
        destinationHex: entry.destination,
        identityHex: entry.destination,
        destinationKind: "app",
        announceClass: isHub ? "RchHubServer" : "PeerApp",
        appData: entry.appData,
        displayName: entry.announcedName ?? entry.label,
        hops: 1,
        interfaceHex: "00000000000000000000000000000000",
        receivedAtMs: now,
      };
      store.discoveredByDestination[entry.destination] = {
        destination: entry.destination,
        label: entry.label,
        announcedName: entry.announcedName,
        appData: entry.appData,
        lastSeenAt: now,
        announceLastSeenAt: now,
        lxmfLastSeenAt: now,
        sources: ["announce"],
        state: "disconnected",
        saved: false,
        stale: false,
        activeLink: false,
      };
    }
  }, peers);
}

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

test("hub selector only lists RCH-capable announce peers and persists the selected hub", async ({ page }) => {
  await seedAppStorage(page, {
    settings: {
      ...defaultSettings,
      hub: {
        ...defaultSettings.hub,
        mode: "SemiAutonomous",
      },
    },
  });

  await gotoApp(page, "/settings");
  await seedAnnouncedPeers(page, [
    {
      destination: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      appData: "R3AKT,rch_hub,name=Relay%20Hub",
      announcedName: "Relay Hub",
    },
    {
      destination: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      appData: "R3AKT,Telemetry,name=Telemetry%20Peer",
      announcedName: "Telemetry Peer",
    },
  ]);

  const hubPanel = page.locator("details").filter({
    has: page.getByRole("heading", { name: "RCH Hub Directory" }),
  });
  await hubPanel.locator("summary").click();
  await hubPanel.getByLabel("Mode").selectOption("SemiAutonomous");

  const hubSelect = hubPanel.getByLabel("Hub from announces (RCH servers)");
  await expect(hubSelect.locator("option")).toHaveCount(2);
  await expect(hubSelect).toContainText("Relay Hub");
  await expect(hubSelect).not.toContainText("Telemetry Peer");

  await hubSelect.selectOption("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
  await page.getByRole("button", { name: "Save" }).click();

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );

  expect(storedSettings.hub.identityHash).toBe("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
  expect(storedSettings.hub.mode).toBe("SemiAutonomous");
});

test("hub summary shows cached peer count, connected override, and missing connected hub state", async ({ page }) => {
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
  await hubPanel.getByLabel("Mode").selectOption("SemiAutonomous");
  await hubPanel.getByLabel("Hub identity hash").fill("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
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

  await expect(hubPanel).toContainText("1 cached peers");
  await expect(hubPanel).toContainText("server forcing connected routing");

  await hubPanel.getByLabel("Mode").selectOption("Connected");
  await hubPanel.getByLabel("Hub identity hash").fill("");

  await expect(hubPanel).toContainText("No hub selected | outbound blocked");
});
