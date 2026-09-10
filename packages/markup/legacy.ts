// Character classes from KojiLexer.g4 in the captured site bundle.
const kanji =
  /^[\u2E80-\u2FDF\u3003-\u3007\u3400-\u4DBF\u4E00-\u9FFF\uF900-\uFAFF]+/u;
const hiragana = /^[\u3040-\u309F]+/u;
const katakana = /^[\u30A0-\u30FF\u31F0-\u31FF]+/u;
const hentaikana = /^[\u0030-\u1B12]+/u;
const nonJpChar =
  /^(?:[^\n#\u002D-\u2010\u2015\u2212\u25A0-\u25A1\u300A-\u300B\u3010-\u3011\u3014-\u3015\u3040-\u30FF\u31F0-\u31FF\u3400-\u9FEA＃％（）／＜＞［］｛-｝]|[./])/u;
function nonJp(text: string): string {
  let result = "";
  while (true) {
    const next = nonJpChar.exec(text.slice(result.length));
    if (!next) return result;
    result += next[0];
  }
}
function token(text: string): string {
  const values = [
    nonJp(text),
    ...[kanji, hiragana, katakana, hentaikana].map(
      (re) => re.exec(text)?.[0] ?? "",
    ),
  ];
  return values.sort((a, b) => b.length - a.length)[0];
}
function reading(text: string): boolean {
  if (!text) return false;
  for (const char of text) if (!token(char)) return false;
  return true;
}
export function legacyRuby(
  text: string,
): { size: number; segments?: string[] } | undefined {
  const prefix = text.startsWith("／") ? 1 : 0;
  const rest = text.slice(prefix);
  const base =
    /^【[^【】]+】/u.exec(rest)?.[0] ??
    /^(?:■+|□+)/u.exec(rest)?.[0] ??
    token(rest);
  if (!base || ["＿", "￣", "｜"].includes(base) || rest[base.length] !== "（")
    return;
  const close = rest.indexOf("）", base.length + 1);
  if (close < 0) return { size: text.length };
  const values = rest.slice(base.length + 1, close).split("｜");
  const size = prefix + close + 1;
  if (
    values.length > 2 ||
    !values.every(reading) ||
    (base.startsWith("【") && !reading(base.slice(1, -1)))
  )
    return { size };
  return { size, segments: [base, ...values] };
}
export function legacyTextRun(text: string): string {
  // Consume one full lexer token so a ruby base cannot start halfway through it.
  return token(text).split(/[《》【】■□〓＃※＿￣／（）｜]/u)[0];
}
