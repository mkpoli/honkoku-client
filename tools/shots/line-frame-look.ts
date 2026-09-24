import { chromium } from "playwright";
import { resolve } from "node:path";
import { mkdir } from "node:fs/promises";

const origin = process.env.HONKOKU_ORIGIN ?? "http://127.0.0.1:27123";
const output = resolve(import.meta.dir, "../../.local/shots");
await mkdir(output, { recursive: true });
const fixture = await import("../../fixtures/api/page-minna-ocr.json");
const { pageLines } = await import("../../packages/client-api/ocr");
const { alignColumns, transcriptionColumns } = await import(
  "../../packages/markup/align"
);
const model = pageLines(fixture.page as never, fixture.canvas);
const matches = alignColumns(
  transcriptionColumns(fixture.page.text).map((c) => c.text),
  model.lines,
);
const lineIndex = 4;
const column = matches.indexOf(lineIndex);
if (column < 0)
  throw Error(`look fixture line ${lineIndex} is unaligned; pick a matched one`);
const raw = model.lines.find((l) => l.index === lineIndex)!;
console.log("ocr line", raw.x, raw.y, raw.width, raw.height, "column", column);

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 1600, height: 1000 },
  locale: "ja-JP",
  colorScheme: "light",
  reducedMotion: "reduce",
});
await context.addInitScript(() => localStorage.setItem("honkoku.theme", "light"));
const page = await context.newPage();
await page.goto(
  `${origin}/#/entries/${fixture.page.entryId}/pages/${fixture.page.index}?column=${column}`,
);
const line = page.locator(`.line-overlay[data-line-index="${lineIndex}"]`);
await line.waitFor({ state: "visible" });
await page.waitForTimeout(800);
if (process.env.LOOK_ZOOM)
  for (let i = 0; i < Number(process.env.LOOK_ZOOM); i++)
    await page.getByRole("button", { name: "拡大", exact: true }).click();
await page.waitForTimeout(600);
const box = (await line.boundingBox())!;
console.log(
  "overlay box",
  box,
  "aspect",
  (box.height / box.width).toFixed(2),
  "raw aspect",
  (raw.height / raw.width).toFixed(2),
);
// The overlay is the raw OCR box; the drawn frame sits outside it.
const margin = Math.max(60, box.height * 0.3, box.width * 0.3);
const clip = {
  x: Math.max(0, box.x - margin),
  y: Math.max(0, box.y - margin),
  width: Math.min(1600 - Math.max(0, box.x - margin), box.width + 2 * margin),
  height: Math.min(1000 - Math.max(0, box.y - margin), box.height + 2 * margin),
};
await page.screenshot({ path: resolve(output, "line-frame-crop.png"), clip });
await page.screenshot({ path: resolve(output, "line-frame-full.png") });
await browser.close();
console.log("wrote line-frame-crop.png / line-frame-full.png");
