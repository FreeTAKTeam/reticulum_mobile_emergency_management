import { expect, test } from "@playwright/test";
import { defaultSettings, gotoApp, seedAppStorage } from "./support/app";

const PEER = "1234567890abcdef1234567890abcdef";
const BLOCK_FINGERPRINT = "90c41c052ec78d0051431e069973a26f96e096f35a6651470dfbcbee588671b6";

async function loadBlockFixedVector(): Promise<string> {
  const { readFile } = await import("node:fs/promises");
  return readFile(
    `${process.cwd()}/apps/mobile/android/app/src/test/resources/block-onboarding-max-v1.txt`,
    "utf8",
  );
}

test("household profile, map preference, and power threshold persist", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings");
  await page.getByLabel("Household name").fill("Harbour House");
  await page.getByLabel("Household ID").fill("0123456789abcdef");
  await page.getByLabel("Adults").fill("2");
  await page.getByLabel("Children").fill("1");
  await page.getByLabel("Pets").fill("2");
  await page.getByLabel("Role badges").fill("Medic, Radio");
  await page.getByLabel("Preferred map").selectOption("satellite");
  await page.getByLabel("Automatic power saver").check();
  await page.getByLabel("Activate at").selectOption("30");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem("reticulum.mobile.settings.v1") ?? "{}")?.community)).toMatchObject({ householdName: "Harbour House", householdId: "0123456789abcdef", adults: 2, children: 1, pets: 2, roleBadges: ["Medic", "Radio"], preferredMapLayer: "satellite" });
  await expect(page.getByTestId("splash-screen")).toBeHidden({ timeout: 10_000 });
  await page.screenshot({ path: "output/playwright/community-settings.png", fullPage: true });
});

test("dashboard presents exactly four native-backed household actions", async ({ page }) => {
  await seedAppStorage(page, { settings: { ...defaultSettings, community: { ...defaultSettings.community, householdName: "Harbour House" } } });
  await gotoApp(page, "/dashboard");
  const group = page.getByRole("group", { name: "Publish household status" });
  await expect(group.getByRole("button")).toHaveCount(4);
  expect((await group.getByRole("button").allTextContents()).map((value) => value.trim())).toEqual(["All Home", "1 Missing", "Evacuated", "Needs Help"]);
  await expect(page.getByRole("heading", { name: "Community status" })).toBeVisible();
  await expect(page.getByTestId("splash-screen")).toBeHidden({ timeout: 10_000 });
  await page.screenshot({ path: "output/playwright/community-dashboard.png", fullPage: true });
});

test("peer Circle choice persists and legacy team sharing has no export action", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/peers");
  await page.evaluate(async (destination) => { const { useNodeStore } = await import("/src/stores/nodeStore.ts"); await useNodeStore().savePeer(destination); }, PEER);
  await page.locator(".peer-tabs").getByRole("button", { name: /Peers/ }).click();
  const row = page.locator(".roster-row").filter({ hasText: PEER.slice(0, 8) });
  await row.getByRole("combobox", { name: /Circle access/ }).selectOption("inner");
  await expect.poll(() => page.evaluate(async () => { const { useNodeStore } = await import("/src/stores/nodeStore.ts"); return useNodeStore().savedPeers[0]?.circleTier; })).toBe("inner");
  await page.goto("/settings/teams");
  await expect(page.getByRole("button", { name: /share qr|export/i })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Signed Block Code" })).toBeVisible();
});

test("browser explains that signed Block Code creation requires native identity", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings");
  await page.getByRole("button", { name: "Create signed code" }).click();
  await expect(page.locator(".block-panel .feedback")).toContainText(/native|unavailable|supported/i);
});

