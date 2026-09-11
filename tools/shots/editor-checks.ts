import assert from "node:assert/strict";
import type { Browser, Page } from "playwright";
import { mkdir } from "node:fs/promises";
export async function checkEditor(browser: Browser, origin: string) {
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
    deviceScaleFactor: 2,
    locale: "ja-JP",
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  async function press(key: string) {
    await page.keyboard.press(key);
    await equal(page);
  }
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  try {
    await page.goto(origin + "/#/spike/editor");
    await page.locator(".vertical-editor").waitFor();
    await page.evaluate(() => document.fonts.ready);
    const initial = await source(page);
    await page.evaluate(async () => {
      const { createEditor } = await import("/src/dev/editor-harness.ts");
      const host = document.createElement("div");
      document.body.append(host);
      const changes: number[] = [];
      const editor = createEditor(
        host,
        "【右丁】\n春はあけぼの\n\n夏は夜",
        undefined,
        (index) => changes.push(index),
      );
      try {
        editor.focusColumn(1);
        if (
          editor.view.state.selection.$head.index(0) !== 3 ||
          editor.view.state.selection.$head.parentOffset !== 0
        )
          throw Error("focusColumn did not skip blank lines and markers");
        editor.focusColumn(1);
        editor.focusColumn(0);
        if (JSON.stringify(changes) !== JSON.stringify([-1, 1, 0]))
          throw Error(
            `Unexpected column callbacks: ${JSON.stringify(changes)}`,
          );
      } finally {
        editor.destroy();
        host.remove();
      }
    });
    await equal(page);
    assert.equal(
      await page
        .locator(".vertical-editor")
        .evaluate((e) => getComputedStyle(e).writingMode),
      "vertical-rl",
    );
    await reset(page, "一二三\r\n四五六\r七八九\n《未知：原文》");
    await select(page, 1);
    await page.keyboard.type("abc");
    assert.equal(
      await source(page),
      "abc一二三\r\n四五六\r七八九\n《未知：原文》",
    );
    await equal(page);
    await press("ArrowLeft");
    assert.equal(await selectedColumn(page), 1);
    await press("ArrowRight");
    assert.equal(await selectedColumn(page), 0);
    await press("Home");
    await press("ArrowDown");
    assert.equal(await position(page), 2);
    await press("ArrowUp");
    assert.equal(await position(page), 1);
    await press("End");
    assert.equal(await position(page), 7);
    await press("Enter");
    assert.equal(await selectedColumn(page), 1);
    await equal(page);

    await reset(page, "峰\r\n変えない＃００１\r《未知：原文》");
    await select(page, 1, 2);
    await page.getByRole("button", { name: "振り仮名", exact: true }).click();
    await page.keyboard.press("Tab");
    await page.keyboard.insertText("みね");
    assert.equal(
      await source(page),
      "《振り仮名：峰｜みね》\r\n変えない＃００１\r《未知：原文》",
    );
    await equal(page);
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("Control+z");
    assert.equal(await source(page), "峰\r\n変えない＃００１\r《未知：原文》");
    await reset(page, "前後\r\n保存＃００１");
    await select(page, 2);
    const cdp = await context.newCDPSession(page);
    await page.evaluate(() => {
      const root = window.editorSpike!.view.dom;
      const events: string[] = [];
      (window as unknown as { compositionEvents: string[] }).compositionEvents =
        events;
      for (const type of [
        "compositionstart",
        "compositionupdate",
        "compositionend",
      ])
        root.addEventListener(type, () => events.push(type));
      (
        window as unknown as { compositionColumn: Node | null }
      ).compositionColumn = root.firstChild;
    });
    await cdp.send("Input.imeSetComposition", {
      text: "にほん",
      selectionStart: 3,
      selectionEnd: 3,
    });
    await cdp.send("Input.imeSetComposition", {
      text: "日本",
      selectionStart: 2,
      selectionEnd: 2,
    });
    await cdp.send("Input.insertText", { text: "日本" });
    await page.waitForTimeout(100);
    assert.equal(await source(page), "前日本後\r\n保存＃００１");
    assert.equal(
      await page.evaluate(
        () =>
          window.editorSpike!.view.dom.firstChild ===
          (window as unknown as { compositionColumn: Node }).compositionColumn,
      ),
      true,
    );
    const compositionEvents = await page.evaluate(
      () =>
        (window as unknown as { compositionEvents: string[] })
          .compositionEvents,
    );
    for (const event of [
      "compositionstart",
      "compositionupdate",
      "compositionend",
    ])
      assert.ok(compositionEvents.includes(event));
    await equal(page);
    await press("Control+z");
    assert.equal(await source(page), "前後\r\n保存＃００１");
    await equal(page);
    await press("Control+Shift+z");
    assert.equal(await source(page), "前日本後\r\n保存＃００１");
    await equal(page);
    await select(page, 2);
    await cdp.send("Input.imeSetComposition", {
      text: "取消",
      selectionStart: 2,
      selectionEnd: 2,
    });
    await cdp.send("Input.imeSetComposition", {
      text: "",
      selectionStart: 0,
      selectionEnd: 0,
    });
    await page.waitForTimeout(100);
    assert.equal(await source(page), "前日本後\r\n保存＃００１");
    await equal(page);

    await select(page, 2, 4);
    await cdp.send("Input.imeSetComposition", {
      text: "にっぽん",
      selectionStart: 4,
      selectionEnd: 4,
    });
    await cdp.send("Input.insertText", { text: "日本国" });
    await page.waitForTimeout(100);
    assert.equal(await source(page), "前日本国後\r\n保存＃００１");
    await equal(page);
    for (const [markup, selector] of [
      ["《振り仮名：峰｜みね》", ".editor-ruby > .editor-right"],
      ["《割書：一｜二｜三｜四》", ".editor-warigaki > .editor-segment"],
    ]) {
      await reset(page, markup + "\n残す");
      await page.locator(selector).first().click();
      await cdp.send("Input.imeSetComposition", {
        text: "かな",
        selectionStart: 2,
        selectionEnd: 2,
      });
      await cdp.send("Input.insertText", { text: "仮名" });
      await page.waitForTimeout(100);
      assert.equal(
        (await source(page)).match(/仮名/g)?.length,
        markup.startsWith("《振り仮名") ? 2 : 1,
      );
      await equal(page);
      await press("Control+z");
      assert.equal(await source(page), markup + "\n残す");
    }
    await reset(page, "前後\n別列");
    await select(page, 2);
    await page.evaluate(async () => {
      const { view } = window.editorSpike!;
      const dom = view.dom,
        column = dom.firstChild;
      dom.dispatchEvent(
        new CompositionEvent("compositionstart", { bubbles: true, data: "" }),
      );
      dom.dispatchEvent(
        new CompositionEvent("compositionupdate", {
          bubbles: true,
          data: "かな",
        }),
      );
      const selection = window.getSelection()!;
      const text = selection.focusNode as Text;
      const offset = selection.focusOffset;
      text.insertData(offset, "かな");
      selection.setPosition(text, offset + 2);
      dom.dispatchEvent(
        new InputEvent("input", {
          bubbles: true,
          inputType: "insertCompositionText",
          data: "かな",
          isComposing: true,
        }),
      );
      await new Promise((resolve) => setTimeout(resolve, 50));
      dom.dispatchEvent(
        new KeyboardEvent("keydown", {
          bubbles: true,
          key: "ArrowLeft",
          isComposing: true,
        }),
      );
      if (
        view.state.selection.$head.index(0) !== 0 ||
        dom.firstChild !== column
      )
        throw Error("Composition changed the column or rebuilt its DOM");
      dom.dispatchEvent(
        new CompositionEvent("compositionend", { bubbles: true, data: "かな" }),
      );
    });
    await page.waitForTimeout(100);
    assert.equal(await source(page), "前かな後\n別列");
    await equal(page);

    for (const [key, expected] of [
      ["Control+r", "前《振り仮名：仮名｜》後"],
      ["Control+w", "前《割書：仮名｜》後"],
      ["Control+m", "前《見せ消ち：仮名｜》後"],
    ]) {
      await reset(page, "前後");
      await select(page, 2);
      await page.keyboard.press(key);
      await cdp.send("Input.imeSetComposition", {
        text: "かな",
        selectionStart: 2,
        selectionEnd: 2,
      });
      await cdp.send("Input.insertText", { text: "仮名" });
      await page.waitForTimeout(100);
      assert.equal(await source(page), expected);
      await equal(page);
    }
    await reset(page, "前後");
    await select(page, 2);
    await page.keyboard.press("Control+w");
    await page.keyboard.press("Control+r");
    await page.keyboard.press("Tab");
    await cdp.send("Input.imeSetComposition", {
      text: "かな",
      selectionStart: 2,
      selectionEnd: 2,
    });
    await cdp.send("Input.insertText", { text: "仮名" });
    await page.waitForTimeout(100);
    assert.equal(await source(page), "前《割書：《振り仮名：｜仮名》｜》後");
    await equal(page);

    await reset(page, "前《振り仮名：峰｜みね》後");
    await select(page, 4, 5);
    await page.keyboard.press("Backspace");
    await cdp.send("Input.imeSetComposition", {
      text: "かな",
      selectionStart: 2,
      selectionEnd: 2,
    });
    await cdp.send("Input.insertText", { text: "仮名" });
    await page.waitForTimeout(100);
    assert.equal(await source(page), "前《振り仮名：仮名｜みね》後");
    await equal(page);

    const benchmark = Array.from(
      { length: 20 },
      (_, i) =>
        `${i + 1}列　《振り仮名：峰｜みね》シリキタイ《割書：一｜二》原文を保存する。`,
    ).join("\n");
    await reset(page, benchmark);
    await select(page, 1);
    for (let i = 0; i < 19; i++) await press("ArrowLeft");
    assert.equal(await selectedColumn(page), 19);
    const visible = await page.evaluate(() => {
      const view = window.editorSpike!.view;
      const caret = view.coordsAtPos(view.state.selection.head);
      const panel = document
        .querySelector(".editor-scroll")!
        .getBoundingClientRect();
      return caret.left >= panel.left && caret.right <= panel.right;
    });
    assert.equal(
      visible,
      true,
      "caret remains visible when navigating a wide page",
    );
    await reset(page, benchmark);
    await select(page, 1);
    await page.evaluate(() => {
      const values: number[] = [];
      const view = window.editorSpike!.view;
      (window as unknown as { typingTimes: number[] }).typingTimes = values;
      const dispatch = view.dispatch.bind(view);
      view.dispatch = (tr) => {
        const start = performance.now();
        dispatch(tr);
        if (tr.docChanged) {
          view.dom.getBoundingClientRect();
          values.push(performance.now() - start);
        }
      };
    });
    const endToEnd: number[] = [];
    for (let i = 0; i < 60; i++) {
      const start = performance.now();
      await page.keyboard.type("a");
      await page.evaluate(
        () =>
          new Promise<void>((resolve) =>
            requestAnimationFrame(() => resolve()),
          ),
      );
      endToEnd.push(performance.now() - start);
    }
    const transactionTimes = await page.evaluate(
      () => (window as unknown as { typingTimes: number[] }).typingTimes,
    );
    const summary = (values: number[]) => {
      const sorted = [...values].sort((a, b) => a - b);
      return {
        samples: values.length,
        medianMs: sorted[Math.floor(sorted.length / 2)],
        p95Ms: sorted[Math.ceil(sorted.length * 0.95) - 1],
        maxMs: sorted.at(-1),
      };
    };
    await equal(page);
    const metrics = {
      browser: browser.version(),
      columns: 20,
      sourceCodeUnits: benchmark.length,
      transactionAndLayout: summary(transactionTimes),
      keyboardToAnimationFrame: summary(endToEnd),
      compositionEvents,
      nativeIME:
        "Not exercised; headless Chromium CDP and synthetic DOM events only",
    };
    await mkdir(".local/shots", { recursive: true });
    await Bun.write(
      ".local/shots/06-editor-metrics.json",
      JSON.stringify(metrics, null, 2) + "\n",
    );
    console.log("Editor metrics:", JSON.stringify(metrics));
    await reset(page, initial);
    await equal(page);
    assert.deepEqual(errors, []);
    console.log(
      "Editor checks passed: fixture round trip, typing, vertical keys, Enter, annotations, palette, raw mode, undo/redo, CDP composition commit/cancel, synthetic composition, 20-column latency.",
    );
  } finally {
    await context.close();
  }
}
async function source(page: Page) {
  return page.evaluate(() => window.editorSpike!.source);
}
async function position(page: Page) {
  return page.evaluate(() => window.editorSpike!.view.state.selection.head);
}
async function selectedColumn(page: Page) {
  return page.evaluate(() =>
    window.editorSpike!.view.state.selection.$head.index(0),
  );
}
async function equal(page: Page) {
  const result = await page.evaluate(async () => {
    const { toMarkup, fromMarkup } = await import("/src/dev/editor-harness.ts");
    const editor = window.editorSpike!;
    return {
      source: editor.source,
      serialized: toMarkup(editor.view.state.doc),
      reparsed: toMarkup(fromMarkup(editor.source)),
    };
  });
  assert.equal(result.serialized, result.source);
  assert.equal(result.reparsed, result.source);
  assert.equal(await page.locator(".editor-equality").textContent(), "一致");
}
async function reset(page: Page, text: string) {
  await page.evaluate((text) => window.editorSpike!.setSource(text), text);
  await equal(page);
}
async function select(page: Page, from: number, to = from) {
  await page.evaluate(
    async ({ from, to }) => {
      const { TextSelection } = await import("/src/dev/editor-harness.ts");
      const view = window.editorSpike!.view;
      view.dispatch(
        view.state.tr.setSelection(
          TextSelection.create(view.state.doc, from, to),
        ),
      );
      view.focus();
    },
    { from, to },
  );
}

