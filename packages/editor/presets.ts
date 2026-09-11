import corpus from "./corpus.json";
export type PresetGroup = "送り仮名" | "常用字" | "常用句" | "注記";
export const pageNotes = [
  "左頁",
  "左丁",
  "右頁",
  "右丁",
  "上段",
  "中段",
  "下段",
];
const okurigana = corpus.okurigana
  .slice(0, 24)
  .map(({ kana, count }) => ({ text: kana, count }));
for (const text of ["ヲバ", "ガ", "ヘ", "ノミ", "タル", "タリ", "ナル", "レバ"])
  if (!okurigana.some((p) => p.text === text))
    okurigana.push({
      text,
      count: corpus.okurigana.find((p) => p.kana === text)?.count ?? 0,
    });
export const presets = {
  注記: [
    ...pageNotes.map((text) => ({
      text,
      count: corpus.notes.find((p) => p.text === text)?.count ?? 0,
    })),
    ...corpus.notes.filter((p) => !pageNotes.includes(p.text)),
  ],
  送り仮名: okurigana,
  常用字: corpus.kanji
    .slice(0, 40)
    .map(({ char, count }) => ({ text: char, count })),
  常用句: corpus.expressions.slice(0, 48),
};
export function isPresetGroup(label: string): label is PresetGroup {
  return label in presets;
}
export function normalizePreset(
  group: PresetGroup,
  value: string,
): string | null {
  let text = value.trim().normalize("NFC");
  if (group === "送り仮名")
    text = text.replace(/[ぁ-ゖ]/gu, (c) =>
      String.fromCodePoint(c.codePointAt(0)! + 0x60),
    );
  const pattern =
    group === "注記"
      ? /^[^【】\r\n]{1,12}$/u
      : group === "送り仮名"
        ? /^[ァ-ヶ]{1,8}$/u
        : group === "常用字"
          ? /^\p{Script=Han}$/u
          : /^\p{Script=Han}{2,8}$/u;
  return pattern.test(text) ? text : null;
}
export const presetKey = (uid: string, group: PresetGroup) =>
  `honkoku.palette.${encodeURIComponent(uid)}.${group}`;
export function loadPresets(
  storage: Pick<Storage, "getItem">,
  uid: string,
  group: PresetGroup,
): string[] {
  try {
    const values: unknown = JSON.parse(
      storage.getItem(presetKey(uid, group)) ?? "[]",
    );
    return Array.isArray(values)
      ? [
          ...new Set(
            values.filter(
              (v): v is string =>
                typeof v === "string" &&
                normalizePreset(group, v) === v &&
                !presets[group].some((p) => p.text === v),
            ),
          ),
        ]
      : [];
  } catch {
    return [];
  }
}
