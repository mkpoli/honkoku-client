export function manifestLabel(value: unknown): string {
  if (typeof value === "string") return value;
  if (Array.isArray(value)) {
    const localized = value.filter(
      (v) => v && typeof v === "object" && "@language" in v,
    );
    if (localized.length) {
      const language =
        ["ja", "en"].find((lang) =>
          localized.some((v) => v["@language"] === lang),
        ) ?? localized[0]["@language"];
      return localized
        .filter((v) => v["@language"] === language)
        .map(manifestLabel)
        .filter(Boolean)
        .join("／");
    }
    return value.map(manifestLabel).filter(Boolean).join("／");
  }
  if (!value || typeof value !== "object") return "";
  const map = value as Record<string, unknown>;
  if ("@value" in map) return manifestLabel(map["@value"]);
  return (
    [...new Set(["ja", "en", "none", ...Object.keys(map)])]
      .map((key) => manifestLabel(map[key]))
      .find(Boolean) ?? ""
  );
}
export function manifestLinks(
  value: unknown,
): { url: string; label: string }[] {
  if (!value) return [];
  if (Array.isArray(value)) return value.flatMap(manifestLinks);
  if (typeof value === "string") return [{ url: value, label: value }];
  if (typeof value !== "object") return [];
  const map = value as Record<string, unknown>,
    url = map.id ?? map["@id"];
  return typeof url === "string"
    ? [{ url, label: manifestLabel(map.label) || url }]
    : [];
}
