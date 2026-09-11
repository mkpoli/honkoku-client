import { expect, test } from "bun:test";
import {
  parse,
  parseLine,
  serialize,
  tokenizeSource,
  notePreview,
} from "./syntax";

import { pageTexts } from "./test-fixtures";
const root = new URL("../../", import.meta.url).pathname;
for (const pattern of [
  "fixtures/api/entry-*.json",
  "fixtures/api/firestore-pages-*.json",
  "fixtures/home/*.json",
]) {
  for (const file of new Bun.Glob(pattern).scanSync({ cwd: root })) {
    test(`lossless fixture ${file}`, async () => {
      for (const text of pageTexts(await Bun.file(root + file).json())) {
        const tree = parse(text);
        expect(serialize(tree)).toBe(text);
        let cursor = 0;
        for (const column of tree.columns)
          for (const leaf of [...column.nodes, column.ending]) {
            expect(leaf.from).toBe(cursor);
            expect(leaf.source).toBe(text.slice(leaf.from, leaf.to));
            cursor = leaf.to;
          }
        expect(cursor).toBe(text.length);
      }
    });
  }
}
test("malformed syntax, empty segments and mixed endings", () => {
  for (const text of [
    "",
    "\r\n\n\r",
    "《",
    "《不明：語》",
    "《割書：a《題：b》｜c》",
    "《振り仮名：｜》",
    "《割書：｜｜｜》",
    "【未閉じ《割書：あ》",
    "𬼂葛\u{E0100}\uD800",
    "※《a\r\n二",
    "一\r\n二\r三\n",
  ])
    expect(serialize(parse(text))).toBe(text);
  expect(parseLine("《割書：a《題：b》｜c》")[0].kind).toBe("warigaki");
  expect(parseLine("《割書：a｜b｜c｜d｜e》")[0].kind).toBe("raw");
});
test("all specified constructs and site legacy ruby", () => {
  expect(
    parseLine(
      "《振り仮名：未｜いまだ｜ズ》《割書：a｜b｜c｜d》《見せ消ち：a｜b》《圏点：x｜m》《右線：x》《題：x》《箱：x》《場所：x》【右丁】【左丁】【注】■□〓＃１２※注",
    ).map((n) => n.kind),
  ).toEqual([
    "ruby",
    "warigaki",
    "misekechi",
    "kenten",
    "rightLine",
    "title",
    "box",
    "place",
    "divider",
    "divider",
    "editorial",
    "gap",
    "gap",
    "gap",
    "reference",
    "comment",
  ]);
  expect(parseLine("未（いまだ｜ズ）")[0]).toMatchObject({
    kind: "ruby",
    segments: ["未", "いまだ", "ズ"],
    form: "legacy",
  });
  for (const text of [
    "説明（日本語）",
    "abc（ruby）",
    "かな（x）",
    "■（x）",
    "【注】（x）",
    "／東京（とうきょう）",
  ])
    expect(parseLine(text)[0].kind).toBe("ruby");
  for (const text of [
    "（説明）",
    "未（）",
    "未（a｜）",
    "未（a＃b）",
    "未（a《b》）",
  ])
    expect(parseLine(text).some((n) => n.kind === "ruby")).toBe(false);
  expect(
    parseLine("ひらカナ（x）").map((n) => [
      n.kind,
      n.segments?.[0] ?? n.source,
    ]),
  ).toEqual([
    ["text", "ひら"],
    ["ruby", "カナ"],
  ]);
  for (const mark of "レ一二三上中下甲乙丙丁天地人")
    expect(parseLine(`＿${mark}`)[0].kind).toBe("return");
});
test("deterministic arbitrary UTF-16 round trips", () => {
  let seed = 42;
  const alphabet = [..."《》【】｜：＿￣＃※\r\n未あズ□", "\uD800", "\uDC00"];
  for (let i = 0; i < 2000; i++) {
    let text = "";
    for (let j = 0; j < i % 100; j++) {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      text += alphabet[seed % alphabet.length];
    }
    expect(serialize(parse(text))).toBe(text);
  }
});

