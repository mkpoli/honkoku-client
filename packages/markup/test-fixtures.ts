export function pageTexts(value: unknown): string[] {
  if (!value || typeof value !== "object") return [];
  if (Array.isArray(value)) return value.flatMap(pageTexts);
  return Object.entries(value).flatMap(([key, v]) => {
    if (["text", "tempText", "prevText"].includes(key)) {
      if (typeof v === "string") return [v];
      if (
        v &&
        typeof v === "object" &&
        "stringValue" in v &&
        typeof v.stringValue === "string"
      )
        return [v.stringValue];
    }
    return pageTexts(v);
  });
}