export async function checkInlineEditor(
  browser: Browser,
  origin: string,
  theme: "light" | "dark",
  suffix = "",
) {
  const context = await browser.newContext({
    viewport: { width: 2000, height: 1100 },
    deviceScaleFactor: 2,
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
    await page.goto(origin + "/#/spike/editor");
    await page.locator(".vertical-editor").waitFor();
    await page.evaluate(() => document.fonts.ready);
    await reset(page, "一二三\n四五六");
    await select(page, 1);
    const points = await page.evaluate(() => {
      const columns = document.querySelectorAll(
        ".vertical-editor .transcription-column",
      );
      return [...columns].map((column) => {
        const range = document.createRange();
        range.setStart(column.firstChild!, 0);
        range.setEnd(column.firstChild!, 1);
        const r = range.getBoundingClientRect();
        return { x: (r.left + r.right) / 2, y: r.top, advance: r.height };
      });
    });
    await page.mouse.move(points[0].x, points[0].y + 1);
    await page.mouse.down();
    await page.mouse.move(points[1].x, points[1].y + points[1].advance * 2, {
      steps: 20,
    });
    await page.mouse.up();
    assert.equal(
      await page.evaluate(() => {
        const v = window.editorSpike!.view;
        return v.someProp("clipboardTextSerializer")!(
          v.state.selection.content(),
          v,
        );
      }),
      "一二三\n四五",
    );
    assert.equal(
      await page.evaluate(() => window.getSelection()!.isCollapsed),
      false,
    );
    if (!suffix) {
      await context.grantPermissions(["clipboard-read", "clipboard-write"]);
      await page.keyboard.press("Control+c");
      assert.equal(
        await page.evaluate(() => navigator.clipboard.readText()),
        "一二三\n四五",
      );
    }
    await select(page, 6);
    await page.keyboard.press("Shift+ArrowRight");
    assert.equal(
      await page.evaluate(() => window.editorSpike!.view.state.selection.empty),
      false,
    );
    await select(page, 1);
    await page.keyboard.press("Control+a");
    assert.equal(
      await page.evaluate(() => window.editorSpike!.view.state.selection.to),
      4,
    );
    await page.keyboard.press("Control+a");
    assert.equal(
      await page.evaluate(() => window.editorSpike!.view.state.selection.to),
      9,
    );

    await reset(page, "かな漢字かな");
    const kanji = await page
      .locator(".transcription-column")
      .first()
      .evaluate((el) => {
        const range = document.createRange();
        range.setStart(el.firstChild!, 2);
        range.setEnd(el.firstChild!, 3);
        const r = range.getBoundingClientRect();
        return { x: (r.left + r.right) / 2, y: r.top + 2 };
      });
    await page.mouse.dblclick(kanji.x, kanji.y);
    assert.equal(
      await page.evaluate(() => window.getSelection()!.toString()),
      "漢字",
    );

    await reset(page, "前後");
    await select(page, 2);
    await page.keyboard.press("Control+w");
    assert.equal(await source(page), "前《割書：｜》後");
    await page.keyboard.insertText("一");
    await page.keyboard.press("Enter");
    await page.keyboard.insertText("二");
    assert.equal(await source(page), "前《割書：一｜二》後");
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    assert.equal(await source(page), "前《割書：一｜二｜｜》後");
    await page.keyboard.press("Backspace");
    await page.keyboard.press("Backspace");
    assert.equal(await source(page), "前《割書：一｜二｜｜》後");
    await reset(page, "前《割書：一｜二》後");
    await select(page, 8);
    await page.keyboard.press("Control+r");
    await page.keyboard.insertText("峰");
    await page.keyboard.press("Tab");
    await page.keyboard.insertText("みね");
    await page.keyboard.press("ArrowDown");
    assert.equal(
      await source(page),
      "前《割書：一｜二《振り仮名：峰｜みね》》後",
    );
    await equal(page);
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await page.keyboard.insertText("続");
    assert.equal(
      await source(page),
      "前《割書：一｜二《振り仮名：峰｜みね》》続後",
    );
    await page.screenshot({
      path: `.local/shots/14-inline-editing-${theme}${suffix}.png`,
      caret: "initial",
    });

    for (const key of ["Control+r", "Control+w", "Control+m"]) {
      await reset(page, "前後");
      await select(page, 2);
      await page.keyboard.press(key);
      await page.keyboard.press("Backspace");
      assert.equal(await source(page), "前後");
    }
    await reset(page, "前後");
    await select(page, 2);
    await page.keyboard.press("Control+w");
    await page.keyboard.press("Control+r");
    await page.keyboard.insertText("山");
    await page.keyboard.press("Tab");
    await page.keyboard.insertText("やま");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await page.keyboard.insertText("川");
    assert.equal(
      await source(page),
      "前《割書：《振り仮名：山｜やま》｜川》後",
    );
    await equal(page);

    await page.keyboard.press("Escape");
    await select(page, 2);
    await page.locator(".vertical-editor").evaluate((dom) => {
      const clipboardData = new DataTransfer();
      clipboardData.setData(
        "text/plain",
        "《割書：《振り仮名：峰｜みね》｜二》",
      );
      dom.dispatchEvent(
        new ClipboardEvent("paste", {
          bubbles: true,
          cancelable: true,
          clipboardData,
        }),
      );
    });
    assert.ok(
      (await source(page)).startsWith("前《割書：《振り仮名：峰｜みね》｜二》"),
    );
    assert.equal(await page.locator(".editor-warigaki").count(), 2);
    await equal(page);
    await button("原文表示").click();
    const raw = page.getByRole("textbox", { name: "原文を編集", exact: true });
    for (const title of ["振り仮名", "割書", "見せ消ち"]) {
      await raw.fill("前後");
      await raw.evaluate((el: HTMLTextAreaElement) =>
        el.setSelectionRange(1, 1),
      );
      await button(title).click();
      assert.equal(await raw.inputValue(), `前《${title}：｜》後`);
      await page.keyboard.insertText("一");
      assert.equal(await raw.inputValue(), `前《${title}：一｜》後`);
    }
    await raw.fill("前後");
    await raw.evaluate((el: HTMLTextAreaElement) => el.setSelectionRange(1, 1));
    await page.keyboard.press("Control+n");
    assert.equal(await raw.inputValue(), "前＃1後");
    assert.equal(
      await page
        .getByLabel("注記の内容", { exact: true })
        .evaluate((e) => e === document.activeElement),
      true,
    );
    await page.keyboard.insertText("欄外の注記");
    await page.keyboard.press("Escape");
    assert.equal(await raw.evaluate((e) => e === document.activeElement), true);
    await button("原文表示").click();

    const layout = await Bun.file(
      new URL("../../fixtures/markup/layout-samples.txt", import.meta.url),
    ).text();
    await reset(page, layout);
    await select(page, 1);
    const { parseInline, renderInline } = await import("../../packages/markup");
    const html = layout
      .split("\n")
      .map(
        (line) =>
          `<div class="transcription-column">${renderInline(parseInline(line))}</div>`,
      )
      .join("");
    await page.evaluate((html) => {
      const panel = document.querySelector(".editor-source-panel")!;
      panel.innerHTML =
        '<div class="pane-toolbar"><h2>表示</h2></div><div class="transcription"><div class="columns">' +
        html +
        "</div></div>";
      document.querySelector<HTMLElement>(
        ".editor-spike-panes",
      )!.style.gridTemplateColumns = "1fr 1fr";
    }, html);
    await page.evaluate(() => document.fonts.ready);
    const geometry = await page
      .locator(".editor-annotation")
      .evaluateAll((nodes) =>
        nodes.flatMap((el) => {
          const rect = el.getBoundingClientRect();
          const font = parseFloat(getComputedStyle(el).fontSize);
          if (el.classList.contains("editor-ruby")) {
            const reading = el
              .querySelector(":scope > .editor-right")!
              .getBoundingClientRect();
            return [
              {
                kind: "ruby",
                centered: Math.abs(
                  reading.top + reading.height / 2 - rect.top - rect.height / 2,
                ),
                right: reading.left >= rect.right - 1,
                compact:
                  rect.height <=
                  el.firstElementChild!.getBoundingClientRect().height + 1,
              },
            ];
          }
          if (el.classList.contains("editor-warigaki")) {
            const lines = [...el.children].map((line) =>
              line.getBoundingClientRect(),
            );
            return [
              {
                kind: "warigaki",
                centered: Math.max(
                  ...lines.map((line) => Math.abs(line.top - rect.top)),
                ),
                right: lines.every(
                  (line, i) => !i || lines[i - 1].left >= line.right - 1,
                ),
                compact:
                  Math.abs(
                    rect.height - Math.max(...lines.map((line) => line.height)),
                  ) < 1,
              },
            ];
          }
          return [];
        }),
      );
    for (const item of geometry) {
      assert.ok(item.centered < 1, JSON.stringify(item));
      assert.ok(item.right && item.compact, JSON.stringify(item));
    }
    await page.mouse.move(0, 0);
    await page.screenshot({
      path: `.local/shots/14-layout-samples-${theme}${suffix}.png`,
    });
    assert.equal(
      await page
        .locator(
          ".vertical-editor .editor-warigaki .editor-ruby > .editor-right",
        )
        .first()
        .evaluate((el) => getComputedStyle(el).fontSize),
      "6.5px",
    );
    assert.deepEqual(errors, []);
    console.log(
      `Inline editor checks passed (${theme}${suffix}): drag, script runs, keyboard selection, source clipboard, shells, nesting, raw templates, notes, layout.`,
    );
  } finally {
    await context.close();
  }
}

export async function checkCaretContexts(
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
  await context.addInitScript(
    (theme) => localStorage.setItem("honkoku.theme", theme),
    theme,
  );
  const page = await context.newPage();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  try {
    await page.goto(origin + "/#/spike/editor");
    await page.locator(".vertical-editor").waitFor();
    await page.evaluate(() => document.fonts.ready);
    await reset(page, "讀＿レ￣ム");
    await select(page, 12);
    await page.keyboard.press("Backspace");
    assert.equal(await source(page), "讀＿レ￣ム");
    assert.equal(
      await page.locator(".editor-okurigana.editor-caret-big").count(),
      1,
    );
    assert.equal(
      await page
        .locator(".vertical-editor")
        .evaluate((el) => getComputedStyle(el).caretColor),
      "rgba(0, 0, 0, 0)",
    );
    if (theme === "light")
      await page.screenshot({
        path: `.local/shots/21-caret-big-light${suffix}.png`,
        caret: "initial",
      });
    await page.keyboard.press("Backspace");
    assert.equal(await source(page), "讀＿レ");
    await reset(page, "讀＿レ￣ム");
    await select(page, 12);
    await page.keyboard.press("ArrowUp");
    assert.equal(await position(page), 10);
    assert.equal(
      await page.locator(".editor-okurigana.editor-caret-small").count(),
      1,
    );
    if (theme === "dark")
      await page.screenshot({
        path: `.local/shots/21-caret-small-dark${suffix}.png`,
        caret: "initial",
      });
    await page.keyboard.press("Backspace");
    assert.equal(await source(page), "讀＿レ￣");
    await page.keyboard.press("Backspace");
    assert.equal(await source(page), "讀＿レ");

    await reset(page, "《割書：《振り仮名：峰｜みね》｜二行》");
    await select(page, 8);
    const path = page.getByRole("navigation", { name: "カーソルの位置" });
    assert.deepEqual(await path.getByRole("button").allTextContents(), [
      "本文",
      "割書 1行目",
      "振り仮名 読み",
    ]);
    assert.equal(await page.locator(".editor-caret-small").count(), 1);
    assert.equal(
      await page.locator(".editor-warigaki.editor-caret-small").count(),
      0,
    );
    if (theme === "light")
      await page.screenshot({
        path: `.local/shots/21-caret-path-light${suffix}.png`,
        caret: "initial",
      });
    await page.keyboard.press("Escape");
    assert.equal(
      await page.locator(".editor-ruby.editor-caret-big").count(),
      1,
    );
    await page.keyboard.press("Escape");
    assert.equal(
      await page.locator(".editor-warigaki.editor-caret-big").count(),
      1,
    );
    await page.keyboard.press("Escape");
    assert.equal(await position(page), 18);
    await select(page, 8);
    await path.getByRole("button", { name: "割書 1行目", exact: true }).click();
    assert.equal(
      await page.locator(".editor-warigaki.editor-caret-big").count(),
      1,
    );
    await page.keyboard.press("Enter");
    assert.equal(await position(page), 3);

    await reset(page, "未（いまだ｜ズ）");
    await select(page, 6);
    await page.locator(".editor-ruby > .editor-right").dblclick();
    assert.equal(
      await page.evaluate(() => window.getSelection()!.toString()),
      "いまだ",
    );
    // The visible label is part of the shell's hit target.
    const labelBox = await page.locator(".editor-ruby").boundingBox();
    await page.mouse.click(labelBox!.x + 4, labelBox!.y - 7);
    assert.equal(
      await page.locator(".editor-ruby.editor-caret-big").count(),
      1,
    );
    await page.keyboard.insertText("字");
    assert.equal(await source(page), "字");
    if (!suffix) {
      await context.grantPermissions(["clipboard-read", "clipboard-write"]);
      await reset(page, "未（いまだ｜ズ）");
      await select(page, 1);
      await page.keyboard.press("Delete");
      await page.keyboard.press("Control+c");
      assert.equal(
        await page.evaluate(() => navigator.clipboard.readText()),
        "未（いまだ｜ズ）",
      );
      await page.keyboard.press("Control+x");
      assert.equal(await source(page), "");
      await reset(page, "未（いまだ｜ズ）");
      await select(page, 1);
      await page.keyboard.press("Delete");
      const cdp = await context.newCDPSession(page);
      await cdp.send("Input.imeSetComposition", {
        text: "かな",
        selectionStart: 2,
        selectionEnd: 2,
      });
      await cdp.send("Input.insertText", { text: "仮名" });
      await page.waitForTimeout(100);
      assert.equal(await source(page), "仮名");
      await cdp.detach();
    }
    for (const atom of ["□", "■", "＃1", "＿レ"]) {
      await reset(page, atom);
      await select(page, 1);
      await page.locator(".editor-source-anchor, .editor-return").click();
      assert.equal(await page.locator(".editor-caret-big").count(), 1);
      await page.keyboard.press("Delete");
      assert.equal(await source(page), "");
    }
    // Switch the same editor surface used by the workbench toggle.
    await page
      .locator(".editor-scroll")
      .evaluate((el) => el.classList.add("horizontal"));
    await reset(page, "讀＿レ￣ム");
    await select(page, 12);
    await page.keyboard.press("ArrowLeft");
    assert.equal(await position(page), 10);
    await page.keyboard.press("ArrowRight");
    assert.equal(await position(page), 12);
    await select(page, 7);
    await page.keyboard.press("Shift+ArrowRight");
    assert.deepEqual(
      await page.evaluate(() => {
        const s = window.editorSpike!.view.state.selection;
        return [s.from, s.to];
      }),
      [7, 12],
    );
    await equal(page);
    assert.deepEqual(errors, []);
    console.log(`Caret context checks passed (${theme}${suffix}).`);
  } finally {
    await context.close();
  }
}