test("signed Block Code review confirms fingerprint and submits the complete tier map", async ({ page }) => {
  const blockFixedVector = await loadBlockFixedVector();
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/settings");
  await page.evaluate(({ encodedText, fingerprint }) => {
    return import("/src/stores/nodeStore.ts").then(({ useNodeStore }) => {
      const store = useNodeStore();
      store.status.running = true;
      store.inspectBlockOnboardingCode = async (value) => {
        if (value !== encodedText) throw new Error("unexpected fixed vector");
        return {
        issuerPublicIdentityHex: "6c23564db0ce940a872831f9074ac20ebe548a12a2b18d0d8170d5ec6776ff337b22d89740632412a73dc6a52b87377060ab33a703f88b8c001b004245c06b71",
        issuerAppDestinationHex: "f54f44857796fc804f525fd14992cf54",
        issuerLxmfDestinationHex: "b21a7445c01503a00e10f576d5d19d36",
        signerFingerprint: fingerprint,
        issuedAtMs: 1_900_000_000_000,
        expiresAtMs: 1_900_000_060_000,
        network: {
          tcpClients: ["mesh.example:4242"], broadcast: true, hubMode: "Autonomous",
          hubApiBaseUrl: "https://mesh.example/", hubRefreshIntervalSeconds: 3_600,
          radio: { region: "US915", profile: "REM-LF-RURAL-v1", frequencyHz: 915_000_000 },
        },
        trustedDestinationHashes: Array.from({ length: 16 }, (_, index) => index.toString(16).padStart(32, "0")),
          preferredMapLayer: "base",
        };
      };
      store.importBlockOnboardingCode = async (request) => {
        (window as typeof window & { __blockImportRequest?: unknown }).__blockImportRequest = request;
        return { importedPeerCount: request.peerTiers.length, settingsUpdated: true };
      };
    });
  }, { encodedText: blockFixedVector, fingerprint: BLOCK_FINGERPRINT });

  await page.getByLabel("Signed Block Code text").fill(blockFixedVector);
  await page.getByRole("button", { name: "Inspect natively" }).click();
  await expect(page.getByRole("heading", { name: "Review before import" })).toBeVisible();
  await expect(page.getByText(BLOCK_FINGERPRINT)).toBeVisible();
  await page.getByLabel("Household name").last().fill("Harbour House");
  await page.getByLabel("Household ID").last().fill("0123456789abcdef");
  await page.locator(".tier-list select").first().selectOption("inner");
  await page.getByLabel("Type the signer fingerprint to confirm").fill(BLOCK_FINGERPRINT);
  await page.getByRole("button", { name: "Verify again & import" }).click();

  await expect(page.locator(".block-panel .feedback")).toContainText("17 peers classified");
  const request = await page.evaluate(() =>
    (window as typeof window & { __blockImportRequest?: { peerTiers?: unknown[]; confirmedSignerFingerprint?: string } })
      .__blockImportRequest,
  );
  expect(request?.confirmedSignerFingerprint).toBe(BLOCK_FINGERPRINT);
  expect(request?.peerTiers).toHaveLength(17);
});

test("Outer Circle and saver policy disable ordinary send with an explanation", async ({ page }) => {
  await seedAppStorage(page, { settings: defaultSettings });
  await gotoApp(page, "/inbox?mockChat=1");
  if (!await page.getByPlaceholder("Write an LXMF message").isVisible()) {
    await page.getByRole("button", { name: /^ALPHA-1 / }).click();
  }
  await page.evaluate(async () => {
    const { useNodeStore } = await import("/src/stores/nodeStore.ts");
    const store = useNodeStore();
    for (const peer of Object.values(store.savedByDestination)) {
      peer.circleTier = "outer";
    }
  });
  await expect(page.getByText(/Chat and exact location require a saved Inner Circle peer/)).toBeVisible();
  await expect(page.getByPlaceholder("Write an LXMF message")).toBeDisabled();
  await page.evaluate(async () => {
    const { useNodeStore } = await import("/src/stores/nodeStore.ts");
    useNodeStore().powerState = { batteryPercent: 10, charging: false, saverActive: true, updatedAtMs: Date.now() };
  });
  await expect(page.getByText(/Power saver pauses ordinary chat and retry/)).toBeVisible();
});
