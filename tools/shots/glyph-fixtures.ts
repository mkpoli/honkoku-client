import other from "../../fixtures/api/page-minna-ocr.json";
import otherEntry from "../../fixtures/api/entry-minna-ocr.json";
import entry from "../../fixtures/api/entry-0916dafb80cdc48ca7687afcad4a4f35.json";
import { transcriptionColumns } from "../../packages/markup/align";
import type {
  GlyphAttestation,
  GlyphAttestations,
  Page,
} from "../../packages/client-api/types";
import { glyphCards } from "../../apps/client/src/components/glyph-regions";
const pages: GlyphAttestation[] = [7, 8, 14, 9].map((index, i) => {
  const page = entry.transcriptions.find((p) => p.index === index)!;
  const text = i === 3 ? other.page.text : page.text;
  const occurrences = transcriptionColumns(text).flatMap(
    ({ text: plain }, column) =>
      [...plain].flatMap((c, offset, chars) =>
        c === "候"
          ? [
              {
                column,
                plain,
                offset,
                before: chars.slice(Math.max(0, offset - 20), offset).join(""),
                matched: c,
                after: chars.slice(offset + 1, offset + 21).join(""),
              },
            ]
          : [],
      ),
  );
  return {
    pageId: i === 3 ? other.page.id : page.id,
    entryId: i === 3 ? other.page.entryId : entry.id,
    projectId: entry.projectId,
    index: i === 3 ? other.page.index : index,
    entryLabel: i === 3 ? "ゑとろふ漂流記" : "蝦夷紀行上",
    projectTitle: "アイヌ関連資料",
    canvas:
      i === 3 ? otherEntry.canvases[other.page.index] : entry.canvases[index],
    text,
    ocr: i === 3 ? null : (page as unknown as Page).ocr,
    occurrences,
    error: null,
  };
});
if (!pages[3].occurrences.length)
  throw Error("Unknown-position fixture needs a match.");
const result: GlyphAttestations = {
  total: 4,
  facets: [[entry.projectId, 4]],
  pages,
  firestoreReads: 0,
};
await Bun.write(
  "fixtures/glyphs/attestations-候.json",
  JSON.stringify(result, null, 2) + "\n",
);
const cards = pages.flatMap(glyphCards).filter((c) => c.crop);
if (new Set(cards.map((c) => c.page.pageId)).size !== 3)
  throw Error("All three OCR pages must align.");
const clips = cards
  .slice(0, 3)
  .map((c, i) => ({
    id: `fixture-clip-${i}`,
    entryId: entry.id,
    index: c.page.index,
    uid: "fixture-account",
    reading: ["御座候", "候文", "蝦夷"][i],
    tags: [["候文", "書簡"], ["字形"], ["地名"]][i],
    comment: ["文末のくずし方を比較する。", "筆順の参考。", "資料の地名。"][i],
    isPrivate: i !== 1,
    xywh: c
      .crop!.slice(0, c.crop!.lastIndexOf("/"))
      .split("/")
      .at(-3)!
      .split(",")
      .map(Number),
    uri: c.crop!.replace(/\/[^/]+\/0\/default.jpg$/, "/full/0/default.jpg"),
    transcriptionId: c.page.pageId,
    projectId: entry.projectId,
    createdAt: `2026-09-10T0${3 - i}:00:00Z`,
  }));
await Bun.write(
  "fixtures/glyphs/clips.json",
  JSON.stringify(clips, null, 2) + "\n",
);
console.log(
  `${cards.length} crops from three pages; ${pages[3].occurrences.length} unknown positions`,
);
