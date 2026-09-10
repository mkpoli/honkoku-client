import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";

const origin = "https://honkoku3-c466c.firebaseapp.com";
const started = Math.floor(Date.now() / 1000);
const search = `?provider=google.com&event=attempt&started=${started}&deadline=${started + 600}&oauth_token=secret-query`;
const source = readFileSync(
  new URL("../src/signin.js", import.meta.url),
  "utf8",
)
  .replace(
    "__SIGN_IN_CONFIG__",
    JSON.stringify({
      apiKey: "key",
      authDomain: "auth",
      origin,
      sdkVersion: "10.14.1",
      provider: "google.com",
    }),
  )
  .replace(
    /import\(`\$\{base\}\/firebase-(app|auth)\.js`\)/g,
    (_, module) => `Promise.resolve(modules.${module})`,
  );
class Element {
  children: Element[] = [];
  style: Record<string, string> = {};
  textContent = "";
  onclick?: () => Promise<void>;
  disabled = false;
  constructor(public tag: string) {}
  setAttribute() {}
  appendChild(child: Element) {
    this.children.push(child);
  }
  set innerHTML(_: string) {
    this.children = [];
  }
  get text(): string {
    return this.textContent + this.children.map((c) => c.text).join("\n");
  }
  button(): Element | undefined {
    return this.tag === "button"
      ? this
      : this.children.map((c) => c.button()).find(Boolean);
  }
}
function user() {
  return {
    uid: "secret-uid",
    refreshToken: "secret-refresh",
    getIdTokenResult: async () => ({
      token: "secret-id-token",
      claims: {
        auth_time: started,
        firebase: { sign_in_provider: "google.com" },
      },
    }),
  };
}
function harness(
  options: {
    origin?: string;
    path?: string;
    child?: boolean;
    phase?: string | null;
    result?: boolean;
    currentUser?: boolean;
    failure?: unknown;
    cleanupFailure?: boolean;
    search?: string;
  } = {},
) {
  const calls: string[] = [],
    captures: any[] = [],
    handlers: Record<string, (event: any) => void> = {};
  const storage = new Map<string, string>();
  if (options.phase !== null)
    storage.set("honkoku:signin:attempt", options.phase ?? "redirecting");
  const session = new Map<string, string>();
  const body = new Element("body"),
    site = new Element("site");
  body.appendChild(site);
  let touches = 0,
    cleanupFailure = options.cleanupFailure;
  const instance = { currentUser: options.currentUser ? user() : null };
  const modules = {
    app: {
      initializeApp: (_: unknown, name: string) => {
        expect(name).toBe("honkoku-client");
        return {};
      },
    },
    auth: {
      getAuth: () => instance,
      browserLocalPersistence: {},
      setPersistence: async () => {
        calls.push("persistence");
      },
      GoogleAuthProvider: class {},
      TwitterAuthProvider: class {},
      signInWithRedirect: async () => {
        calls.push("redirect");
      },
      getRedirectResult: async () => {
        calls.push("result");
        expect(session.get("firebase:pendingRedirect:key:honkoku-client")).toBe(
          '"true"',
        );
        if (options.failure) throw options.failure;
        return options.result ? { user: user() } : null;
      },
      signOut: async (auth: unknown) => {
        expect(auth).toBe(instance);
        calls.push("signout");
        if (cleanupFailure) throw Error("cleanup failed");
      },
    },
  };
  const window: any = {
    __TAURI_INTERNALS__: {
      invoke: async (command: string, payload: unknown) => {
        calls.push(command);
        captures.push({ command, payload });
      },
    },
    addEventListener: (type: string, fn: (event: any) => void) => {
      touches++;
      handlers[type] = fn;
    },
  };
  window.top = options.child ? {} : window;
  const location = {
    origin: options.origin ?? origin,
    pathname: options.path ?? "/__client_signin",
    search: options.search ?? search,
  };
  const context = {
    window,
    location,
    modules,
    URLSearchParams,
    Date,
    Promise,
    setTimeout,
    clearTimeout,
    localStorage: {
      getItem: (key: string) => {
        touches++;
        return storage.get(key) ?? null;
      },
      setItem: (key: string, value: string) => {
        touches++;
        storage.set(key, value);
      },
    },
    sessionStorage: {
      getItem: (key: string) => {
        touches++;
        return session.get(key) ?? null;
      },
      setItem: (key: string, value: string) => {
        touches++;
        session.set(key, value);
      },
    },
    document: {
      get body() {
        touches++;
        return body;
      },
      title: "",
      createElement: (tag: string) => new Element(tag),
      addEventListener: () => {
        touches++;
      },
    },
  };
  const run = () => runInNewContext(source, context);
  run();
  return {
    calls,
    captures,
    storage,
    body,
    site,
    window,
    handlers,
    run,
    touches: () => touches,
    allowCleanup: () => {
      cleanupFailure = false;
    },
    settle: async () => {
      for (let i = 0; i < 30; i++) await Promise.resolve();
    },
  };
}
describe("Firebase redirect sign-in", () => {
  test("is inert outside the exact top-level entry document, even with a pending phase", () => {
    for (const options of [
      { path: "/" },
      { path: "/other" },
      { path: "/__/auth/handler" },
      { path: "/__/auth/iframe" },
      { path: "/__client_signin/" },
      { child: true },
      { origin: "http://honkoku3-c466c.firebaseapp.com" },
      { origin: `${origin}.evil.test` },
      { origin: `${origin}:444` },
      { origin: "https://accounts.google.com" },
    ]) {
      const h = harness(options);
      expect(h.touches()).toBe(0);
      expect(h.calls).toEqual([]);
      expect(h.window.__HONKOKU_SIGNIN_READY__).toBeUndefined();
    }
  });
  test("restores the pending flag before collection and never redirects a missing result", async () => {
    const h = harness({ currentUser: true });
    await h.settle();
    expect(h.calls).toEqual(["persistence", "result"]);
    expect(h.body.text).toContain("ログイン結果を受け取れませんでした");
    expect(h.body.button()).toBeDefined();
    expect(h.captures).toEqual([]);
    expect(h.site.style.display).toBe("none");
    expect(h.body.children).toContain(h.site);
  });
  test("departs once and remains in the retry state on an empty return", async () => {
    const departure = harness({ phase: null });
    await departure.settle();
    expect(departure.calls.filter((c) => c === "redirect")).toHaveLength(1);
    expect(departure.storage.get("honkoku:signin:attempt")).toBe("redirecting");
    departure.run();
    await departure.settle();
    expect(departure.calls.filter((c) => c === "redirect")).toHaveLength(1);
    const returning = harness({
      phase: departure.storage.get("honkoku:signin:attempt"),
    });
    await returning.settle();
    expect(returning.calls).not.toContain("redirect");
    await returning.body.button()?.onclick?.();
    expect(returning.captures).toEqual([
      { command: "session_sign_in_retry", payload: { attemptId: "attempt" } },
    ]);
  });
  test("captures the attempt id, then clears Firebase persistence before requesting close", async () => {
    const h = harness({ result: true });
    await h.settle();
    expect(h.captures).toEqual([
      {
        command: "session_capture",
        payload: {
          captured: {
            attemptId: "attempt",
            uid: "secret-uid",
            refreshToken: "secret-refresh",
            idToken: "secret-id-token",
          },
        },
      },
      {
        command: "session_sign_in_complete",
        payload: { attemptId: "attempt" },
      },
    ]);
    expect(h.calls).toEqual([
      "persistence",
      "result",
      "session_capture",
      "signout",
      "session_sign_in_complete",
    ]);
    expect(h.storage.get("honkoku:signin:attempt")).toBe("completed");
    expect(h.body.text).toContain("user present");
    for (const secret of [
      "?",
      "secret-query",
      "secret-uid",
      "secret-refresh",
      "secret-id-token",
    ])
      expect(h.body.text).not.toContain(secret);
  });
  test("cleanup failure keeps the window open and retries only cleanup", async () => {
    const h = harness({ result: true, cleanupFailure: true });
    await h.settle();
    expect(h.calls).not.toContain("session_sign_in_complete");
    expect(h.body.text).toContain("ログインは保存されました");
    h.allowCleanup();
    await h.body.button()?.onclick?.();
    expect(h.calls.filter((c) => c === "session_capture")).toHaveLength(1);
    expect(h.calls.at(-1)).toBe("session_sign_in_complete");
  });
  test("never logs error messages, query strings, credentials, or provider response bodies", async () => {
    const h = harness({
      failure: {
        code: "secret-code",
        message: "https://user:password@example.test/?token=secret",
        customData: { message: "secret-body" },
      },
    });
    await h.settle();
    h.handlers.error({ message: "secret-error" });
    h.handlers.unhandledrejection({ reason: "secret-rejection" });
    expect(h.body.text).toContain("page /__client_signin");
    expect(h.body.text).not.toContain("secret");
    expect(h.body.text).not.toContain("?");
  });
  test("expired attempts never initialize Firebase or depart", async () => {
    const h = harness({
      phase: null,
      search: `?event=attempt&started=${started - 601}&deadline=${started - 1}`,
    });
    await h.settle();
    expect(h.calls).toEqual([]);
    expect(h.body.button()).toBeDefined();
  });
  test("the remote capability is restricted to the entry page and sign-in commands", () => {
    const remote = JSON.parse(
      readFileSync(
        new URL("../capabilities/signin.json", import.meta.url),
        "utf8",
      ),
    );
    expect(remote.local).toBe(false);
    expect(remote.windows).toEqual(["signin"]);
    expect(remote.remote.urls).toEqual([`${origin}/__client_signin`]);
    expect(remote.permissions).toEqual([
      "allow-session-capture",
      "allow-session-sign-in-retry",
      "allow-session-sign-in-complete",
    ]);
    const main = JSON.parse(
      readFileSync(
        new URL("../capabilities/default.json", import.meta.url),
        "utf8",
      ),
    );
    expect(main.remote).toBeUndefined();
    expect(main.permissions).not.toContain("allow-session-capture");
  });
});
