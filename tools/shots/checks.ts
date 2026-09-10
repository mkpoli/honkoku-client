import { checkEditor } from "./editor-checks";
import assert from "node:assert/strict";
import type { Browser, Page } from "playwright";
import { resolve } from "node:path";
const entry = "0916dafb80cdc48ca7687afcad4a4f35";
const collection = "3R4VhlBfvOYeqPY13cJm";
export async function checkInteractions(browser: Browser, origin: string) {
  await checkOcr(browser, origin);
  await checkEditor(browser, origin);
  for (const theme of ["light", "dark"] as const)
    await checkEditing(browser, origin, theme);
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
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "ログアウト", exact: true })
    .click();
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
  await page
    .getByRole("button", { name: "開発用セッションを読み込む", exact: true })
    .click();
  await page.locator(".own-record").waitFor();
  await page
    .getByRole("textbox", { name: "全プロジェクトを検索", exact: true })
    .fill("");
  const feed = page.locator(".home-centre>.timeline-panel");
  await page.waitForFunction(
    () =>
      document.querySelectorAll(".home-centre>.timeline-panel .activity")
        .length === 20,
  );
  await feed.getByRole("button", { name: "さらに表示", exact: false }).click();
  await page.waitForFunction(
    () =>
      document.querySelectorAll(".home-centre>.timeline-panel .activity")
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
  const uncaptured = (
    await import("../../fixtures/api/projects.json")
  ).default.find((p) => p.id !== "ainu")!;
  await route(`#/projects/${uncaptured.id}`, '.entries-panel [role="alert"]');
  assert.match(
    await page.locator('.entries-panel [role="alert"]').innerText(),
    /サンプルデータ/,
  );
  assert.equal(
    await page.locator(".entries-panel code").textContent(),
    "devrun bun run --cwd apps/client tauri dev",
  );
  await route("#/entries/missing-preview-entry", 'main [role="alert"]');
  assert.match(
    await page.locator('main [role="alert"]').innerText(),
    /サンプルデータ/,
  );
  assert.match(
    await page.locator('main [role="alert"]').innerText(),
    /すべての資料/,
  );
  assert.equal(
    await page.locator('main [role="alert"] code').textContent(),
    "devrun bun run --cwd apps/client tauri dev",
  );
  assert.equal(
    await page
      .locator('main [role="alert"]')
      .getByRole("link", { name: "ホームへ" })
      .getAttribute("href"),
    "#/",
  );
  await route(`#/projects/ainu/collections/${collection}`, ".entry-row");
  assert.equal(await page.locator(".entry-row").count(), 3);
  await page.waitForFunction(() => {
    const rows = [...document.querySelectorAll(".collection-row")];
    return (
      rows.length === 67 &&
      rows.every(
        (row) =>
          row.querySelector('[role="progressbar"]') &&
          /[\d,]+／[\d,]+コマ/.test(row.textContent ?? ""),
      )
    );
  });
  assert.equal(
    await page.locator(".collection-row .progress-skeleton").count(),
    0,
  );
  assert.ok(
    !(await page.locator(".collection-list").textContent())?.includes(
      "選択して進捗を表示",
    ),
  );
  for (const row of await page.locator(".collection-row").all()) {
    assert.match(
      await row.locator(".collection-meta").innerText(),
      /[\d,]+資料/,
    );
    assert.match(
      (await row
        .locator('[role="progressbar"]')
        .getAttribute("aria-valuetext")) ?? "",
      /翻刻中[\d,]+コマ/,
    );
    assert.equal(await row.locator(".completed").count(), 1);
    assert.equal(await row.locator(".initiated").count(), 1);
  }
  await page.waitForFunction(
    () => document.querySelectorAll(".entry-statuses").length === 3,
  );
  assert.equal(
    await page.locator(".entries-panel .description").textContent(),
    (await import("../../fixtures/api/collection-3R4VhlBfvOYeqPY13cJm.json"))
      .description,
  );
  assert.ok(
    (await page.locator(".project-header .description").textContent())?.length,
  );

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
  const panel = await page.locator(".transcription").evaluate((e) => ({
    scrollHeight: e.scrollHeight,
    clientHeight: e.clientHeight,
  }));
  assert.ok(
    panel.scrollHeight <= panel.clientHeight + 1,
    "columns wrap instead of overflowing downward",
  );
  await page.getByRole("radio", { name: "ダーク", exact: true }).click();
  assert.equal(await page.locator("html").getAttribute("data-theme"), "dark");
  assert.equal(
    await page
      .locator(".facsimile-canvas")
      .evaluate((e) => getComputedStyle(e).backgroundColor),
    "rgb(27, 24, 22)",
  );
  await page.getByRole("radio", { name: "システム", exact: true }).click();
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
  assert.equal(await page.locator(".note-popover .note").count(), 1);
  assert.match(await page.locator(".note-popover").innerText(), /欄外の注記/);
  await page.getByRole("button", { name: "OCR", exact: true }).click();
  assert.ok(
    (await page.locator(".ocr-panel").textContent())?.includes("ローカルOCR"),
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

export async function checkEditing(
  browser: Browser,
  origin: string,
  theme: "light" | "dark",
  suffix = "",
) {
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
    locale: "ja-JP",
    colorScheme: theme,
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  try {
    await page.goto(`${origin}/#/entries/${entry}/pages/3`);
    await page.getByRole("button", { name: "編集開始", exact: true }).click();
    const editor = page.getByRole("textbox", { name: "翻刻本文", exact: true });
    await editor.waitFor();
    await editor.locator(".transcription-column").first().click();
    await page.keyboard.press("Home");
    await page.keyboard.insertText("追記");
    await page
      .locator(".edit-status")
      .filter({ hasText: "下書き保存" })
      .waitFor();
    await page.evaluate(() => document.fonts.ready);
    await page.waitForLoadState("networkidle", { timeout: 20000 });
    await page.screenshot({
      path: resolve(
        import.meta.dir,
        `../../.local/shots/07-editing-${theme}${suffix}.png`,
      ),
    });
    const geometry = await page
      .locator(".editor-scroll")
      .evaluate((element) => {
        const pane = element.getBoundingClientRect();
        const first = element
          .querySelector(".transcription-column")!
          .getBoundingClientRect();
        return {
          right: first.right,
          paneRight: pane.right,
          height: element.clientHeight,
          scrollHeight: element.scrollHeight,
          scrollLeft: element.scrollLeft,
        };
      });
    assert.ok(
      geometry.right <= geometry.paneRight && geometry.height > 80,
      JSON.stringify(geometry),
    );
    assert.ok(
      geometry.scrollHeight <= geometry.height + 1,
      "editor columns fit vertically",
    );
    await page.getByRole("button", { name: "保存", exact: true }).click();
    await page
      .getByRole("checkbox", { name: "このコマを完了", exact: true })
      .check();
    await page
      .getByRole("textbox", { name: "コメント", exact: true })
      .fill("本文を確認");
    await page.getByRole("button", { name: "保存を確定", exact: true }).click();
    await page
      .locator(".completion-toast")
      .filter({ hasText: "2文字" })
      .waitFor();
    assert.ok(
      (
        await page.locator(".status-strip a").nth(3).getAttribute("class")
      )?.includes("completed"),
    );
    assert.equal(await editor.count(), 0);
    if (theme === "light")
      await page.screenshot({
        path: resolve(
          import.meta.dir,
          `../../.local/shots/07-completed-light${suffix}.png`,
        ),
      });
    await page.getByRole("button", { name: "次のコマ", exact: true }).click();
    await page.waitForURL("**/pages/4");
    const previous = await page
      .locator(".status-strip a")
      .nth(4)
      .getAttribute("aria-label");
    await page.getByRole("button", { name: "編集開始", exact: true }).click();
    await page.getByRole("button", { name: "原文表示", exact: true }).click();
    const raw = page.getByRole("textbox", { name: "原文を編集", exact: true });
    await raw.fill("長い行".repeat(150));
    assert.ok(
      await raw.evaluate((e) => e.scrollWidth <= e.clientWidth + 1),
      "raw source wraps long lines",
    );
    await page.keyboard.press("Escape");
    assert.equal(await raw.count(), 1);
    await page.getByRole("button", { name: "破棄", exact: true }).click();
    await page.getByText("変更を破棄しますか", { exact: true }).waitFor();
    await page.getByRole("button", { name: "破棄する", exact: true }).click();
    await page.getByRole("button", { name: "編集開始", exact: true }).waitFor();
    assert.equal(
      await page.locator(".status-strip a").nth(4).getAttribute("aria-label"),
      previous,
    );
    // Navigate before the draft timer fires, then resume the same retained lock.
    await page.getByRole("button", { name: "編集開始", exact: true }).click();
    await page.getByRole("button", { name: "原文表示", exact: true }).click();
    await raw.fill("再開する本文");
    await page.getByRole("button", { name: "次のコマ", exact: true }).click();
    await page.waitForURL("**/pages/5");
    await page.getByRole("button", { name: "前のコマ", exact: true }).click();
    await page.waitForURL("**/pages/4");
    await page.getByRole("button", { name: "編集を再開", exact: true }).click();
    await page.getByRole("button", { name: "原文表示", exact: true }).click();
    assert.equal(await raw.inputValue(), "再開する本文");
    await page.keyboard.press("Control+s");
    await page.getByRole("button", { name: "編集開始", exact: true }).waitFor();
    assert.ok(
      (
        await page.locator(".status-strip a").nth(4).getAttribute("class")
      )?.includes("completed"),
      "shortcut reuses last save options",
    );
    await page.getByRole("button", { name: "編集開始", exact: true }).click();
    await page.getByRole("button", { name: "原文表示", exact: true }).click();
    await raw.fill(
      Array.from({ length: 30 }, (_, i) => `列${i + 1}本文`).join("\n"),
    );
    await page.getByRole("button", { name: "原文表示", exact: true }).click();
    const scroll = page.locator(".editor-scroll");
    const wide = await scroll.evaluate((e) => {
      const pane = e.getBoundingClientRect();
      e.scrollLeft = -e.scrollWidth;
      const last = e
        .querySelector(".transcription-column:last-child")!
        .getBoundingClientRect();
      return {
        scrollWidth: e.scrollWidth,
        width: e.clientWidth,
        left: last.left,
        paneLeft: pane.left,
        right: last.right,
        paneRight: pane.right,
      };
    });
    assert.ok(
      wide.scrollWidth > wide.width,
      "wide document scrolls horizontally",
    );
    assert.ok(
      wide.left >= wide.paneLeft && wide.right <= wide.paneRight,
      JSON.stringify(wide),
    );
    await scroll.evaluate((e) => {
      e.scrollLeft = 0;
    });
    if (suffix) {
      await page.waitForLoadState("networkidle", { timeout: 20000 });
      await page.screenshot({
        path: resolve(
          import.meta.dir,
          `../../.local/shots/07-editing-wide-${theme}${suffix}.png`,
        ),
      });
    }
    await page.getByRole("button", { name: "破棄", exact: true }).click();
    await page.getByRole("button", { name: "破棄する", exact: true }).click();
    await page.getByRole("button", { name: "編集開始", exact: true }).waitFor();
    await page.getByRole("button", { name: "前のコマ", exact: true }).click();
    await page.waitForURL("**/pages/3");
    await page.getByRole("button", { name: "編集開始", exact: true }).click();
    await page.getByRole("button", { name: "原文表示", exact: true }).click();
    await page.getByRole("button", { name: "OCR", exact: true }).click();
    const beforeOcr = await raw.inputValue();
    const ocr = (
      await page.locator(".ocr-line .line-text").allTextContents()
    ).join("\n");
    await page
      .getByRole("button", { name: "本文に挿入", exact: true })
      .first()
      .click();
    assert.equal(await raw.inputValue(), `${beforeOcr}\n${ocr}`);
    await page.getByRole("button", { name: "効果音", exact: true }).click();
    assert.equal(
      await page.evaluate(() => localStorage.getItem("honkoku.sound")),
      "off",
    );
    await page.getByRole("button", { name: "破棄", exact: true }).click();
    await page.getByRole("button", { name: "破棄する", exact: true }).click();
    await page.getByRole("button", { name: "編集開始", exact: true }).waitFor();
    if (theme === "light" && !suffix) {
      const changeLock = async (owner: "other" | "me", syncMode: boolean) => {
        const name = await page.evaluate(
          async ({ entry, owner, syncMode }) => {
            const { fixtureInvoke } = await import("/src/dev/fixtures.ts");
            const me = (await fixtureInvoke("me", {})) as {
              uid: string;
              displayName: string;
            };
            const users = (await fixtureInvoke("home_ranking", {})) as {
              uid: string;
              displayName: string;
            }[];
            const actor =
              owner === "me" ? me : users.find((u) => u.uid !== me.uid)!;
            const pages = (await fixtureInvoke("list_pages", {
              entryId: entry,
            })) as {
              index: number;
              status: string;
              prevStatus: string;
              tempEditedBy: string;
              tempText: string;
              syncMode: boolean;
            }[];
            Object.assign(
              pages.find((p) => p.index === 3)!,
              {
                status: "editing",
                prevStatus: "completed",
                tempEditedBy: actor.uid,
                tempText: "共有前の下書き",
                syncMode,
              },
            );
            location.hash = "#/";
            return actor.displayName;
          },
          { entry, owner, syncMode },
        );
        await page.locator(".home-grid").waitFor();
        await page.evaluate((entry) => {
          location.hash = `#/entries/${entry}/pages/3`;
        }, entry);
        await page.locator(".workbench").waitFor();
        return name;
      };
      const editorName = await changeLock("other", false);
      await page
        .getByText(`他のユーザーが編集中・${editorName}`, { exact: true })
        .waitFor();
      assert.equal(
        await page
          .getByRole("button", { name: "編集開始", exact: true })
          .count(),
        0,
      );
      assert.ok(
        !(await page.locator(".transcription").textContent())?.includes(
          "共有前の下書き",
        ),
      );
      await changeLock("other", true);
      assert.ok(
        (await page.locator(".transcription").textContent())?.includes(
          "共有前の下書き",
        ),
      );
      await changeLock("me", false);
      await page.getByText("この端末以外で編集中", { exact: true }).waitFor();
      await page
        .getByRole("button", { name: "破棄して引き継ぐ", exact: true })
        .click();
      await page.getByRole("button", { name: "原文表示", exact: true }).click();
      await page.evaluate(async () => {
        const { fixtureInvoke } = await import("/src/dev/fixtures.ts");
        await fixtureInvoke("session_clear", {});
      });
      await raw.fill("接続失敗でも残る本文");
      await page
        .locator(".edit-notice")
        .filter({ hasText: "ログインしてください" })
        .waitFor();
      assert.equal(await raw.inputValue(), "接続失敗でも残る本文");
      await page.evaluate(async () => {
        const { fixtureInvoke } = await import("/src/dev/fixtures.ts");
        await fixtureInvoke("session_import", {});
      });
      await page.keyboard.press("Control+s");
      await page
        .getByRole("button", { name: "編集開始", exact: true })
        .waitFor();
      await page.getByRole("button", { name: "編集開始", exact: true }).click();
      await page.getByRole("button", { name: "原文表示", exact: true }).click();
      await page.evaluate(async (entry) => {
        const { fixtureInvoke } = await import("/src/dev/fixtures.ts");
        const pages = (await fixtureInvoke("list_pages", {
          entryId: entry,
        })) as { index: number; status: string }[];
        pages.find((p) => p.index === 3)!.status = "initiated";
      }, entry);
      await raw.fill("競合後も残る本文");
      await page
        .locator(".edit-notice")
        .filter({ hasText: "編集状態が変わりました" })
        .waitFor();
      await page.getByText("端末に残っている本文", { exact: true }).click();
      assert.equal(
        await page
          .getByRole("textbox", { name: "端末に残っている本文", exact: true })
          .inputValue(),
        "競合後も残る本文",
      );
      assert.ok(
        (
          await page.locator(".status-strip a").nth(3).getAttribute("class")
        )?.includes("initiated"),
      );
    }
    assert.deepEqual(errors, []);
    console.log(
      `Editing checks passed (${theme}${suffix}): draft, save, completion, discard, wrapping, navigation, resume, shortcut.`,
    );
  } catch (error) {
    await page.screenshot({
      path: resolve(
        import.meta.dir,
        `../../.local/shots/12-editing-failure${suffix}.png`,
      ),
    });
    console.error(
      "Editing failure:",
      JSON.stringify({
        errors,
        status: await page.locator(".edit-status").allTextContents(),
        notice: await page.locator(".edit-notice").allTextContents(),
      }),
    );
    throw error;
  } finally {
    await context.close();
  }
}

export async function checkAlignment(
  browser: Browser,
  origin: string,
  theme: "light" | "dark",
  suffix = "",
) {
  const fixture = await import("../../fixtures/api/page-minna-ocr.json");
  const { pageLines } = await import("../../packages/client-api/ocr");
  const { alignColumns, transcriptionColumns } =
    await import("../../packages/markup/align");
  const model = pageLines(
    fixture.page as unknown as import("../../packages/client-api/types").Page,
    fixture.canvas,
  );
  const matches = alignColumns(
    transcriptionColumns(fixture.page.text).map((c) => c.text),
    model.lines,
  );
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
    locale: "ja-JP",
    colorScheme: theme,
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  try {
    await page.goto(
      `${origin}/#/entries/${fixture.page.entryId}/pages/${fixture.page.index}`,
    );
    const toggle = page.getByRole("button", { name: "行枠", exact: true });
    await toggle.waitFor();
    await page.waitForFunction(
      (count) => document.querySelectorAll(".line-overlay").length === count,
      model.lines.length,
    );
    assert.equal(await toggle.getAttribute("aria-pressed"), "false");
    assert.equal(await page.locator(".line-overlay:visible").count(), 0);
    await toggle.click();
    assert.equal(
      await page.locator(".line-overlay:visible").count(),
      model.lines.length,
    );
    const third = page.locator('.line-overlay[data-line-index="2"]');
    await third.click();
    const expectedColumn = matches.indexOf(2);
    assert.ok(expectedColumn >= 0);
    assert.equal(
      await page
        .locator(".transcription-reader .alignment-active-column")
        .getAttribute("data-column-index"),
      String(expectedColumn),
    );
    await page.mouse.move(0, 0);
    await page.waitForTimeout(2100);
    assert.equal(
      await page
        .locator(".transcription-reader .alignment-active-column")
        .count(),
      0,
    );
    await third.click();
    await page.getByRole("button", { name: "編集開始", exact: true }).click();
    const editor = page.getByRole("textbox", { name: "翻刻本文", exact: true });
    await editor.waitFor();
    await third.click();
    await page.mouse.move(0, 0);
    await page.waitForFunction(
      (index) =>
        document
          .querySelector(".editor-active-column")
          ?.getAttribute("data-column-index") === String(index),
      expectedColumn,
    );
    assert.equal(
      await page
        .locator(".line-overlay.highlighted")
        .getAttribute("data-line-index"),
      "2",
    );
    const before = await third.boundingBox();
    await page.keyboard.press("ArrowRight");
    await page.waitForFunction(
      (index) =>
        document
          .querySelector(".line-overlay.highlighted")
          ?.getAttribute("data-line-index") === String(index),
      matches[expectedColumn - 1],
    );
    assert.equal(
      await page
        .locator(".editor-active-column")
        .getAttribute("data-column-index"),
      String(expectedColumn - 1),
    );
    assert.deepEqual(
      await third.boundingBox(),
      before,
      "visible lines do not pan the image",
    );
    await page.keyboard.press("ArrowLeft");
    await page.waitForFunction(
      () =>
        document
          .querySelector(".line-overlay.highlighted")
          ?.getAttribute("data-line-index") === "2",
    );
    await page.getByRole("button", { name: "拡大", exact: true }).click();
    await page.waitForTimeout(300);
    const zoomed = await third.boundingBox();
    assert.ok(
      zoomed && before && zoomed.height > before.height,
      "overlays scale with the image",
    );
    for (let i = 0; i < 3; i++)
      await page.getByRole("button", { name: "拡大", exact: true }).click();
    await page.waitForTimeout(300);
    const zoomLabel = await page
      .locator(".zoom-controls .numeric")
      .textContent();
    const lastBefore = await page
      .locator('.line-overlay[data-line-index="9"]')
      .boundingBox();
    await editor.focus();
    for (let i = expectedColumn; i < matches.length - 1; i++)
      await page.keyboard.press("ArrowLeft");
    await page.waitForTimeout(300);
    const lastAfter = await page
      .locator('.line-overlay[data-line-index="9"]')
      .boundingBox();
    const viewport = await page.locator(".osd").boundingBox();
    assert.ok(
      lastBefore &&
        lastAfter &&
        viewport &&
        Math.abs(lastBefore.x - lastAfter.x) > 1,
      "offscreen columns pan the image",
    );
    assert.ok(
      lastAfter.x + lastAfter.width / 2 >= viewport.x &&
        lastAfter.x + lastAfter.width / 2 <= viewport.x + viewport.width,
      "selected line is horizontally visible",
    );
    assert.equal(
      await page.locator(".zoom-controls .numeric").textContent(),
      zoomLabel,
      "caret panning preserves zoom",
    );
    await page.getByRole("button", { name: "全体", exact: true }).click();
    await third.click();
    await page.mouse.move(0, 0);
    await page
      .getByRole("button", { name: "左右を入れ替え", exact: false })
      .click();
    await page.waitForTimeout(300);
    const swapped = await third.boundingBox();
    assert.ok(
      swapped && before && swapped.x < before.x,
      "overlay follows the swapped viewer",
    );
    await page
      .getByRole("button", { name: "左右を入れ替え", exact: false })
      .click();
    await toggle.click();
    assert.equal(
      await page.locator(".line-overlay:visible").count(),
      1,
      "current line remains visible with frames off",
    );
    await toggle.click();
    await third.click();
    await page.mouse.move(0, 0);
    await page.waitForFunction(() => {
      const column = document
        .querySelector(".editor-active-column")
        ?.getBoundingClientRect();
      const pane = document
        .querySelector(".editor-scroll")
        ?.getBoundingClientRect();
      return (
        column && pane && column.left >= pane.left && column.right <= pane.right
      );
    });
    await page.evaluate(() => document.fonts.ready);
    await page.waitForLoadState("networkidle", { timeout: 30000 });
    await page.screenshot({
      path: resolve(
        import.meta.dir,
        `../../.local/shots/08-alignment-${theme}${suffix}.png`,
      ),
    });
    await page.getByRole("button", { name: "破棄", exact: true }).click();
    await page.getByRole("button", { name: "破棄する", exact: true }).click();
    await page.getByRole("button", { name: "次のコマ", exact: true }).click();
    await page.waitForURL(`**/pages/${fixture.page.index + 1}`);
    await page.waitForFunction(() =>
      [...document.querySelectorAll("button")].some(
        (button) => button.textContent === "行枠" && button.disabled,
      ),
    );
    assert.equal(await toggle.isDisabled(), true);
    assert.equal(await page.locator(".line-overlay").count(), 0);
    assert.deepEqual(errors, []);
    console.log(
      `Alignment checks passed (${theme}${suffix}): ${model.lines.length} overlays, ${matches.filter((v) => v !== null).length}/${matches.length} matched columns, selection, caret, zoom, pan, swap, visibility, page cleanup.`,
    );
  } finally {
    await context.close();
  }
}

async function checkOcr(browser: Browser, origin: string) {
  const fixture = await import("../../fixtures/api/ocr-local-0916dafb-3.json");
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
    locale: "ja-JP",
  });
  const page = await context.newPage();
  try {
    await page.goto(`${origin}/#/entries/${entry}/pages/3`);
    await page.getByRole("button", { name: "OCR", exact: true }).click();
    await page.getByText("ローカルOCR · v18", { exact: false }).waitFor();
    assert.equal(await page.locator(".ocr-line").count(), fixture.lines.length);
    await page.waitForFunction(
      (count) => document.querySelectorAll(".line-overlay").length === count,
      fixture.lines.length,
    );
    await page.getByRole("button", { name: "行枠", exact: true }).click();
    assert.equal(
      await page.locator(".line-overlay:visible").count(),
      fixture.lines.length,
    );
    await page
      .getByRole("button", { name: "サイトに保存", exact: true })
      .click();
    await page
      .getByRole("alertdialog", { name: "翻刻サイトにOCR結果を保存しますか" })
      .waitFor();
    await page.getByRole("button", { name: "戻る", exact: true }).click();
    await page.getByRole("button", { name: "編集開始", exact: true }).click();
    const before = await page.locator(".ProseMirror").textContent();
    await page
      .locator(".ocr-line")
      .first()
      .getByRole("button", { name: "挿入", exact: true })
      .click();
    assert.ok(
      (await page.locator(".ProseMirror").textContent())?.includes(
        fixture.lines[0].koji,
      ),
    );
    assert.notEqual(await page.locator(".ProseMirror").textContent(), before);
    await page.getByRole("button", { name: "原文表示", exact: true }).click();
    const text = await page
      .getByRole("textbox", { name: "原文を編集", exact: true })
      .inputValue();
    assert.ok(text.split("\n").includes(fixture.lines[0].koji));
    const raw = page.getByRole("textbox", { name: "原文を編集", exact: true });
    await raw.fill("前の行\n後の行");
    await raw.evaluate((element: HTMLTextAreaElement) =>
      element.setSelectionRange(1, 1),
    );
    await page
      .locator(".ocr-line")
      .first()
      .getByRole("button", { name: "挿入", exact: true })
      .click();
    assert.equal(
      await raw.inputValue(),
      `前の行\n${fixture.lines[0].koji}\n後の行`,
    );

    await page.getByRole("button", { name: "破棄", exact: true }).click();
    await page.getByRole("button", { name: "破棄する", exact: true }).click();
    await page.getByRole("button", { name: "編集開始", exact: true }).waitFor();
    await page.screenshot({
      path: resolve(import.meta.dir, "../../.local/shots/09-ocr-light.png"),
    });
    await page.getByRole("button", { name: "次のコマ", exact: true }).click();
    await page.waitForURL("**/pages/4");
    await page.waitForFunction(
      () => document.querySelectorAll(".ocr-line").length === 0,
    );
    console.log(
      "OCR checks passed: stored fixture, local overlays, confirmation, insertion, page cleanup.",
    );
  } finally {
    await context.close();
  }
}

