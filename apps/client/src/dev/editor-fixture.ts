import pages from "../../../../fixtures/home/pages.json";
import entry from "../../../../fixtures/api/entry-0916dafb80cdc48ca7687afcad4a4f35.json";
import firestore from "../../../../fixtures/api/firestore-pages-0916dafb.json";
import catalog from "../../../../fixtures/home/catalog.json";
import { parse } from "@honkoku/markup";
const candidates: { text: string; title: string; index: number }[] = [];
for (const [id, rows] of Object.entries(pages)) {
  const record = catalog.entries.find((e) => e.id === id);
  const title =
    typeof record?.label === "string"
      ? record.label
      : JSON.stringify(record?.label ?? "");
  if (/蝦夷|藻汐/.test(title))
    for (const row of rows)
      candidates.push({ text: row.text, title, index: row.index });
}
for (const row of entry.transcriptions)
  candidates.push({ text: row.text, title: "蝦夷語箋", index: row.index });
for (const row of firestore)
  if ("document" in row && row.document) {
    const fields = row.document.fields;
    candidates.push({
      text: fields.text?.stringValue ?? "",
      title: "蝦夷語箋",
      index: Number(fields.index.integerValue),
    });
  }
const score = (text: string) =>
  parse(text).columns.reduce(
    (total, c) =>
      total +
      c.nodes.filter((n) => n.kind !== "text" && n.kind !== "raw").length,
    0,
  );
export const editorFixture = candidates.sort(
  (a, b) => score(b.text) - score(a.text) || b.text.length - a.text.length,
)[0];
