export type ParsedReplicationEnvelope = {
  kind: string;
  payload: Record<string, unknown>;
};

export function parseReplicationEnvelope(raw: string): ParsedReplicationEnvelope | null {
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return null;
    }

    const payload = parsed as Record<string, unknown>;
    if (typeof payload.kind !== "string") {
      return null;
    }
    const kind = payload.kind.trim();
    if (!kind) {
      return null;
    }
    const normalizedPayload = { ...payload, kind };

    return {
      kind,
      payload: normalizedPayload,
    };
  } catch {
    return null;
  }
}

export function asTrimmedString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function asNumber(value: unknown, fallback: number): number {
  if (
    (typeof value !== "number" && typeof value !== "string")
    || (typeof value === "string" && value.trim().length === 0)
  ) {
    return fallback;
  }
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : fallback;
}
