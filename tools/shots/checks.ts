import assert from "node:assert/strict";
import type { Browser, Page } from "playwright";
const entry = "0916dafb80cdc48ca7687afcad4a4f35";
const collection = "3R4VhlBfvOYeqPY13cJm";
export async function checkInteractions(browser: Browser, origin: string) {
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
    locale: "ja-JP",
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  async function route(hash: string, selector: string) {
    await page.goto(origin + "/" + hash);
    await page.locator(selector).first().waitFor();
  }
  await route("#/", ".project-row");
  const projects = page.locator(".project-list");
  await projects.getByRole("button", { name: "ユーザー", exact: true }).click();
  await page
    .getByRole("textbox", { name: "全プロジェクトを検索", exact: true })
    .fill("アイヌ");
  await page
    .getByRole("link", { name: "アイヌ関連資料", exact: false })
    .first()
    .waitFor();
  assert.equal(await projects.locator(".project-row").count(), 1);
  assert.equal(
    await page
      .getByRole("textbox", { name: "プロジェクトを検索", exact: true })
      .inputValue(),
    "アイヌ",
  );
  await projects
    .getByRole("combobox", { name: "グループ", exact: true })
    .selectOption("keyword");
  assert.equal(await projects.locator(".group-heading").count(), 3);
  await projects.locator(".group-heading").first().click();
  assert.equal(await projects.locator(".project-row").count(), 2);
  await page.getByRole("button", { name: "ログアウト", exact: true }).click();
  await page.waitForFunction(
    () =>
      document.querySelector<HTMLButtonElement>(
        ".project-list .chips button:last-child",
      )?.disabled,
  );
  assert.equal(
    await projects
      .getByRole("button", { name: "参加中", exact: true })
      .isDisabled(),
    true,
  );
  await page.getByRole("button", { name: "ログイン", exact: true }).click();
  await page.locator(".own-record").waitFor();
  await page
    .getByRole("textbox", { name: "全プロジェクトを検索", exact: true })
    .fill("");
  const feed = page.locator(".home-grid>.timeline-panel");
  await page.waitForFunction(
    () =>
      document.querySelectorAll(".home-grid>.timeline-panel .activity")
        .length === 20,
  );
  await feed.getByRole("button", { name: "さらに表示", exact: false }).click();
  await page.waitForFunction(
    () =>
      document.querySelectorAll(".home-grid>.timeline-panel .activity")
        .length === 40,
  );
  await page
    .locator(".ranking")
    .getByRole("button", { name: "今日", exact: true })
    .click();
  assert.equal(
    await page.locator(".ranking .caption").first().textContent(),
    "読み込んだ活動から集計",
  );
  await route(`#/projects/ainu/collections/${collection}`, ".entry-row");
  assert.equal(await page.locator(".entry-row").count(), 3);
  await page.waitForFunction(
    () => document.querySelectorAll(".breadcrumb a").length === 3,
  );
  assert.equal(
    await page.locator(".breadcrumb a").nth(2).textContent(),
    "蝦夷方言藻汐草",
  );
  assert.equal(
    await page
      .getByRole("textbox", { name: "全プロジェクトを検索", exact: true })
      .isDisabled(),
    true,
  );
  await page
    .locator(".project-tabs")
    .getByRole("button", { name: "概要", exact: true })
    .click();
  await page.locator(".markdown").waitFor();
  await page
    .locator(".project-tabs")
    .getByRole("button", { name: "お知らせ", exact: true })
    .click();
  assert.ok(
    (await page.locator(".project-prose").textContent())?.includes("準備中"),
  );
  await route(`#/entries/${entry}`, ".page-card");
  assert.equal(await page.locator(".page-card").count(), 18);
  assert.equal(await page.locator(".breadcrumb a").count(), 4);
  await page.locator(".page-card").nth(3).click();
  await page.locator(".workbench").waitFor();
  await page.keyboard.press("ArrowRight");
  assert.ok(page.url().endsWith("/pages/4"));
  await page.keyboard.press("Home");
  assert.ok(page.url().endsWith("/pages/0"));
  await page.keyboard.press("End");
  assert.ok(page.url().endsWith("/pages/17"));
  await page.keyboard.press("ArrowRight");
  assert.ok(page.url().endsWith("/pages/17"));
  await page
    .getByRole("button", { name: "左右を入れ替え", exact: false })
    .click();
  assert.equal(await page.locator(".workbench-panes.swapped").count(), 1);
  await page.getByRole("button", { name: "縦書き", exact: false }).click();
  assert.equal(await page.locator(".transcription.horizontal").count(), 1);
  await page.locator(".status-strip a").nth(3).focus();
  await page.locator(".filmstrip.expanded").waitFor();
  await page.locator(".filmstrip-thumbnails a").nth(3).click();
  await page.waitForURL("**/pages/3");
  await page.getByRole("button", { name: "横書き", exact: false }).click();
  const metrics = await page
    .locator(".transcription-column")
    .first()
    .evaluate((e) => ({
      mode: getComputedStyle(e).writingMode,
      whiteSpace: getComputedStyle(e).whiteSpace,
      width: e.getBoundingClientRect().width,
      height: e.getBoundingClientRect().height,
    }));
  assert.equal(metrics.mode, "vertical-rl");
  assert.equal(metrics.whiteSpace, "pre-wrap");
  assert.ok(Math.round(metrics.width) >= 44);
  assert.ok(metrics.height > 0);
  const panel = await page
    .locator(".transcription")
    .evaluate((e) => ({
      scrollHeight: e.scrollHeight,
      clientHeight: e.clientHeight,
    }));
  assert.ok(panel.scrollHeight <= panel.clientHeight + 1, "columns wrap instead of overflowing downward");
  await page.getByRole("combobox", { name: "表示テーマ" }).selectOption("dark");
  assert.equal(await page.locator("html").getAttribute("data-theme"), "dark");
  assert.equal(
    await page
      .locator(".facsimile-canvas")
      .evaluate((e) => getComputedStyle(e).backgroundColor),
    "rgb(27, 24, 22)",
  );
  await page
    .getByRole("combobox", { name: "表示テーマ" })
    .selectOption("system");
  await page.emulateMedia({ colorScheme: "light" });
  assert.equal(
    await page
      .locator(".facsimile-canvas")
      .evaluate((e) => getComputedStyle(e).backgroundColor),
    "rgb(244, 242, 238)",
  );
  await page.evaluate(async (id) => {
    const { fixtureInvoke } = await import("/src/dev/fixtures.ts");
    const pages = (await fixtureInvoke("list_pages", { entryId: id })) as {
      index: number;
      text: string;
      notes: unknown[];
      ocr: unknown;
    }[];
    const p = pages.find((p) => p.index === 3)!;
    p.text =
      "【右丁】\n《振り仮名：峰｜みね》＃1\n【左丁】\n《割書：一｜二》讀＿レ￣ム";
    p.notes = [{ content: "欄外の注記" }];
    p.ocr = { minna: "認識した本文" };
    location.hash = "#/";
  }, entry);
  await page.locator(".home-grid").waitFor();
  await page.evaluate(
    (id) => (location.hash = `#/entries/${id}/pages/3`),
    entry,
  );
  await page.locator(".markup-reference").waitFor();
  await page.getByRole("button", { name: "注記1", exact: true }).click();
  assert.equal(await page.locator(".note.highlighted").count(), 1);
  assert.ok(
    (await page.locator(".ocr-panel").textContent())?.includes("認識した本文"),
  );
  await page
    .locator(".column-label")
    .getByText("右丁", { exact: true })
    .click();
  assert.equal(await page.locator(".half-selected").count(), 1);
  assert.deepEqual(errors, []);
  await context.close();
  const fallbackContext = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
  });
  const fallback = await fallbackContext.newPage();
  await fallback.route("**/*info.json", (route) => route.abort());
  await fallback.goto(origin + `/#/entries/${entry}/pages/3`);
  await fallback.locator(".plain-image img").waitFor();
  assert.ok(await fallback.locator(".plain-image img").getAttribute("src"));
  await fallbackContext.close();
  console.log(
    "Interaction checks passed: filters, grouping, session, pagination, ranking, routes, page keys, layout, themes, notes, OCR, image fallback.",
  );
}
