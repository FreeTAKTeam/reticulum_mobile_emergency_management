export function safeTrim(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function safeLower(value: unknown): string {
  return safeTrim(value).toLowerCase();
}

export function routeQueryString(value: unknown): string {
  return Array.isArray(value) ? safeTrim(value[0]) : safeTrim(value);
}

export function normalizedValuesMatch(left: unknown, right: unknown): boolean {
  const normalizedLeft = safeLower(left);
  const normalizedRight = safeLower(right);
  return normalizedLeft.length > 0 && normalizedLeft === normalizedRight;
}
