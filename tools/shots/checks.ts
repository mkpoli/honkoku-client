import { checkEditor } from "./editor-checks";
import assert from "node:assert/strict";
import type { Browser, Page } from "playwright";
import { resolve } from "node:path";
const entry = "0916dafb80cdc48ca7687afcad4a4f35";
const collection = "3R4VhlBfvOYeqPY13cJm";
export async function checkInteractions(browser: Browser, origin: string) {
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
  const panel = await page.locator(".transcription").evaluate((e) => ({
    scrollHeight: e.scrollHeight,
    clientHeight: e.clientHeight,
  }));
  assert.ok(
    panel.scrollHeight <= panel.clientHeight + 1,
    "columns wrap instead of overflowing downward",
  );
  await page.locator(".theme-menu summary").click();
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
  await page.locator(".theme-menu summary").click();
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
    const beforeOcr = await raw.inputValue();
    const ocr = await page.locator(".ocr-columns pre").first().textContent();
    await page
      .getByRole("button", { name: "本文に挿入", exact: true })
      .first()
      .click();
    assert.equal(await raw.inputValue(), `${beforeOcr}\n${ocr}`);
    await page.locator(".theme-menu summary").click();
    await page.getByRole("checkbox", { name: "効果音", exact: true }).uncheck();
    assert.equal(
      await page.evaluate(() => localStorage.getItem("honkoku.sound")),
      "off",
    );
    await page.locator(".theme-menu summary").click();
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
  } finally {
    await context.close();
  }
}
