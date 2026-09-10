import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";

const source = readFileSync(new URL("../src/signin.js", import.meta.url), "utf8")
  .replace("__SIGN_IN_CONFIG__", JSON.stringify({apiKey: "key", provider: "google.com", startedAt: 1000, eventId: "event"}));
function harness(origin = "https://app.honkoku.org", child = false) {
  let poll: (() => Promise<void>) | undefined;
  let record: unknown = null;
  let closed = 0, stopped = 0;
  const captured: unknown[] = [];
  const storage = new Map<string, string>();
  const window: any = { __TAURI_INTERNALS__: { invoke: async (command: string, payload: unknown) => { captured.push({command, payload}); } } };
  window.top = child ? {} : window;
  const context = { window, location: {origin}, sessionStorage: {
    getItem: (key: string) => storage.get(key), setItem: (key: string, value: string) => storage.set(key, value),
  }, Object, JSON, Promise, Date, atob, setInterval: (callback: () => Promise<void>, delay: number) => {
    expect(delay).toBe(1000); poll = callback; return 1;
  }, clearInterval: () => stopped++, indexedDB: {open: (name: string) => {
    expect(name).toBe("firebaseLocalStorageDb");
    const transaction: any = {objectStore: (store: string) => {
      expect(store).toBe("firebaseLocalStorage");
      return {get: (key: string) => {
        expect(key).toBe("firebase:authUser:key:[DEFAULT]");
        const request: any = {};
        queueMicrotask(() => { request.result = record; request.onsuccess(); transaction.oncomplete(); });
        return request;
      }};
    }};
    const request: any = {result:{transaction: () => transaction, close: () => closed++}};
    queueMicrotask(() => request.onsuccess());
    return request;
  }}};
  runInNewContext(source, context);
  return {window, storage, captured, tick: () => poll?.(), hasPoll: () => !!poll,
    setRecord: (value: unknown) => record = value, closed: () => closed, stopped: () => stopped};
}
function record(authTime = 1000, provider = "google.com") {
  return {value:{uid:"user", email:"user@example.test", displayName:"利用者",providerData:[{providerId:provider}],
    stsTokenManager:{refreshToken:"refresh", accessToken:`e30.${btoa(JSON.stringify({auth_time:authTime,firebase:{sign_in_provider:provider}}))}.signature`,expirationTime:2000000000000}}};
}
describe("Firebase session capture", () => {
  test("only the exact top-level HTTPS origin gets a poll or redirect marker", () => {
    for (const origin of ["http://app.honkoku.org", "https://app.honkoku.org.evil.test", "https://accounts.google.com", "https://app.honkoku.org:444"]) {
      const h = harness(origin); expect(h.hasPoll()).toBe(false); expect(h.storage.size).toBe(0);
    }
    expect(harness(undefined, true).hasPoll()).toBe(false);
  });
  test("sets the SDK marker and captures only a fresh result from the chosen provider", async () => {
    const h = harness();
    expect(h.storage.get("firebase:pendingRedirect:key:[DEFAULT]")).toBe('"true"');
    await h.tick(); expect(h.captured).toHaveLength(0);
    h.setRecord(record(999)); await h.tick(); expect(h.captured).toHaveLength(0);
    h.setRecord(record(1000, "twitter.com")); await h.tick(); expect(h.captured).toHaveLength(0);
    h.setRecord(record()); await h.tick();
    expect(h.captured).toEqual([{command:"session_capture",payload:{captured:{uid:"user",email:"user@example.test",displayName:"利用者",providers:["google.com"],refreshToken:"refresh",idToken:record().value.stsTokenManager.accessToken,expiresAt:new Date(2000000000000).toISOString()}}}]);
    expect(h.closed()).toBe(4); expect(h.stopped()).toBe(1);
  });
  test("the remote capability exposes only capture and the main window can close", () => {
    const remote = JSON.parse(readFileSync(new URL("../capabilities/signin.json", import.meta.url), "utf8"));
    expect(remote.local).toBe(false); expect(remote.windows).toEqual(["signin"]);
    expect(remote.remote.urls).toEqual(["https://app.honkoku.org/*"]);
    expect(remote.permissions).toEqual(["allow-session-capture"]);
    const main = JSON.parse(readFileSync(new URL("../capabilities/default.json", import.meta.url), "utf8"));
    expect(main.remote).toBeUndefined(); expect(main.windows).toEqual(["main"]);
    expect(main.permissions).toContain("core:window:allow-close");
    expect(main.permissions).not.toContain("allow-session-capture");
  });
});
