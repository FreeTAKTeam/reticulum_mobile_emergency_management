import { DEFAULT_SOS_SETTINGS } from "@reticulum/node-client";
import { expect, test } from "vitest";

import { readSosDraft, sosSettingsForSave, sosSettingsWithoutPin } from "../composables/sosFormDraft";
import { cloneDefaultSettings } from "../stores/nodeSettingsModel";
import { identitySlice, locationSlice, networkSlice, normalizedTcpClients, routingSlice } from "./settingsFormSlices";
import { SETTINGS_PAGE_COPY } from "./settingsPageCopy";

test("normalizedTcpClients trims, drops blanks and de-duplicates", () => {
  expect(normalizedTcpClients([" a:4242 ", "", "a:4242", "b:4242"])).toEqual(["a:4242", "b:4242"]);
});

test("identity slice clamps household counts and merges typed role badges", () => {
  const settings = cloneDefaultSettings();
  const draft = { ...identitySlice.read(settings, "abcd"), displayName: " CORVO ", adults: 99, roleBadgeText: "medic, radio" };
  const patch = identitySlice.patch(draft, settings);
  expect(patch.community?.adults).toBe(20);
  expect(patch.community?.roleBadges).toEqual(expect.arrayContaining(["medic", "radio"]));
  expect(identitySlice.validate?.({ ...draft, displayName: "  " })).toBe("Call sign is required.");
});

test("network, routing and location slices keep their input bounds", () => {
  const settings = cloneDefaultSettings();
  expect(networkSlice.validate?.({ ...networkSlice.read(settings, ""), rnodeFrequencyHz: 1 })).toMatch(/frequency/);
  expect(routingSlice.patch({ ...routingSlice.read(settings, ""), hubRefreshIntervalSeconds: 5 }, settings).hub?.refreshIntervalSeconds).toBe(30);
  const location = { ...locationSlice.read(settings, ""), staleAfterMinutes: 60, expireAfterMinutes: 10 };
  expect(locationSlice.validate?.(location)).toMatch(/Hide time/);
});

test("SOS draft keeps the stored PIN out of the form and back on save", () => {
  const stored = { ...DEFAULT_SOS_SETTINGS, deactivationPinHash: "hash", deactivationPinSalt: "salt" };
  expect(sosSettingsWithoutPin(stored)).not.toHaveProperty("deactivationPinHash");
  const saved = sosSettingsForSave({ ...readSosDraft(stored), deactivationPinHash: "typed" }, stored);
  expect(saved.deactivationPinHash).toBe("hash");
  expect(saved.deactivationPinSalt).toBe("salt");
});

test("every settings page has a title and description", () => {
  for (const copy of Object.values(SETTINGS_PAGE_COPY)) {
    expect(copy.title.trim()).not.toBe("");
    expect(copy.description.trim()).not.toBe("");
  }
});
