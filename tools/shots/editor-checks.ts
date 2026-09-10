import assert from "node:assert/strict";
import type { Browser, Page } from "playwright";
import { mkdir } from "node:fs/promises";
export async function checkEditor(browser: Browser, origin: string) {
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
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
    await page.getByLabel("読み", { exact: true }).fill("みね");
    await page.getByRole("button", { name: "挿入", exact: true }).click();
    assert.equal(
      await source(page),
      "《振り仮名：峰｜みね》\r\n変えない＃００１\r《未知：原文》",
    );
    await equal(page);
    await page.getByRole("button", { name: "元に戻す", exact: true }).click();
    assert.equal(await source(page), "峰\r\n変えない＃００１\r《未知：原文》");
    await equal(page);
    await page.getByRole("button", { name: "やり直す", exact: true }).click();
    await page.locator(".editor-ruby > .editor-right").click();
    await page.keyboard.type("A");
    assert.ok((await source(page)).includes("A"));
    await equal(page);

    for (const [button, expected] of [
      ["割書", "《割書：峰｜》"],
      ["見せ消ち", "《見せ消ち：峰｜》"],
    ]) {
      await reset(page, "峰\n残す");
      await select(page, 1, 2);
      await page.getByRole("button", { name: button, exact: true }).click();
      assert.equal(await source(page), expected + "\n残す");
      await equal(page);
    }
    await reset(page, "文\n残す");
    await select(page, 2);
    await page.getByRole("button", { name: "欠字 □", exact: true }).click();
    await page.getByRole("button", { name: "注記", exact: true }).click();
    await page.getByRole("button", { name: "合字 ゟ", exact: true }).click();
    assert.equal(await source(page), "文□＃1ゟ\n残す");
    await equal(page);
    await page.getByRole("button", { name: "原文表示", exact: true }).click();
    await page
      .getByRole("textbox", { name: "原文を編集", exact: true })
      .fill(
        "【右丁】\r\n《割書：一｜二｜三｜四》\r\n【左丁】\n《振り仮名：未｜いまだ｜ズ》",
      );
    await equal(page);
    await page.getByRole("button", { name: "原文表示", exact: true }).click();
    assert.equal(await page.locator(".editor-half-divider").count(), 2);
    assert.equal(
      await page.locator(".editor-warigaki > .editor-segment").count(),
      4,
    );
    assert.equal(
      await page.locator(".editor-ruby > .editor-segment").count(),
      3,
    );
    for (const selector of [
      ".editor-warigaki > .editor-segment",
      ".editor-ruby > .editor-segment",
    ]) {
      const count = await page.locator(selector).count();
      for (let i = 0; i < count; i++) {
        await page.locator(selector).nth(i).click();
        await page.keyboard.type("x");
        await equal(page);
      }
    }
    assert.equal((await source(page)).match(/x/g)?.length, 7);

    await reset(page, "《振り仮名：峰｜みね》\n残す");
    await select(page, 3);
    await page.locator(".vertical-editor").evaluate((dom) => {
      const data = new DataTransfer();
      data.setData("text/plain", "a\nb");
      dom.dispatchEvent(
        new ClipboardEvent("paste", {
          bubbles: true,
          cancelable: true,
          clipboardData: data,
        }),
      );
    });
    assert.equal(await source(page), "《振り仮名：a\nb峰｜みね》\n残す");
    await equal(page);
    await reset(page, "一\r\n二\r三");
    const rawInput = page.getByRole("textbox", { name: "原文", exact: true });
    await rawInput.focus();
    await rawInput.evaluate((e: HTMLTextAreaElement) =>
      e.setSelectionRange(2, 2),
    );
    await page.keyboard.type("X");
    assert.equal(await source(page), "一\r\nX二\r三");
    await press("Control+z");
    assert.equal(await source(page), "一\r\n二\r三");
    await press("Control+Shift+z");
    assert.equal(await source(page), "一\r\nX二\r三");
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
