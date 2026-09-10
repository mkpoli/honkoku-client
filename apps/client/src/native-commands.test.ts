import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const read = (path: string) => readFileSync(new URL(path, import.meta.url), "utf8");
test("client commands are registered and allowed in the local main window", () => {
  const api = read("../../../packages/client-api/invoke.ts");
  const commands = new Set([...api.matchAll(/"([a-z]+(?:_[a-z]+)+)"/g)].map(match => match[1]));
  const manifest = read("../src-tauri/build.rs");
  const handler = read("../src-tauri/src/main.rs").split(".invoke_handler(tauri::generate_handler![")[1].split("])")[0];
  const capability = JSON.parse(read("../src-tauri/capabilities/default.json"));
  expect(capability.local).toBe(true);
  expect(capability.windows).toEqual(["main"]);
  for (const command of commands) {
    expect(manifest, command).toContain(`"${command}"`);
    expect(handler, command).toMatch(new RegExp(`\\b${command}\\s*,`));
    expect(capability.permissions, command).toContain(`allow-${command.replaceAll("_", "-")}`);
  }
});
