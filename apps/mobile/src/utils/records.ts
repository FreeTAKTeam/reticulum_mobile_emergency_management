export function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  if (value instanceof Map) {
    const normalized: Record<string, unknown> = {};
    for (const [key, entry] of value.entries()) {
      normalized[String(key)] = entry;
    }
    return normalized;
  }
  return value as Record<string, unknown>;
}
