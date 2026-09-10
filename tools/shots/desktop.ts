import { resolve } from "node:path";

const root = resolve(import.meta.dir, "../..");
const port = process.env.HONKOKU_SHOT_PORT;
if (!port) throw Error("Desktop capture requires a port.");
const server = Bun.spawn(
  [
    process.execPath,
    "--bun",
    resolve(root, "apps/client/node_modules/vite/bin/vite.js"),
    "--host",
    "127.0.0.1",
    "--port",
    port,
  ],
  {
    cwd: resolve(root, "apps/client"),
    env: {
      ...process.env,
      CHOKIDAR_USEPOLLING: "1",
      CHOKIDAR_INTERVAL: "1500",
    },
    stdout: "inherit",
    stderr: "inherit",
  },
);
try {
  let ready = false;
  const deadline = Date.now() + 60000;
  while (Date.now() < deadline && server.exitCode === null) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/`, {
        signal: AbortSignal.timeout(2000),
      });
      if (response.ok && (await response.text()).includes("みんなで翻刻")) {
        ready = true;
        break;
      }
    } catch {}
    await Bun.sleep(200);
  }
  if (!ready) throw Error("Vite did not become ready.");
  const app = Bun.spawn([resolve(root, "target/debug/honkoku-client")], {
    cwd: root,
    stdout: "inherit",
    stderr: "inherit",
  });
  process.exitCode = await app.exited;
} finally {
  server.kill();
  await server.exited;
}
