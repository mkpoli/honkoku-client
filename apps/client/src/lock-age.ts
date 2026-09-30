export function lockAge(updatedAt?: string | null, now = Date.now()) {
  const timestamp = Date.parse(updatedAt ?? "");
  const ageMs = Number.isFinite(timestamp)
    ? Math.max(0, now - timestamp)
    : null;
  return { ageMs, idle: ageMs !== null && ageMs > 24 * 60 * 60 * 1000 };
}
