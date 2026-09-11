import { readdir } from "node:fs/promises";
import { homedir } from "node:os";
import { resolve } from "node:path";
import { plainColumn } from "../../packages/markup";

const dump = resolve(
  process.argv[2] ??
    `${homedir()}/projects/Linguistics/kuzushiji/honkoku-data/v3`,
);
const revision = "58ceb1fdcb3d11674f768b94964c1fd0e35156e0";
const git = Bun.spawn(["git", "-C", dump, "rev-parse", "HEAD"], {
  stdout: "pipe",
  stderr: "ignore",
});
if (
  (await new Response(git.stdout).text()).trim() !== revision ||
  (await git.exited) !== 0
)
  throw Error("Unexpected corpus revision");
const started = performance.now();
const files: string[] = [];
async function walk(directory: string, depth = 0) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.name === "translations") continue;
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) await walk(path, depth + 1);
    else if (depth === 2 && entry.isFile() && /^\d+\.txt$/.test(entry.name))
      files.push(path);
  }
}
await walk(dump);
const okurigana = new Map<string, number>();
const kanji = new Map<string, number>();
const notes = new Map<string, number>();
const df = new Map<string, number>();
const grams = Array.from({ length: 8 }, () => new Map<string, number>());
const increment = (map: Map<string, number>, key: string) =>
  map.set(key, (map.get(key) ?? 0) + 1);
const excluded =
  /[一二三四五六七八九十百千万拾弐参壱廿卅両石斗升合匁貫年月日々]/u;
async function scan(consume: (text: string) => void) {
  let cursor = 0;
  await Promise.all(
    Array.from({ length: 64 }, async () => {
      while (cursor < files.length)
        consume(await Bun.file(files[cursor++]).text());
    }),
  );
}
function runs(text: string): string[][] {
  return text
    .split(/\r\n|\r|\n/)
    .flatMap((line) =>
      [...plainColumn(line).matchAll(/\p{Script=Han}+/gu)].map((match) => [
        ...match[0],
      ]),
    );
}
await scan((text) => {
  for (const match of text.matchAll(/【([^【】\n]{1,12})】/gu))
    increment(notes, match[1]);
  const seen = new Set<string>();
  const cleanedRuns = runs(text);
  for (const run of cleanedRuns)
    for (let start = 0; start < run.length - 1; start++)
      for (let n = 2; n <= 4 && start + n <= run.length; n++)
        seen.add(run.slice(start, start + n).join(""));
  for (const gram of seen) increment(df, gram);
  for (const match of text.matchAll(/￣([ァ-ヶ]+)/gu))
    increment(okurigana, match[1]);
  for (const run of cleanedRuns)
    for (let i = 0; i < run.length; i++) {
      increment(kanji, run[i]);
      if (i + 1 < run.length) increment(grams[2], run[i] + run[i + 1]);
    }
});
const seeds = new Set(
  [...grams[2]].filter(([, count]) => count >= 200).map(([text]) => text),
);
// The seventh order supplies containment counts for six-character expressions.
await scan((text) => {
  for (const run of runs(text))
    for (let start = 0; start < run.length - 2; start++) {
      if (!seeds.has(run[start] + run[start + 1])) continue;
      let gram = run[start] + run[start + 1];
      for (let end = start + 2; end < Math.min(run.length, start + 7); end++) {
        if (!seeds.has(run[end - 1] + run[end])) break;
        gram += run[end];
        increment(grams[end - start + 1], gram);
      }
    }
});
const compareTerms = new Intl.Collator("ja").compare;
const ordered = (map: Map<string, number>) =>
  [...map].sort((a, b) => b[1] - a[1] || compareTerms(a[0], b[0]));
const characters = ordered(kanji).filter(([char]) => !excluded.test(char));
const top = new Set(characters.slice(0, 200).map(([char]) => char));
const expressions: { text: string; count: number; free: number }[] = [];
for (let n = 2; n <= 6; n++) {
  const containing = new Map<string, number>();
  for (const [text, count] of grams[n + 1]) {
    const chars = [...text];
    for (const part of [chars.slice(0, -1).join(""), chars.slice(1).join("")])
      containing.set(part, Math.max(containing.get(part) ?? 0, count));
  }
  for (const [text, count] of grams[n]) {
    if (
      count < 200 ||
      excluded.test(text) ||
      /衛門|兵衛|郎/u.test(text) ||
      ![...text].every((char) => top.has(char))
    )
      continue;
    const free = count - (containing.get(text) ?? 0);
    if (free >= Math.max(200, count * 0.25))
      expressions.push({ text, count, free });
  }
}
expressions.sort(
  (a, b) =>
    b.free - a.free || b.count - a.count || compareTerms(a.text, b.text),
);
const data = {
  source:
    "みんなで翻刻データ v3, CC BY-SA 4.0, https://github.com/yuta1984/honkoku-data",
  commit: revision,
  pages: files.length,
  notes: ordered(notes)
    .filter(([, count]) => count >= 100)
    .map(([text, count]) => ({ text, count })),
  okurigana: ordered(okurigana)
    .filter(([, count]) => count >= 100)
    .map(([kana, count]) => ({ kana, count })),
  kanji: characters.map(([char, count]) => ({ char, count })),
  expressions,
};
await Bun.write(
  new URL("../../packages/editor/corpus.json", import.meta.url),
  JSON.stringify(data, null, 2) + "\n",
);
const frequentDF: [string, number][] = [];
for (const entry of df) if (entry[1] >= 30) frequentDF.push(entry);
frequentDF.sort((a, b) => b[1] - a[1] || compareTerms(a[0], b[0]));
await Bun.write(
  new URL("../../packages/editor/corpus-df.json", import.meta.url),
  JSON.stringify({
    pages: files.length,
    df: Object.fromEntries(frequentDF),
  }),
);
console.log(
  JSON.stringify(
    {
      pages: files.length,
      kanjiTotal: [...kanji.values()].reduce((a, b) => a + b, 0),
      seconds: (performance.now() - started) / 1000,
      okurigana: data.okurigana.slice(0, 30),
      kanji: data.kanji.slice(0, 40),
      expressions: expressions.slice(0, 48),
    },
    null,
    2,
  ),
);
