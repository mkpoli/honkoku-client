import {
  createResponsiveTermScorer,
  loadCorpusDF,
  pageTerms,
} from "../../packages/editor/terms";
import { readdir } from "node:fs/promises";
import { homedir } from "node:os";
import { resolve } from "node:path";

async function* transcriptions(
  directory: string,
  depth = 0,
): AsyncGenerator<string> {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((a, b) => a.name.localeCompare(b.name, "en"));
  for (const entry of entries) {
    if (entry.name === "translations") continue;
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) yield* transcriptions(path, depth + 1);
    else if (depth === 2 && /^\d+\.txt$/.test(entry.name)) yield path;
  }
}
const pages: string[] = [];
for await (const file of transcriptions(
  process.argv[2] ??
    resolve(homedir(), "projects/Linguistics/kuzushiji/honkoku-data/v3"),
)) {
  const text = await Bun.file(file).text();
  if (text.length >= 200) pages.push(text);
  if (pages.length === 100) break;
}
if (pages.length < 100) throw Error("Need 100 transcription pages");
const corpus = await loadCorpusDF();
const slices: number[] = [];
const score = createResponsiveTermScorer((ms) => slices.push(ms));
const start = performance.now();
await score(pages, corpus);
const cold = performance.now() - start;
const initialMaxSliceMs = Math.max(...slices);
const timings: number[] = [];
for (let i = 0; i < 100; i++) {
  const changed = [...pages];
  changed[0] += i % 2 ? "\n松前藩と松前藩" : "\n江戸城と江戸城";
  const began = performance.now();
  await score(changed, corpus);
  timings.push(performance.now() - began);
}
timings.sort((a, b) => a - b);
const pageStart = performance.now();
pageTerms(pages[0]);
const changedPageMs = performance.now() - pageStart;
console.log(
  JSON.stringify(
    {
      pages: pages.length,
      characters: pages.join("").length,
      preparationWallMs: cold,
      initialMaxSliceMs,
      changedPageChars: pages[0].length,
      changedPageMs,
      medianEditMs: timings[50],
      p95EditMs: timings[94],
      maxEditMs: timings[99],
    },
    null,
    2,
  ),
);
if (initialMaxSliceMs >= 50 || timings[99] >= 50) process.exitCode = 1;
