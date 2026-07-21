export const MIN_ANNOUNCE_INTERVAL_SECONDS = 60;
export const PEER_PRESENCE_GRACE_MS = 60_000;

function boundedPositiveInteger(value: number, fallback: number): number {
  if (!Number.isFinite(value) || value <= 0) {
    return fallback;
  }
  return Math.min(0xFFFF_FFFF, Math.max(1, Math.trunc(value)));
}

export function peerPresenceFreshnessMs(
  announceIntervalSeconds: number,
  staleAfterMinutes: number,
): number {
  const intervalSeconds = Math.max(
    MIN_ANNOUNCE_INTERVAL_SECONDS,
    boundedPositiveInteger(announceIntervalSeconds, 1800),
  );
  const configuredStaleMs = boundedPositiveInteger(staleAfterMinutes, 30) * 60_000;
  return Math.max(
    configuredStaleMs,
    intervalSeconds * 1000 + PEER_PRESENCE_GRACE_MS,
  );
}

export function peerHasFreshPresence(options: {
  activeLink: boolean;
  lastSeenAt?: number;
  nowMs: number;
  announceIntervalSeconds: number;
  staleAfterMinutes: number;
}): boolean {
  if (options.activeLink) {
    return true;
  }
  return typeof options.lastSeenAt === "number"
    && Number.isFinite(options.lastSeenAt)
    && options.nowMs - options.lastSeenAt <= peerPresenceFreshnessMs(
      options.announceIntervalSeconds,
      options.staleAfterMinutes,
    );
}
