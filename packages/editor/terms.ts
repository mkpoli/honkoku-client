import { plainColumn } from "@honkoku/markup";

export interface CorpusDF {
  pages: number;
  df: Record<string, number>;
}
export interface DocumentTerm {
  text: string;
  tf: number;
  score: number;
}
let corpusPromise: Promise<CorpusDF> | undefined;
export function loadCorpusDF(): Promise<CorpusDF> {
  return (corpusPromise ??= import("./corpus-df.json")
    .then((module) => module.default)
    .catch((error) => {
      corpusPromise = undefined;
      throw error;
    }));
}
const compareTerms = new Intl.Collator("ja").compare;
const excluded =
  /^[一二三四五六七八九十百千万拾弐参壱廿卅両石斗升合匁貫年月日]+$/u;
export function pageTerms(source: string): Map<string, number> {
  const counts = new Map<string, number>();
  for (const line of source.split(/\r\n|\r|\n/))
    for (const match of (/[％※《【（＿￣＃]/u.test(line)
      ? plainColumn(line)
      : line
    ).matchAll(/\p{Script=Han}+/gu)) {
      const run = [...match[0]];
      for (let start = 0; start < run.length - 1; start++)
        for (let n = 2; n <= 4 && start + n <= run.length; n++) {
          const text = run.slice(start, start + n).join("");
          counts.set(text, (counts.get(text) ?? 0) + 1);
        }
    }
  return counts;
}
const parts = (text: string) => [
  text.slice(text.codePointAt(0)! > 0xffff ? 2 : 1),
  text.slice(
    0,
    text.charCodeAt(text.length - 1) >= 0xdc00 &&
      text.charCodeAt(text.length - 1) <= 0xdfff
      ? -2
      : -1,
  ),
];
const compare = (a: DocumentTerm, b: DocumentTerm) =>
  b.score - a.score || b.tf - a.tf || compareTerms(a.text, b.text);
function topTerms(candidates: Iterable<DocumentTerm>): DocumentTerm[] {
  const terms: DocumentTerm[] = [];
  for (const term of candidates) {
    if (terms.length === 24 && compare(term, terms[23]) >= 0) continue;
    let position = 0;
    while (position < terms.length && compare(term, terms[position]) >= 0)
      position++;
    terms.splice(position, 0, term);
    if (terms.length > 24) terms.pop();
  }
  return terms;
}
function scoredTerm(
  text: string,
  tf: number,
  containing: number,
  corpus: CorpusDF,
): DocumentTerm | undefined {
  if (tf < 2 || excluded.test(text) || tf - containing < Math.max(2, tf * 0.25))
    return;
  return {
    text,
    tf,
    score: tf * Math.log((corpus.pages + 1) / ((corpus.df[text] ?? 0) + 1)),
  };
}
export function scoreTerms(
  counts: Map<string, number>,
  corpus: CorpusDF,
): DocumentTerm[] {
  const containing = new Map<string, number>();
  for (const [text, tf] of counts) {
    if (tf < 2 || excluded.test(text)) continue;
    for (const part of parts(text))
      containing.set(part, Math.max(containing.get(part) ?? 0, tf));
  }
  const terms: DocumentTerm[] = [];
  for (const [text, tf] of counts) {
    const term = scoredTerm(text, tf, containing.get(text) ?? 0, corpus);
    if (term) terms.push(term);
  }
  return topTerms(terms);
}
/** Retain current pages and rescore only changed terms and their immediate parts. */
export function createTermScorer() {
  let cached = new Map<
    string,
    { counts: Map<string, number>; copies: number }
  >();
  const total = new Map<string, number>();
  const longer = new Map<string, Set<string>>();
  const candidates = new Map<string, DocumentTerm>();
  let previousCorpus: CorpusDF | undefined;
  return (pages: readonly string[], corpus: CorpusDF): DocumentTerm[] => {
    const copies = new Map<string, number>();
    for (const text of pages) copies.set(text, (copies.get(text) ?? 0) + 1);
    const next = new Map<
      string,
      { counts: Map<string, number>; copies: number }
    >();
    const changed = new Set<string>();
    const adjust = (counts: Map<string, number>, delta: number) => {
      if (!delta) return;
      for (const [term, count] of counts) {
        const value = (total.get(term) ?? 0) + count * delta;
        if (value) total.set(term, value);
        else total.delete(term);
        changed.add(term);
        if (excluded.test(term)) continue;
        for (const part of parts(term)) {
          changed.add(part);
          let parents = longer.get(part);
          if (value >= 2) {
            if (!parents) longer.set(part, (parents = new Set()));
            parents.add(term);
          } else if (parents) {
            parents.delete(term);
            if (!parents.size) longer.delete(part);
          }
        }
      }
    };
    for (const [text, count] of copies) {
      const old = cached.get(text);
      const counts = old?.counts ?? pageTerms(text);
      adjust(counts, count - (old?.copies ?? 0));
      next.set(text, { counts, copies: count });
    }
    for (const [text, old] of cached)
      if (!next.has(text)) adjust(old.counts, -old.copies);
    cached = next;
    if (corpus !== previousCorpus)
      for (const text of total.keys()) changed.add(text);
    previousCorpus = corpus;
    for (const text of changed) {
      let containing = 0;
      for (const parent of longer.get(text) ?? [])
        containing = Math.max(containing, total.get(parent) ?? 0);
      const term = scoredTerm(text, total.get(text) ?? 0, containing, corpus);
      if (term) candidates.set(text, term);
      else candidates.delete(text);
    }
    return topTerms(candidates.values());
  };
}

/** Prepare a new entry one page per turn so the palette does not block editing. */
export function createResponsiveTermScorer(
  onCompute?: (milliseconds: number) => void,
) {
  let score = createTermScorer();
  let known = new Set<string>();
  return async (
    pages: readonly string[],
    corpus: CorpusDF,
    cancelled = () => false,
  ): Promise<DocumentTerm[] | undefined> => {
    const newPages = pages.filter((text) => !known.has(text)).length;
    const compute = (
      scorer: ReturnType<typeof createTermScorer>,
      texts: readonly string[],
    ) => {
      const started = performance.now();
      const result = scorer(texts, corpus);
      onCompute?.(performance.now() - started);
      return result;
    };
    if (newPages > 1) {
      const prepared = createTermScorer();
      const prefix: string[] = [];
      let terms: DocumentTerm[] = [];
      for (const text of pages) {
        if (cancelled()) return;
        prefix.push(text);
        terms = compute(prepared, prefix);
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
      }
      if (cancelled()) return;
      score = prepared;
      known = new Set(pages);
      return terms;
    }
    if (cancelled()) return;
    const terms = compute(score, pages);
    known = new Set(pages);
    return terms;
  };
}
