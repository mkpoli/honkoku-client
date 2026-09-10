import { checkInteractions } from "./checks";
import { chromium, type Page } from "playwright";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
const root = resolve(import.meta.dir, "../..");
const origin = "http://127.0.0.1:1420";
const output = resolve(root, ".local/shots");
const routes = [
  ["01-home", "#/"],
  ["02-project", "#/projects/ainu"],
  ["03-collection", "#/projects/ainu/collections/3R4VhlBfvOYeqPY13cJm"],
  ["04-entry", "#/entries/0916dafb80cdc48ca7687afcad4a4f35"],
  ["05-workbench", "#/entries/0916dafb80cdc48ca7687afcad4a4f35/pages/3"],
] as const;
async function available() {
  try {
    const r = await fetch(origin);
    return r.ok && (await r.text()).includes("みんなで翻刻");
  } catch {
    return false;
  }
}
let server: ReturnType<typeof Bun.spawn> | undefined;
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
    if (/^devrun-\d+-\d+\.scope$/.test(name))
      await Bun.spawn(["systemctl", "--user", "stop", name], {
        stdout: "ignore",
        stderr: "ignore",
      }).exited;
  }
  if (server.exitCode === null) server.kill("SIGTERM");
  await server.exited;
  console.log("Owned dev server stopped.");
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
    server = Bun.spawn(
      ["devrun", "bun", "run", "--cwd", "apps/client", "dev"],
      {
        cwd: root,
        stdout: Bun.file(resolve(output, "vite.log")),
        stderr: Bun.file(resolve(output, "vite-error.log")),
      },
    );
    for (let attempt = 0; attempt < 100; attempt++) {
      if (await available()) break;
      if (server.exitCode !== null)
        throw Error("Vite exited before becoming ready.");
      await Bun.sleep(200);
    }
    if (!(await available())) throw Error("Vite did not start on port 1420.");
  }
  browser = await chromium.launch({ headless: true });
  await checkInteractions(browser, origin);
  for (const theme of ["light", "dark"] as const) {
    const context = await browser.newContext({
      viewport: { width: 1600, height: 1000 },
      deviceScaleFactor: 1,
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
    }
    if (failures.length) throw Error(failures.join("\n"));
    await context.close();
  }
} finally {
  await browser?.close();
  await stopServer();
}
