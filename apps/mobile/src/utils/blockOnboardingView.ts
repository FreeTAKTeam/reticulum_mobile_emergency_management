import type {
  BlockOnboardingInspection,
  BlockPeerTierRecord,
  CircleTier,
} from "@reticulum/node-client";

export function onboardingDestinations(
  inspection: BlockOnboardingInspection,
): string[] {
  return [...new Set([
    inspection.issuerAppDestinationHex,
    ...inspection.trustedDestinationHashes,
  ].map((value) => value.trim().toLowerCase()).filter(Boolean))];
}

export function completePeerTierMap(
  inspection: BlockOnboardingInspection,
  issuerTier: CircleTier,
  overrides: Readonly<Record<string, CircleTier>> = {},
): BlockPeerTierRecord[] {
  const issuerDestinations = new Set([
    inspection.issuerAppDestinationHex,
  ].map((value) => value.trim().toLowerCase()).filter(Boolean));
  return onboardingDestinations(inspection).map((destinationHex) => ({
    destinationHex,
    circleTier: overrides[destinationHex]
      ?? (issuerDestinations.has(destinationHex) ? issuerTier : "outer"),
  }));
}
