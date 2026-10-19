import { expect, test } from "@playwright/test";

import { DEFAULT_TCP_COMMUNITY_ENDPOINT } from "../apps/mobile/src/utils/tcpCommunityServers";
import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

const GREEK_CALLSIGN_PATTERN = /^(Alpha|Beta|Gamma|Delta|Epsilon|Zeta|Eta|Theta|Iota|Kappa|Lambda|Mu|Nu|Xi|Omicron|Pi|Rho|Sigma|Tau|Upsilon|Phi|Chi|Psi|Omega)\d{3}$/;

test("first start redirects to setup wizard", async ({ page }) => {
  await seedAppStorage(page, {
    setupWizardCompleted: false,
  });

  await page.goto("/dashboard");

  await expect(page).toHaveURL(/\/setup$/);
  await expect(page.getByRole("heading", { name: "Reticulum Emergency Manager" })).toBeVisible();

  const status = await page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    return mod.useNodeStore().status;
  });
  expect(status.running).toBe(false);
  expect(status.name).toBe("");

  await page.getByRole("button", { name: "Start Setup" }).click();
  await expect(page.getByRole("heading", { name: "Identity" })).toBeVisible();
  await expect(page.getByLabel("Call sign")).toHaveValue(GREEK_CALLSIGN_PATTERN);

  const defaultSettings = await page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    return mod.useNodeStore().settings;
  });
  expect(defaultSettings.telemetry.publishIntervalSeconds).toBe(360);
});

test("operators complete first-run setup and persist core choices", async ({ page }) => {
  await seedAppStorage(page, {
    setupWizardCompleted: false,
    settings: {
      ...defaultSettings,
      tcpClients: [DEFAULT_TCP_COMMUNITY_ENDPOINT],
    },
  });

  await gotoApp(page, "/setup");

  await page.getByRole("button", { name: "Start setup" }).click();
  await expect(page.getByRole("heading", { name: "Identity" })).toBeVisible();
  await expect(page.getByText("Step 2 of 7")).toBeVisible();
  await page.getByLabel("Call sign").fill("Atlas-9");
  await page.getByLabel("Household name").fill("Harbour House");
  await page.getByRole("button", { name: "Next" }).click();

  await expect(page.getByRole("heading", { name: "Network interfaces" })).toBeVisible();
  await expect(page.getByText("Step 3 of 7")).toBeVisible();
  await page.getByPlaceholder("Add custom endpoint (host:port or tcp://host:port)").fill("mesh.example.org:5151");
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await expect(page.getByText("mesh.example.org:5151")).toBeVisible();
  await page.getByLabel("Enable RNode Bluetooth LoRa").check();
  await page.getByRole("button", { name: /Pair RNode device/ }).click();
  await expect(page.getByRole("heading", { name: "Pair RNode device" })).toBeVisible();
  await page.getByLabel("RNode device id").fill("AA:BB:CC:DD:EE:FF");
  await page.getByRole("button", { name: "Done" }).click();
  await expect(page.getByRole("button", { name: /Pair RNode device/ })).toContainText("AA:BB:CC:DD:EE:FF");
  await page.getByLabel("Region").selectOption("EU868");
  await page.getByLabel("REM LoRa profile").selectOption("REM-MF-URBAN-v1");
  await page.getByRole("button", { name: "Next" }).click();

  await expect(page.getByRole("heading", { name: "Routing & hubs" })).toBeVisible();
  await expect(page.getByText("Optional", { exact: true })).toBeVisible();
  await expect(page.getByLabel(/^Autonomous/)).toBeChecked();
  await page.getByRole("button", { name: "Next" }).click();

  await expect(page.getByRole("heading", { name: "Location sharing" })).toBeVisible();
  await expect(page.getByText("Location permission")).toBeVisible();
  await page.getByLabel("Enable telemetry sharing").check();
  await expect(page.getByLabel("Telemetry publish interval (seconds)")).toHaveValue("360");
  await page.getByLabel("Telemetry publish interval (seconds)").fill("420");
  await page.getByRole("button", { name: "Next" }).click();

  await expect(page.getByRole("heading", { name: "SOS" })).toBeVisible();
  await page.getByLabel("Enable SOS").check();
  await expect(page.getByLabel("Floating SOS button")).toBeChecked();
  await page.getByRole("button", { name: "Next" }).click();

  await expect(page.getByRole("heading", { name: "Review setup" })).toBeVisible();
  await expect(page.getByText('Atlas-9 · Household "Harbour House"')).toBeVisible();
  await page.getByTestId("setup-finish").click();

  await expect(page).toHaveURL(/\/dashboard$/);

  const storedSettings = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}"),
  );
  const setupState = await page.evaluate(() =>
    JSON.parse(window.localStorage.getItem("reticulum.mobile.setupWizard.v1") ?? "{}"),
  );
  const sosSettings = await page.evaluate(async () => {
    const mod = await import("/src/stores/sosStore.ts");
    const store = mod.useSosStore();
    return {
      enabled: store.settings.enabled,
      floatingButton: store.settings.floatingButton,
    };
  });
  const status = await page.evaluate(async () => {
    const mod = await import("/src/stores/nodeStore.ts");
    return mod.useNodeStore().status;
  });

  expect(storedSettings.displayName).toBe("Atlas-9");
  expect(storedSettings.tcpClients).toContain("mesh.example.org:5151");
  expect(storedSettings.rnode).toEqual({
    enabled: true,
    connectionMode: "ble",
    peripheralId: "AA:BB:CC:DD:EE:FF",
    displayName: "",
    region: "EU868",
    profile: "REM-MF-URBAN-v1",
    frequencyHz: 868_000_000,
  });
  expect(storedSettings.telemetry.enabled).toBe(true);
  expect(storedSettings.telemetry.publishIntervalSeconds).toBe(420);
  expect(storedSettings.community.householdName).toBe("Harbour House");
  expect(storedSettings.community.householdId).toMatch(/^[0-9a-f]{16}$/);
  expect(status.appDestinationHex.toLowerCase().startsWith(storedSettings.community.householdId)).toBe(true);
  expect(status.running).toBe(true);
  expect(status.name).toBe("Atlas-9");
  expect(sosSettings).toEqual({
    enabled: true,
    floatingButton: true,
  });
  expect(setupState.completed).toBe(true);
});

test("setup steps use the Settings validation", async ({ page }) => {
  await seedAppStorage(page, { setupWizardCompleted: false });
  await gotoApp(page, "/setup");

  await page.getByRole("button", { name: "Start setup" }).click();
  await page.getByLabel("Call sign").fill("   ");
  await expect(page.getByRole("alert")).toHaveText("Call sign is required.");
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
  await page.getByLabel("Call sign").fill("Atlas-3");
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Next" }).click();

  await expect(page.getByRole("heading", { name: "Location sharing" })).toBeVisible();
  await page.getByLabel("Telemetry goes stale after (minutes)").fill("90");
  await page.getByLabel("Telemetry disappears after (minutes)").fill("30");
  await expect(page.getByRole("alert")).toHaveText("Hide time must be the same as or longer than the stale time.");
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
});

test("settings can relaunch setup wizard", async ({ page }) => {
  await seedAppStorage(page, {
    settings: defaultSettings,
  });

  await page.goto("/settings");
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  await page.getByRole("button", { name: "Run setup wizard" }).click();
  await expect(page).toHaveURL(/\/setup\?source=settings$/);
});
