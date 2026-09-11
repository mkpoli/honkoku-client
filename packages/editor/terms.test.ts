import { expect, test } from "bun:test";
import { createTermScorer, pageTerms, scoreTerms } from "./terms";
const corpus = { pages: 100, df: { 京都: 99, 江戸: 1 } };

test("TF-IDF combines pages, uses zero for unknown DF, and ranks by score", () => {
  const score = createTermScorer();
  const result = score(["京都と江戸と松前", "京都と江戸と松前"], corpus);
  expect(result.map((t) => t.text)).toEqual(["松前", "江戸", "京都"]);
  expect(result[0].tf).toBe(2);
  expect(result[0].score).toBeCloseTo(2 * Math.log(101));
  expect(result[1].score).toBeCloseTo(2 * Math.log(101 / 2));
});

test("containment uses the maximum immediate longer count and the free threshold", () => {
  const counts = pageTerms("松前藩と松前藩と松前と松前と松前家と松前家");
  expect(scoreTerms(counts, corpus).find((t) => t.text === "松前")?.tf).toBe(6);
  expect(
    scoreTerms(pageTerms("松前藩と松前藩"), corpus).map((t) => t.text),
  ).toEqual(["松前藩"]);
  expect(
    scoreTerms(
      new Map([
        ["松前", 12],
        ["松前藩", 10],
      ]),
      corpus,
    ).some((t) => t.text === "松前"),
  ).toBe(false);
  expect(
    scoreTerms(
      new Map([
        ["松前", 12],
        ["松前藩", 9],
      ]),
      corpus,
    ).some((t) => t.text === "松前"),
  ).toBe(true);
});

test("excludes numeric, unit and date-only terms while keeping names containing them", () => {
  const result = createTermScorer()(
    ["一二三四と両石斗升と年月日と山田と山田と一年と一年".repeat(2)],
    corpus,
  );
  expect(result.map((t) => t.text)).toEqual(["山田"]);
});

test("counts kanji runs of plain text with Unicode characters and no cross-line tokens", () => {
  const counts = pageTerms(
    "《振り仮名：松前｜まつまえ》【朱書】\n松前と𠮷田と𠮷田\n※江戸江戸\n％京都京都",
  );
  expect([...counts]).toEqual([
    ["松前", 2],
    ["𠮷田", 2],
  ]);
});

test("memoized scoring removes changed and departed pages and counts duplicate pages", () => {
  const score = createTermScorer();
  expect(score(["松前", "松前"], corpus)[0].tf).toBe(2);
  expect(score(["江戸", "松前"], corpus)).toEqual([]);
  expect(score(["江戸と江戸"], corpus)[0].text).toBe("江戸");
  expect(score([], corpus)).toEqual([]);
});

test("only the top 24 qualifying terms are returned", () => {
  const counts = new Map(
    Array.from({ length: 40 }, (_, i) => [
      String.fromCodePoint(0x4e80 + i) + "城",
      2,
    ]),
  );
  expect(scoreTerms(counts, corpus)).toHaveLength(24);
});

test("plain-text fast path matches markup cleaning on captured transcription pages", async () => {
  const { plainColumn } = await import("@honkoku/markup");
  const { pageTexts } = await import("../markup/test-fixtures");
  const fixture = await Bun.file(
    new URL(
      "../../fixtures/api/entry-0916dafb80cdc48ca7687afcad4a4f35.json",
      import.meta.url,
    ),
  ).json();
  for (const source of pageTexts(fixture)) {
    const expected = new Map<string, number>();
    for (const line of source.split(/\r\n|\r|\n/))
      for (const match of plainColumn(line).matchAll(/\p{Script=Han}+/gu)) {
        const chars = [...match[0]];
        for (let i = 0; i < chars.length; i++)
          for (let n = 2; n <= 4 && i + n <= chars.length; n++) {
            const text = chars.slice(i, i + n).join("");
            expected.set(text, (expected.get(text) ?? 0) + 1);
          }
      }
    expect(pageTerms(source)).toEqual(expected);
  }
});

test("incremental scores match complete recounts across edits, removals, and DF changes", () => {
  const score = createTermScorer();
  const pages = ["松前藩と松前と最上徳内", "松前藩と最上徳内", "江戸と江戸"];
  for (let i = 0; i < 30; i++) {
    pages[i % pages.length] = [
      "松前藩と松前藩",
      "江戸と最上徳内",
      "かな",
      "松前と松前と松前藩",
      "𠮷田と𠮷田",
    ][i % 5];
    const table = i < 15 ? corpus : { pages: 200, df: { 松前: 100 } };
    const counts = new Map<string, number>();
    for (const page of pages)
      for (const [text, count] of pageTerms(page))
        counts.set(text, (counts.get(text) ?? 0) + count);
    expect(score(pages, table)).toEqual(scoreTerms(counts, table));
  }
});

test("new entries yield between pages and cancelled preparation cannot replace current terms", async () => {
  const { createResponsiveTermScorer } = await import("./terms");
  let cancelled = false;
  let cancelDuringCompute = false;
  let turns = 0;
  const score = createResponsiveTermScorer(() => {
    turns++;
    if (cancelDuringCompute) cancelled = true;
  });
  const first = await score(["松前", "松前"], corpus);
  expect(first?.[0].text).toBe("松前");
  expect(turns).toBe(2);
  cancelDuringCompute = true;
  expect(
    await score(["江戸", "江戸"], corpus, () => cancelled),
  ).toBeUndefined();
  cancelDuringCompute = false;
  expect(await score(["松前", "松前"], corpus)).toEqual(first);
});
