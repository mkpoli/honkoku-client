/** 見開きの右丁・左丁から始まるページの下書き。 */
export const choPairTemplate = "【右丁】\n\n【左丁】\n\n";

const rightHalf = "【右丁】";
const leftHalf = "【左丁】";

export function usesChoPair(text: string): boolean {
  return text.includes(rightHalf) && text.includes(leftHalf);
}

/** 表紙・扉のような短いラベルと、印だけの下書きは本紙の多数決に入れない。 */
function isMainPage(text: string): boolean {
  const body = text.replaceAll(rightHalf, "").replaceAll(leftHalf, "").trim();
  if (!body) return false;
  return usesChoPair(text) || body.replace(/\s+/g, "").length >= 20;
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
