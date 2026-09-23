/** 見開きの右丁・左丁から始まるページの下書き。 */
export const choPairTemplate = "【右丁】\n\n【左丁】\n\n";

const halfLine = /^\s*【([右左]丁)】\s*$/;

function halfLabels(text: string): string[] {
  const labels: string[] = [];
  for (const line of text.split(/\r\n|\r|\n/)) {
    const label = halfLine.exec(line)?.[1];
    if (label) labels.push(label);
  }
  return labels;
}

export function usesChoPair(text: string): boolean {
  const labels = halfLabels(text);
  return labels.includes("右丁") && labels.includes("左丁");
}

/** 印だけの下書きは本紙の多数決に入れない。 */
function isMainPage(text: string): boolean {
  const body = text
    .split(/\r\n|\r|\n/)
    .filter((line) => !halfLine.test(line))
    .join("\n");
  return body.replace(/\s+/g, "").length > 0;
}

/**
 * 同じ書物の他ページ本文から、新しいページの初期テキストを決める。
 * 本紙の大半が【右丁】【左丁】を持つときだけ、その雛形を返す。
 */
export function suggestPageTemplate(
  siblingTexts: readonly string[],
): string | null {
  const main = siblingTexts.map((text) => text.trim()).filter(isMainPage);
  if (!main.length) return null;
  const paired = main.filter(usesChoPair).length;
  return paired > 0 && paired * 2 > main.length ? choPairTemplate : null;
}
