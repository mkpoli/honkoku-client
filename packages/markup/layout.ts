/**
 * Page layout: the own-line 【…】 markers that split a transcription into
 * sections, and the page templates those markers imply for a new page.
 */

/** Inner text of a section marker. Longest alternatives first. */
export const sectionInner =
  "[右左](?:丁|頁)?[上下]段|[右左](?:丁|頁|側|帖)?[・　]?(?:白紙|文字無|文字無し|文字なし)|[右左](?:丁|頁|側|帖)?|[上下中]段";

const sectionWhole = new RegExp(`^(?:${sectionInner})$`);
const sectionLine = new RegExp(`^\\s*【(${sectionInner})】\\s*$`);
export const sectionSplit = new RegExp(`(【(?:${sectionInner})】)`);

/** The label inside a section marker line, or null when the line is ordinary text. */
export function sectionLabel(line: string): string | null {
  return sectionLine.exec(line)?.[1] ?? null;
}

export function sectionLabelsIn(text: string): string[] {
  const labels: string[] = [];
  for (const line of text.split(/\r\n|\r|\n/)) {
    const label = sectionLabel(line);
    if (label !== null) labels.push(label);
  }
  return labels;
}

/** Strip section-marker lines, leaving the body a transcriber would type. */
export function bodyWithoutSections(text: string): string {
  return text
    .split(/\r\n|\r|\n/)
    .filter((line) => sectionLabel(line) === null)
    .join("\n");
}

export function isSectionMarker(token: string): boolean {
  const match = /^【([^【】\n]+)】$/.exec(token);
  return !!match && sectionWhole.test(match[1]);
}

/** 右丁・白紙 and 左丁　文字無 sit in the same slot as 右丁 / 左丁. */
export function sectionSlot(label: string): string {
  return label.split(/[・　]/)[0]!;
}

export interface LayoutTemplate {
  text: string;
  labels: readonly string[];
}

/**
 * Layout families seen on みんなで翻刻 pages (データ v3). A book is in one
 * family; a blank page of that book starts from the family's markers.
 */
export const layoutTemplates: readonly LayoutTemplate[] = [
  { text: "【右丁】\n\n【左丁】\n\n", labels: ["右丁", "左丁"] },
  { text: "【右頁】\n\n【左頁】\n\n", labels: ["右頁", "左頁"] },
  {
    text: "【右頁上段】\n\n【右頁下段】\n\n【左頁上段】\n\n【左頁下段】\n\n",
    labels: ["右頁上段", "右頁下段", "左頁上段", "左頁下段"],
  },
  {
    text: "【右上段】\n\n【右下段】\n\n【左上段】\n\n【左下段】\n\n",
    labels: ["右上段", "右下段", "左上段", "左下段"],
  },
  { text: "【上段】\n\n【下段】\n\n", labels: ["上段", "下段"] },
  { text: "【右側】\n\n【左側】\n\n", labels: ["右側", "左側"] },
  { text: "【右帖】\n\n【左帖】\n\n", labels: ["右帖", "左帖"] },
  { text: "【右】\n\n【左】\n\n", labels: ["右", "左"] },
  { text: "【右丁上段】\n\n【左丁上段】\n\n", labels: ["右丁上段", "左丁上段"] },
];

function bestTemplate(labels: readonly string[]): LayoutTemplate | null {
  const slots = new Set(labels.map(sectionSlot));
  let best: LayoutTemplate | null = null;
  for (const template of layoutTemplates) {
    if (!template.labels.every((label) => slots.has(label))) continue;
    if (!best || template.labels.length > best.labels.length) best = template;
  }
  return best;
}

function isMainPage(text: string): boolean {
  return bodyWithoutSections(text).replace(/\s+/g, "").length > 0;
}

/**
 * 同じ書物の他ページ本文から、新しいページの初期テキストを決める。
 * 本紙の大半が同じ版式の節見出しを持つときだけ、その雛形を返す。
 */
export function suggestPageTemplate(
  siblingTexts: readonly string[],
): string | null {
  const votes = new Map<LayoutTemplate, number>();
  let main = 0;
  for (const text of siblingTexts) {
    const trimmed = text.trim();
    if (!isMainPage(trimmed)) continue;
    main++;
    const template = bestTemplate(sectionLabelsIn(trimmed));
    if (template) votes.set(template, (votes.get(template) ?? 0) + 1);
  }
  if (!main) return null;
  for (const [template, count] of votes)
    if (count > 0 && count * 2 > main) return template.text;
  return null;
}
