import { expect, test } from "bun:test";
import { exportTranscription } from "./export";
const text = "未（いまだ｜ズ）《割書：一行｜二行》讀￣ム｛＿レ｝【注記】□■";
test("text retains source notation and one column per line", () => {
  expect(
    exportTranscription([{ index: 3, text: text + "\r\n次の列" }], "txt"),
  ).toBe(text + "\n次の列");
});
test("TEI maps ruby, warigaki, okuri, kaeriten, annotation and both gaps", () => {
  const xml = exportTranscription([{ index: 3, text }], "xml");
  expect(xml).toContain(
    '<pb n="4"/>\n<lb/><ruby><rb><ruby><rb>未</rb><rt>いまだ</rt></ruby></rb><rt>ズ</rt></ruby>',
  );
  expect(xml).toContain(
    '<note type="wari">一行<milestone unit="wrb"/>二行<milestone unit="wrb"/></note>',
  );
  expect(xml).toContain(
    '讀<note type="okuri">ム</note><metamark function="kaeriten">レ</metamark><!-- 注記 -->',
  );
  expect(xml).toContain(
    '<gap quantity="1" unit="chars" reason="wormhole"/><gap quantity="1" unit="chars" reason="illegible"/>',
  );
});
test("LaTeX reproduces the site's ruby and kanbun macros", () => {
  const tex = exportTranscription([{ index: 3, text }], "tex");
  expect(tex).toContain("\\HidariNakaTn{ \\MigiNakaTn{ 未 }{ いまだ } }{ ズ }");
  expect(tex).toContain(
    "\\sougyou{ 一行 }{ 二行 }讀\\kokana{ ム }{}\\kaeriten{ レ }\\textcolor{red}{【注記】}□■",
  );
});
test("reading marks export equally from short, legacy and bracket forms", () => {
  const marks = [
    "レ",
    ...[..."一二三四五六七八九十上中下甲乙丙丁天地人"].flatMap((mark) => [
      mark,
      `${mark}レ`,
    ]),
  ];
  for (const mark of marks)
    for (const source of [
      `＿${mark}`,
      `｛＿${mark}｝`,
      `《返り点：${mark}》`,
    ]) {
      const pages = [{ index: 0, text: `之${source}人` }];
      expect(exportTranscription(pages, "xml")).toContain(
        `之<metamark function="kaeriten">${mark}</metamark>人`,
      );
      expect(exportTranscription(pages, "tex")).toContain(
        `之\\kaeriten{ ${mark} }人`,
      );
    }
  for (const format of ["xml", "tex"] as const)
    expect(
      exportTranscription([{ index: 0, text: "之《送り仮名：カナ》" }], format),
    ).toBe(exportTranscription([{ index: 0, text: "之￣カナ" }], format));
  const pages = [{ index: 0, text: "之《送り仮名：ト云》" }];
  expect(exportTranscription(pages, "xml")).toContain(
    '之<note type="okuri">ト云</note>',
  );
  expect(exportTranscription(pages, "tex")).toContain("之\\kokana{ ト云 }{}");
});
test("a warigaki without ｜ exports as 双行 with an empty left half", () => {
  const source = "《割書：松前志摩守内》";
  expect(exportTranscription([{ index: 0, text: source }], "tex")).toContain(
    "\\sougyou{ 松前志摩守内 }{  }",
  );
  expect(exportTranscription([{ index: 0, text: source }], "xml")).toContain(
    '<note type="wari">松前志摩守内<milestone unit="wrb"/><milestone unit="wrb"/></note>',
  );
  expect(
    exportTranscription([{ index: 0, text: "《割書：松前志摩守内｜》" }], "xml"),
  ).toContain(
    '<note type="wari">松前志摩守内<milestone unit="wrb"/><milestone unit="wrb"/></note>',
  );
});
test("entry exports sort pages and preserve empty page breaks", () => {
  const pages = [
    { index: 2, text: "< & $" },
    { index: 0, text: "｛人名｝〔場所〕＜日時＞" },
    { index: 1, text: "" },
  ];
  const xml = exportTranscription(pages, "xml");
  expect(xml.match(/<pb n="\d+"\/>/g)).toEqual([
    '<pb n="1"/>',
    '<pb n="2"/>',
    '<pb n="3"/>',
  ]);
  expect(xml).toContain(
    "<persName>人名</persName><placeName>場所</placeName><date>日時</date>",
  );
  expect(xml).toContain("&lt; &amp; $");
  expect(exportTranscription(pages, "tex")).toContain("< \\& \\$");
});
test("TEI block wrappers and comments remain well-formed", () => {
  const xml = exportTranscription(
    [{ index: 0, text: "％表紙\n《題：資料》\n％\n％字下げ二\n【a--b-】\n％" }],
    "xml",
  );
  expect(xml).toContain(
    "<titlePage>\n<lb/><docTitle>資料</docTitle>\n</titlePage>",
  );
  expect(xml).toContain(
    '<div rend="indent(-2)">\n<lb/><!-- a- -b-  -->\n</div>',
  );
});
test("nested semantic wrappers do not split annotations and ruby", () => {
  const xml = exportTranscription(
    [{ index: 0, text: "【｛人物｝】《振り仮名：〔日本橋〕｜にほんばし》" }],
    "xml",
  );
  expect(xml).toContain("<!-- ｛人物｝ -->");
  expect(xml).toContain(
    "<ruby><rb><placeName>日本橋</placeName></rb><rt>にほんばし</rt></ruby>",
  );
});
test("gap quantities count each run of unreadable characters", () => {
  expect(exportTranscription([{ index: 0, text: "□□■■■" }], "xml")).toContain(
    '<gap quantity="2" unit="chars" reason="wormhole"/><gap quantity="3" unit="chars" reason="illegible"/>',
  );
});
