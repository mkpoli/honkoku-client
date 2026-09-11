import { checkInlineEditor } from "./editor-checks";
import {
  checkCaretContexts,
  checkHistory,
  checkRecognition,
  checkRankingSelf,
  checkNotes,
  checkGlyphs,
  checkKunten,
  checkSearch,
  checkRegionTimeout,
  checkWorkbenchParity,
  checkBrowsePolish,
  checkInteractions,
  checkEditing,
  checkAlignment,
  checkQuietWorkbench,
} from "./checks";
import { chromium, webkit, type Page } from "playwright";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
const root = resolve(import.meta.dir, "../..");
const probe = Bun.serve({
  hostname: "127.0.0.1",
  port: 0,
  fetch: () => new Response(),
});
const port = probe.port;
await probe.stop(true);
const origin = `http://127.0.0.1:${port}`;
const output = resolve(root, ".local/shots");
const routes = [
  ["01-home", "#/"],
  ["02-project", "#/projects/ainu"],
  ["03-collection", "#/projects/ainu/collections/3R4VhlBfvOYeqPY13cJm"],
  ["04-entry", "#/entries/0916dafb80cdc48ca7687afcad4a4f35"],
  ["05-workbench", "#/entries/0916dafb80cdc48ca7687afcad4a4f35/pages/3"],
  ["06-editor", "#/spike/editor"],
] as const;
async function available() {
  try {
    const r = await fetch(origin, { signal: AbortSignal.timeout(5000) });
    return r.ok && (await r.text()).includes("みんなで翻刻");
  } catch {
    return false;
  }
}
let server: ReturnType<typeof Bun.spawn> | undefined;
let serverLogs: Promise<unknown> | undefined;
async function stopServer() {
  if (!server) return;
  const p = Bun.spawn(
    [
      "systemctl",
      "--user",
      "list-units",
      "--plain",
      "--no-legend",
      `devrun-${server.pid}-*.scope`,
    ],
    { stdout: "pipe", stderr: "ignore" },
  );
  const text = await new Response(p.stdout).text();
  await p.exited;
  for (const line of text.split("\n")) {
    const name = line.trim().split(/\s+/)[0];
    if (/^devrun-\d+-\d+\.scope$/.test(name)) {
      await Bun.spawn(
        ["systemctl", "--user", "kill", "--signal=SIGKILL", name],
        {
          stdout: "ignore",
          stderr: "ignore",
        },
      ).exited;
      await Bun.spawn(["systemctl", "--user", "stop", name], {
        stdout: "ignore",
        stderr: "ignore",
      }).exited;
    }
  }
  if (server.exitCode === null) server.kill("SIGTERM");
  await server.exited;
  await serverLogs;
  const check = Bun.spawn(
    [
      "systemctl",
      "--user",
      "list-units",
      "--plain",
      "--no-legend",
      "--state=running",
      `devrun-${server.pid}-*.scope`,
    ],
    { stdout: "pipe", stderr: "ignore" },
  );
  const remaining = await new Response(check.stdout).text();
  await check.exited;
  if (remaining.trim()) throw Error("Owned dev server scope is still running.");
  console.log("Owned dev server stopped; no running descendants.");
}
process.on("SIGINT", () => {
  void stopServer().then(() => process.exit(130));
});
process.on("SIGTERM", () => {
  void stopServer().then(() => process.exit(143));
});
async function settle(page: Page) {
  await page.locator('main[aria-busy="false"]').waitFor();
  await page.locator(".route-screen").waitFor();
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(2000);
  await page.evaluate(async () => {
    await Promise.all(
      [...document.images]
        .filter((img) => {
          const r = img.getBoundingClientRect();
          return r.bottom > 0 && r.top < innerHeight;
        })
        .map((img) => img.decode().catch(() => {})),
    );
  });
}
await mkdir(output, { recursive: true });
let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined;
try {
  if (!(await available())) {
    await Bun.write(resolve(output, "vite.log"), "");
    await Bun.write(resolve(output, "vite-error.log"), "");
    server = Bun.spawn(
      [
        "devrun",
        "bun",
        "run",
        "--cwd",
        "apps/client",
        "dev",
        "--port",
        String(port),
      ],
      {
        cwd: root,
        env: {
          ...process.env,
          CHOKIDAR_USEPOLLING: "1",
          CHOKIDAR_INTERVAL: "1500",
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    );
    await Bun.write(
      resolve(output, "server.json"),
      JSON.stringify({ pid: server.pid, owner: process.pid, port }),
    );
    serverLogs = Promise.all(
      [
        [server.stdout, "vite.log"],
        [server.stderr, "vite-error.log"],
      ].map(async ([stream, filename]) => {
        const log = await new Response(stream as ReadableStream).text();
        await Bun.write(
          resolve(output, filename as string),
          log.replace(/\/home\/[^/\s]+/g, "~"),
        );
      }),
    );
    const startupDeadline = Date.now() + 60_000;
    while (Date.now() < startupDeadline) {
      if (await available()) break;
      if (server.exitCode !== null)
        throw Error("Vite exited before becoming ready.");
      await Bun.sleep(200);
    }
    if (!(await available()))
      throw Error("Vite did not start on the assigned port.");
  }
  browser = await chromium.launch({ headless: true });
  for (const theme of ["light", "dark"] as const)
    await checkHistory(browser, origin, theme);
  if (process.env.HONKOKU_SHOTS_HISTORY_ONLY === "1") {
    await browser.close();
    browser = undefined;
    await stopServer();
    process.exit(0);
  }
  for (const theme of ["light", "dark"] as const)
    await checkNotes(browser, origin, theme);
  if (process.env.HONKOKU_SHOTS_NOTES_ONLY === "1") {
    await browser.close();
    browser = undefined;
    await stopServer();
    process.exit(0);
  }
  for (const theme of ["light", "dark"] as const)
    await checkCaretContexts(browser, origin, theme);
  for (const theme of ["light", "dark"] as const)
    await checkRecognition(browser, origin, theme);
  if (process.env.HONKOKU_SHOTS_RECOGNITION_ONLY === "1") {
    await browser.close();
    browser = undefined;
    await stopServer();
    process.exit(0);
  }
  await checkInteractions(browser, origin);
  for (const theme of ["light", "dark"] as const)
    await checkWorkbenchParity(browser, origin, theme);
  for (const theme of ["light", "dark"] as const)
    await checkRankingSelf(browser, origin, theme);
  for (const theme of ["light", "dark"] as const)
    await checkGlyphs(browser, origin, theme);
  if (process.env.HONKOKU_SHOTS_GLYPHS_ONLY === "1") {
    await browser.close();
    browser = undefined;
    await stopServer();
    process.exit(0);
  }
  for (const theme of ["light", "dark"] as const)
    await checkKunten(browser, origin, theme);
  for (const theme of ["light", "dark"] as const)
    await checkSearch(browser, origin, theme);
  await checkRegionTimeout(browser, origin);
  for (const theme of ["light", "dark"] as const)
    await checkInlineEditor(browser, origin, theme);
  for (const theme of ["light", "dark"] as const)
    await checkBrowsePolish(browser, origin, theme);
  for (const theme of ["light", "dark"] as const)
    await checkQuietWorkbench(browser, origin, theme);
  for (const theme of ["light", "dark"] as const)
    await checkAlignment(browser, origin, theme);
  const webkitBrowser = await webkit.launch({ headless: true });
  try {
    await checkCaretContexts(webkitBrowser, origin, "light", "-webkit");
    await checkGlyphs(webkitBrowser, origin, "light", "-webkit");
    await checkInlineEditor(webkitBrowser, origin, "light", "-webkit");
    await checkQuietWorkbench(webkitBrowser, origin, "light", "-webkit");
    await checkAlignment(webkitBrowser, origin, "light", "-webkit");
    await checkEditing(webkitBrowser, origin, "light", "-webkit");
  } finally {
    await webkitBrowser.close();
  }
  for (const theme of ["light", "dark"] as const) {
    const context = await browser.newContext({
      viewport: { width: 1600, height: 1000 },
      deviceScaleFactor: 2,
      colorScheme: theme,
      locale: "ja-JP",
      timezoneId: "Asia/Tokyo",
      reducedMotion: "reduce",
    });
    await context.addInitScript(
      (theme) => localStorage.setItem("honkoku.theme", theme),
      theme,
    );
    const page = await context.newPage();
    const failures: string[] = [];
    page.on("pageerror", (e) => failures.push(e.message));
    for (const [name, route] of routes) {
      await page.goto(origin + "/" + route);
      await settle(page);
      if (name === "01-home") await page.locator(".activity").first().waitFor();
      if (name === "03-collection")
        await page.locator(".entry-row").first().waitFor();
      if (name === "05-workbench") await page.waitForTimeout(1200);
      const file = `${name}-${theme}.png`;
      await page.screenshot({ path: resolve(output, file) });
      console.log(`.local/shots/${file}`);
      if (name === "06-editor") {
        await page.evaluate(() => {
          const editor = window.editorSpike!;
          editor.setSource(
            "【右丁】\n《振り仮名：峰｜みね》　シリキタイ\n《割書：原本の小字｜二行目｜三行目｜四行目》\n《見せ消ち：旧字｜新字》　■□〓＃１２\n【左丁】\n未（いまだ｜ズ）　讀＿レ￣ム\n《圏点：燃ゆる天河｜﹅》《右線：川の道》\n《題：蝦夷語箋》《箱：字》《場所：松前》\nゟ　ヿ　𛀂　𬼂\n※欄外の書入れ\n《未知：原文を保持》",
          );
          editor.view.focus();
        });
        await page.evaluate(() => document.fonts.ready);
        await page.screenshot({
          path: resolve(output, `06-editor-annotations-${theme}.png`),
        });
        console.log(`.local/shots/06-editor-annotations-${theme}.png`);
      }
    }
    if (failures.length) throw Error(failures.join("\n"));
    await context.close();
  }
} finally {
  await browser?.close();
  await stopServer();
}