test("compound fields split only at their own separators", () => {
  const source =
    "《割書：《振り仮名：峰｜みね》の字｜《見せ消ち：旧｜《振り仮名：新｜しん》》》";
  expect(parseLine(source)[0]).toMatchObject({
    kind: "warigaki",
    segments: [
      "《振り仮名：峰｜みね》の字",
      "《見せ消ち：旧｜《振り仮名：新｜しん》》",
    ],
  });
  expect(serialize(parse(source))).toBe(source);
  for (const source of [
    "《振り仮名：《割書：一｜二》｜あ》",
    "《割書：《割書：一｜二》｜三》",
  ])
    expect(parseLine(source)[0].kind).toBe(
      source.startsWith("《割書") ? "warigaki" : "ruby",
    );
});

test("an okurigana run ends where the katakana ends, as on the site", () => {
  const kinds = (source: string) =>
    parseLine(source).map((node) => `${node.kind}:${node.source}`);
  expect(kinds("辺￣ニはゑそ")).toEqual([
    "text:辺",
    "okurigana:￣ニ",
    "text:はゑそ",
  ]);
  expect(kinds("讀￣ムー")).toEqual(["text:讀", "okurigana:￣ム", "text:ー"]);
  expect(kinds("地￣へ罷越")).toEqual(["text:地", "raw:￣", "text:へ罷越"]);
});

test("notation spans cover annotation fields, ruby and reading marks losslessly", () => {
  const source = "《割書：一行｜二行》【ハヵ】未（いまだ｜ズ）讀＿レ￣ム＃1■□";
  const spans = tokenizeSource(source);
  expect(spans.map(({ kind, source }) => [kind, source])).toEqual([
    ["punctuation", "《"],
    ["label", "割書"],
    ["punctuation", "："],
    ["warigaki", "一行"],
    ["punctuation", "｜"],
    ["warigaki", "二行"],
    ["punctuation", "》"],
    ["punctuation", "【"],
    ["note", "ハヵ"],
    ["punctuation", "】"],
    ["text", "未"],
    ["punctuation", "（"],
    ["ruby-reading", "いまだ"],
    ["punctuation", "｜"],
    ["ruby-reading", "ズ"],
    ["punctuation", "）"],
    ["text", "讀"],
    ["return", "＿レ"],
    ["okurigana", "￣ム"],
    ["reference", "＃1"],
    ["gap", "■"],
    ["gap", "□"],
  ]);
  expect(spans.map((s) => s.source).join("")).toBe(source);
  let offset = 0;
  for (const span of spans) {
    expect(span.from).toBe(offset);
    expect(source.slice(span.from, span.to)).toBe(span.source);
    offset = span.to;
  }
});
test("notation spans preserve nested fields, legacy wrappers and incomplete input", () => {
  for (const source of [
    "《割書：《注記：ハヵ》｜《振り仮名：未｜いまだ》》\r\n【右丁】\r％表紙\n％",
    "〔日本橋〕｛内蔵助｝＜安政二年＞讀｛＿レ｝",
    "《未完\n𬼂葛\u{E0100}\t【未完",
    "／東京（とうきょう）",
    "",
  ]) {
    const spans = tokenizeSource(source);
    expect(spans.map((s) => s.source).join("")).toBe(source);
    for (const span of spans)
      expect(source.slice(span.from, span.to)).toBe(span.source);
  }
  expect(tokenizeSource("【右丁】\n％字下げ一\n％").map((s) => s.kind)).toEqual(
    ["divider", "newline", "block", "newline", "block"],
  );
  expect(
    parseLine("〔日本橋〕｛内蔵助｝＜安政二年＞").map((n) => n.kind),
  ).toEqual(["place", "person", "date"]);
});
test("note previews count graphemes and retain short notes", () => {
  expect(notePreview("ハヵ")).toBe("ハヵ");
  expect(notePreview("一二三四五六七八九十一二")).toBe(
    "一二三四五六七八九十一二",
  );
  expect(notePreview("𬼂葛\u{E0100}か\u3099四五六七八九十一二三")).toBe(
    "𬼂葛\u{E0100}か\u3099四五六…",
  );
});
