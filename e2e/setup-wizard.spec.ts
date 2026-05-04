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
  await expect(page.getByTestId("setup-callsign")).toHaveValue(GREEK_CALLSIGN_PATTERN);
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
  await page.getByTestId("setup-callsign").fill("Atlas-9");
  await page.getByRole("button", { name: "Next" }).click();

  await page.getByPlaceholder("host:port").fill("mesh.example.org:5151");
  await page.getByRole("button", { name: "Add TCP endpoint" }).click();
  await expect(page.getByText("mesh.example.org:5151")).toBeVisible();
  await page.getByRole("button", { name: "Next" }).click();

  await page.getByLabel("Activate telemetry").check();
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByLabel("Enable SOS").check();
  await page.getByRole("button", { name: "Next" }).click();
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
  expect(storedSettings.telemetry.enabled).toBe(true);
  expect(status.running).toBe(true);
  expect(status.name).toBe("Atlas-9");
  expect(sosSettings).toEqual({
    enabled: true,
    floatingButton: true,
  });
  expect(setupState.completed).toBe(true);
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