export async function checkQuietWorkbench(
  browser: Browser,
  origin: string,
  theme: "light" | "dark",
  suffix = "",
) {
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
    locale: "ja-JP",
    colorScheme: theme,
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const button = (name: string) =>
    page.getByRole("button", { name, exact: true });
  const shot = async (name: string) => {
    await page.waitForFunction(async () => {
      const { default: OpenSeadragon } =
        await import("/node_modules/.vite/deps/openseadragon.js");
      const viewer = OpenSeadragon.getViewer(document.querySelector(".osd"));
      return viewer?.world.getItemCount() > 0 && viewer.getFullyLoaded();
    });
    await page.evaluate(
      () =>
        new Promise<void>((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        ),
    );
    await page.mouse.move(0, 0);
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({
      path: resolve(
        import.meta.dir,
        `../../.local/shots/12-${name}-${theme}${suffix}.png`,
      ),
      caret: "initial",
    });
  };
  try {
    await page.goto(`${origin}/#/entries/${entry}/pages/3`);
    await button("編集開始").waitFor();
    await page.waitForLoadState("networkidle");
    assert.equal(
      await page.locator(".topbar input, .topbar .brand").count(),
      0,
    );
    assert.ok(
      !(await page.locator(".topbar").innerText()).includes("みんなで翻刻"),
    );
    assert.equal(await page.locator(".topbar .breadcrumb a").count(), 4);
    assert.match(
      await page.locator(".topbar .page-count").innerText(),
      /4／18コマ/,
    );
    assert.equal(
      await page.locator(".notes-panel, .workbench-supplement").count(),
      0,
    );
    const drawer = page.locator(".ocr-drawer");
    assert.equal(await drawer.isVisible(), false);
    const pane = await page.locator(".transcription-panel").boundingBox();
    assert.ok(pane && pane.height > 700);
    await shot("workbench-quiet");
    await button("OCR").click();
    assert.equal(await drawer.isVisible(), true);
    assert.deepEqual(
      await page.locator(".transcription-panel").boundingBox(),
      pane,
    );
    await page.reload();
    await button("編集開始").waitFor();
    assert.equal(await button("OCR").getAttribute("aria-pressed"), "true");
    await button("OCRを閉じる").click();
    await button("編集開始").click();
    await page
      .locator(".vertical-editor .transcription-column")
      .first()
      .click();
    await page.keyboard.press("Home");
    await shot("editing-quiet");
    await button("原文表示").click();
    const raw = page.getByRole("textbox", { name: "原文を編集", exact: true });
    await raw.fill("前後\n別列");
    await raw.evaluate((element: HTMLTextAreaElement) =>
      element.setSelectionRange(1, 1),
    );
    await button("合字").hover();
    await button("合字ゟ").click();
    assert.equal(await raw.inputValue(), "前ゟ後\n別列");
    assert.equal(await page.locator(".palette-glyphs").isVisible(), true);
    await raw.evaluate((element: HTMLTextAreaElement) =>
      element.setSelectionRange(2, 3),
    );
    await button("振り仮名").click();
    assert.equal(
      await page.getByLabel("親文字", { exact: true }).inputValue(),
      "後",
    );
    await page.getByLabel("読み", { exact: true }).fill("あと");
    await button("挿入").click();
    assert.equal(await raw.inputValue(), "前ゟ《振り仮名：後｜あと》\n別列");
    await raw.fill("前後");
    await raw.evaluate((element: HTMLTextAreaElement) =>
      element.setSelectionRange(1, 1),
    );
    await button("割書").click();
    await page.getByLabel("1行目", { exact: true }).fill("一");
    await page.getByLabel("2行目", { exact: true }).fill("二");
    await button("行を追加").click();
    await page.getByLabel("3行目", { exact: true }).fill("三");
    await button("行を追加").click();
    await page.getByLabel("4行目", { exact: true }).fill("四");
    assert.equal(await button("行を追加").count(), 0);
    await button("挿入").click();
    assert.equal(await raw.inputValue(), "前《割書：一｜二｜三｜四》後");
    await button("原文表示").click();
    await page
      .locator(".editor-warigaki > .editor-segment")
      .nth(1)
      .click({ position: { x: 6, y: 1 } });
    await page.keyboard.press("ArrowDown");
    await page.keyboard.type("x");
    await button("原文表示").click();
    assert.equal(await raw.inputValue(), "前《割書：一｜二x｜三｜四》後");
    await raw.fill("前後");
    await raw.evaluate((element: HTMLTextAreaElement) =>
      element.setSelectionRange(1, 1),
    );
    await button("注記").click();
    await page
      .getByLabel("注記の内容", { exact: true })
      .fill("合字は「より」を表す。");
    await button("追加").click();
    assert.equal(await raw.inputValue(), "前＃1後");
    await button("原文表示").click();
    await button("注記1").hover();
    await page.locator(".note-popover").waitFor();
    assert.match(
      await page.locator(".note-popover").innerText(),
      /合字は「より」を表す。/,
    );
    if (theme === "light" && !suffix)
      await page.screenshot({
        path: resolve(
          import.meta.dir,
          "../../.local/shots/12-note-popover-light.png",
        ),
      });
    await button("注記1").click();
    await page
      .getByLabel("注記の内容", { exact: true })
      .fill("原本の合字は「より」を表す。");
    await button("更新").click();
    await button("原文表示").click();
    assert.equal(await raw.inputValue(), "前＃1後");
    await raw.evaluate((element: HTMLTextAreaElement) =>
      element.setSelectionRange(4, 4),
    );
    await button("注記").click();
    await page.getByLabel("注記の内容", { exact: true }).fill("二つ目の注記");
    await button("追加").click();
    assert.equal(await raw.inputValue(), "前＃1後＃2");
    await button("原文表示").click();
    await button("注記1").click();
    await button("削除").click();
    await button("次のコマ").click();
    await page.waitForURL("**/pages/4");
    await button("前のコマ").click();
    await page.waitForURL("**/pages/3");
    await button("編集を再開").click();
    await button("注記2").focus();
    await page.locator(".note-popover").waitFor();
    assert.match(
      await page.locator(".note-popover").innerText(),
      /二つ目の注記/,
    );
    await page.keyboard.press("Escape");
    await button("保存").click();
    await button("保存を確定").click();
    await button("編集開始").waitFor();
    const saved = await page.evaluate(async (entryId) => {
      const { fixtureInvoke } = await import("/src/dev/fixtures.ts");
      const pages = (await fixtureInvoke("list_pages", { entryId })) as {
        index: number;
        notes: ({
          content: string;
          type: string;
          markdown: string;
          createdBy: string;
          createdAt: string;
          updatedAt: string;
        } | null)[];
      }[];
      return pages.find((p) => p.index === 3)!.notes;
    }, entry);
    assert.equal(saved[0], null);
    assert.equal(saved[1]?.content, "二つ目の注記");
    assert.equal(saved[1]?.type, "note");
    assert.equal(saved[1]?.markdown, saved[1]?.content);
    assert.ok(
      saved[1]?.createdBy && saved[1]?.createdAt && saved[1]?.updatedAt,
    );
    await button("編集開始").click();
    await button("原文表示").click();
    await raw.fill(
      "原本を読む\n《振り仮名：峰｜みね》\n《割書：一行目｜二行目》\nゟ　ヿ　〆",
    );
    await button("注記1件").click();
    assert.match(
      await page.locator(".note-popover").innerText(),
      /二つ目の注記/,
    );
    await button("注記を閉じる").click();
    await button("原文表示").click();
    const editor = page.locator(".vertical-editor");
    await editor.locator(".transcription-column").first().click();
    await page.keyboard.press("Home");
    await editor.focus();
    await page.keyboard.press("ArrowDown");
    assert.equal(
      await editor.evaluate((element) => getComputedStyle(element).caretColor),
      "rgb(217, 89, 54)",
    );
    assert.equal(
      await editor.evaluate(() => window.getSelection()?.isCollapsed),
      true,
    );
    await page.screenshot({
      path: resolve(
        import.meta.dir,
        `../../.local/shots/12-native-caret-${theme}${suffix}.png`,
      ),
      caret: "initial",
    });

    await button("踊り字").focus();
    await page.keyboard.press("Enter");
    await page.keyboard.press("ArrowRight");
    assert.equal(
      await page.evaluate(() =>
        document.activeElement?.getAttribute("aria-label"),
      ),
      "踊り字々",
    );
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("Enter");
    await page.keyboard.press("Escape");
    assert.equal(await page.locator(".palette-glyphs").count(), 0);
    await button("変体仮名").hover();
    if (theme === "light" && !suffix)
      await page.screenshot({
        path: resolve(
          import.meta.dir,
          "../../.local/shots/12-palette-open-light.png",
        ),
      });
    await page.mouse.move(0, 0);
    await button("原文表示").click();
    await raw.fill("一二\n三四");
    await button("原文表示").click();
    const firstColumn = await editor
      .locator(".transcription-column")
      .first()
      .boundingBox();
    assert.ok(firstColumn);
    await page.mouse.click(
      firstColumn.x + firstColumn.width / 2,
      (await editor.boundingBox())!.y +
        (await editor.boundingBox())!.height -
        20,
    );
    await page.keyboard.type("x");
    await button("原文表示").click();
    assert.equal(await raw.inputValue(), "一二x\n三四");
    await button("原文表示").click();
    const lastColumn = await editor
      .locator(".transcription-column")
      .last()
      .boundingBox();
    assert.ok(lastColumn);
    await page.mouse.click(lastColumn.x - 30, lastColumn.y + 30);
    await page.keyboard.type("y");
    await button("原文表示").click();
    assert.equal(await raw.inputValue(), "一二x\n三四\ny");
    await button("破棄").click();
    await button("破棄する").click();
    assert.deepEqual(errors, []);
    console.log(
      `Quiet workbench checks passed (${theme}${suffix}): chrome, drawers, raw constructs, palette keyboard, notes add/edit/delete/save/resume, caret.`,
    );
  } finally {
    await context.close();
  }
}

export async function checkBrowsePolish(
  browser: Browser,
  origin: string,
  theme: "light" | "dark",
) {
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
    locale: "ja-JP",
    colorScheme: theme,
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const button = (name: string) =>
    page.getByRole("button", { name, exact: true });
  try {
    await page.goto(`${origin}/#/projects/ainu/collections/${collection}`);
    await page.waitForFunction(
      () =>
        document.querySelectorAll(".collection-row [role=progressbar]")
          .length === 67,
    );
    const rows = page.locator(".collection-row h3");
    const platform = await rows.allTextContents();
    const sorting = page.locator(".collection-list .sort-control");
    await sorting.getByRole("button", { name: "名前順", exact: true }).click();
    const names = await rows.allTextContents();
    assert.notDeepEqual(names, platform);
    assert.deepEqual(
      names,
      [...platform].sort((a, b) => a.localeCompare(b, "ja")),
    );
    await page.reload();
    await page.waitForFunction(
      () =>
        document.querySelectorAll(".collection-row [role=progressbar]")
          .length === 67,
    );
    assert.deepEqual(await rows.allTextContents(), names);
    await sorting.getByRole("button", { name: "進捗順", exact: true }).click();
    const ratios = () =>
      page
        .locator(".collection-row [role=progressbar]")
        .evaluateAll((nodes) =>
          nodes.map(
            (node) =>
              Number(node.getAttribute("aria-valuenow")) /
              Number(node.getAttribute("aria-valuemax")),
          ),
        );
    const ascending = await ratios();
    assert.deepEqual(
      ascending,
      [...ascending].sort((a, b) => a - b),
    );
    await sorting.getByRole("button", { name: "進捗順", exact: false }).click();
    const descending = await ratios();
    assert.deepEqual(
      descending,
      [...descending].sort((a, b) => b - a),
    );
    await sorting.getByRole("button", { name: "表示順", exact: true }).click();
    assert.deepEqual(await rows.allTextContents(), platform);
    await page.goto(`${origin}/#/entries/${entry}`);
    await page.locator(".page-card").first().waitFor();
    assert.equal(await page.locator(".page-card").count(), 18);
    const filters = page.locator(".page-filters");
    await filters.getByRole("button", { name: /^未着手/ }).click();
    assert.equal(await page.locator(".page-card").count(), 1);
    assert.equal(
      await page.locator(".page-card").getAttribute("data-index"),
      "5",
    );
    await page.reload();
    await page.locator(".page-card").first().waitFor();
    assert.equal(await page.locator(".page-card").count(), 1);
    await filters.getByRole("button", { name: /^完了/ }).click();
    await button("次の未着手へ").click();
    assert.equal(
      await page.evaluate(() => document.activeElement?.id),
      "page-5",
    );
    assert.equal(await page.locator(".page-card").count(), 18);
    await page.mouse.move(0, 0);
    await page.locator(".entry-screen").evaluate((el) => (el.scrollTop = 0));
    await page.evaluate(() => document.fonts.ready);
    await page.waitForLoadState("networkidle");
    await page.screenshot({
      path: resolve(
        import.meta.dir,
        `../../.local/shots/13-entry-status-${theme}.png`,
      ),
    });
    const finished = page.locator(".page-card.completed").first();
    assert.equal(
      await finished
        .locator(".thumbnail")
        .evaluate((el) => getComputedStyle(el).opacity),
      "0.55",
    );
    await finished.hover();
    assert.equal(
      await finished
        .locator(".thumbnail")
        .evaluate((el) => getComputedStyle(el).opacity),
      "1",
    );
    await page.locator("#page-0").click();
    await page.locator(".workbench").waitFor();
    await page.keyboard.press("n");
    await page.waitForURL("**/pages/5");
    await page.keyboard.press("n");
    await page.waitForURL("**/pages/6");
    await page.keyboard.press("n");
    await page.waitForURL("**/pages/5");
    await page.locator(".breadcrumb a").first().click();
    const recent = page.locator(`.recent-row[data-entry-id="${entry}"]`);
    await recent.waitFor();
    assert.match(await recent.innerText(), /コマ6/);
    assert.equal(
      await recent
        .getByRole("link", { name: "続きから", exact: true })
        .getAttribute("href"),
      `#/entries/${entry}/pages/5`,
    );
    await recent.getByRole("link", { name: "続きから", exact: true }).click();
    await page.waitForURL("**/pages/5");
    await page.locator(".workbench").waitFor();
    await page.keyboard.press("Home");
    await page.waitForURL("**/pages/0");
    await page.locator(".breadcrumb a").first().click();
    await recent.waitFor();
    assert.equal(
      await recent
        .getByRole("link", { name: "次の未着手へ", exact: true })
        .getAttribute("href"),
      `#/entries/${entry}/pages/5`,
    );
    if (theme === "light") {
      await page.evaluate(() => document.fonts.ready);
      await page.waitForLoadState("networkidle");
      await page.screenshot({
        path: resolve(
          import.meta.dir,
          "../../.local/shots/13-home-recent-light.png",
        ),
      });
    }
    assert.deepEqual(errors, []);
    console.log(
      `Browse checks passed (${theme}): sorting, persistence, filters, unfinished navigation, recent history.`,
    );
  } finally {
    await context.close();
  }
}
