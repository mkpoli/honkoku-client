import { expect, test } from "bun:test";
import { manifestLabel, manifestLinks } from "./bibliography";
test("IIIF labels prefer Japanese, then English, then any available language", () => {
  expect(
    manifestLabel({ none: ["und"], en: ["English"], ja: ["日本語"] }),
  ).toBe("日本語");
  expect(manifestLabel({ none: ["und"], en: ["English"] })).toBe("English");
  expect(manifestLabel("資料")).toBe("資料");
  expect(manifestLabel({ "@value": "資料", "@language": "ja" })).toBe("資料");
  expect(manifestLabel({ fr: ["texte"] })).toBe("texte");
});
test("IIIF links retain v2 and v3 resources", () => {
  expect(
    manifestLinks([
      { "@id": "https://example.org/a" },
      { id: "https://example.org/b", label: { ja: ["資料"] } },
    ]),
  ).toEqual([
    { url: "https://example.org/a", label: "https://example.org/a" },
    { url: "https://example.org/b", label: "資料" },
  ]);
});
test("IIIF v2 language values and empty preferred languages fall back correctly", () => {
  expect(
    manifestLabel([
      { "@value": "English", "@language": "en" },
      { "@value": "日本語", "@language": "ja" },
    ]),
  ).toBe("日本語");
  expect(manifestLabel({ ja: [], en: ["English"] })).toBe("English");
});
