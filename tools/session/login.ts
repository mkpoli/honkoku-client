// Opens a visible Chromium with a persistent profile on app.honkoku.org so the
// account holder can sign in. Once Firebase Auth has a user, the refresh token
// and profile are written to ~/.local/share/honkoku-client/session.json, and
// every request the site makes to its backends is appended to capture.jsonl
// until the window is closed.
//
//   bun tools/session/login.ts            sign in, capture until the window closes
//   bun tools/session/login.ts --url URL  open a different page (after signing in)

import { chromium, type BrowserContext, type Request, type Response } from "playwright";
import { appendFileSync, chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const STATE = join(homedir(), ".local", "share", "honkoku-client");
const PROFILE = join(STATE, "browser-profile");
const SESSION = join(STATE, "session.json");
const CAPTURE = join(STATE, "capture.jsonl");
const API_KEY = "AIzaSyB-n5klhtxCtVmJqcsnhIc7-bWj5Ou--GY";

const BACKENDS = [
  "app.honkoku.org/api",
  "firestore.googleapis.com",
  "identitytoolkit.googleapis.com",
  "securetoken.googleapis.com",
  "firebasestorage.googleapis.com",
  "cloudfunctions.net",
  "run.app",
  "mp.ex.nii.ac.jp",
];
const isBackend = (url: string) => BACKENDS.some((b) => url.includes(b));

const args = process.argv.slice(2);
const startUrl = args.includes("--url") ? args[args.indexOf("--url") + 1] : "https://app.honkoku.org/login";

mkdirSync(PROFILE, { recursive: true });

function record(kind: "request" | "response", data: Record<string, unknown>) {
  appendFileSync(CAPTURE, JSON.stringify({ t: new Date().toISOString(), kind, ...data }) + "\n");
}

function attachCapture(ctx: BrowserContext) {
  ctx.on("request", (req: Request) => {
    if (!isBackend(req.url())) return;
    record("request", {
      method: req.method(),
      url: req.url(),
      headers: req.headers(),
      body: req.postData()?.slice(0, 200_000) ?? null,
      resourceType: req.resourceType(),
    });
  });
  ctx.on("response", async (res: Response) => {
    if (!isBackend(res.url())) return;
    let body: string | null = null;
    try {
      const ct = res.headers()["content-type"] ?? "";
      if (/json|text|javascript/.test(ct)) body = (await res.text()).slice(0, 200_000);
    } catch {
      body = null;
    }
    record("response", { method: res.request().method(), url: res.url(), status: res.status(), headers: res.headers(), body });
  });
}

interface AuthRecord {
  fbase_key: string;
  value: {
    uid: string;
    email?: string;
    displayName?: string;
    providerData?: { providerId: string }[];
    stsTokenManager: { refreshToken: string; accessToken: string; expirationTime: number };
  };
}

async function readAuth(ctx: BrowserContext): Promise<AuthRecord | null> {
  for (const page of ctx.pages()) {
    if (!page.url().startsWith("https://app.honkoku.org")) continue;
    try {
      const rows = await page.evaluate(
        () =>
          new Promise<unknown[]>((resolve) => {
            const open = indexedDB.open("firebaseLocalStorageDb");
            open.onerror = () => resolve([]);
            open.onsuccess = () => {
              try {
                const db = open.result;
                const all = db.transaction("firebaseLocalStorage", "readonly").objectStore("firebaseLocalStorage").getAll();
                all.onsuccess = () => resolve(all.result as unknown[]);
                all.onerror = () => resolve([]);
              } catch {
                resolve([]);
              }
            };
          }),
      );
      const hit = (rows as AuthRecord[]).find((r) => r?.fbase_key?.startsWith("firebase:authUser:") && r.value?.stsTokenManager?.refreshToken);
      if (hit) return hit;
    } catch {
      // page navigating; try again on the next tick
    }
  }
  return null;
}

const ctx = await chromium.launchPersistentContext(PROFILE, {
  headless: false,
  viewport: { width: 1440, height: 900 },
  locale: "ja-JP",
});
attachCapture(ctx);
const page = ctx.pages()[0] ?? (await ctx.newPage());
await page.goto(startUrl);

console.log(`Capturing backend traffic to ${CAPTURE}`);
console.log("Sign in in the browser window. Close the window when done.");

let saved: string | null = null;
const poll = setInterval(async () => {
  const auth = await readAuth(ctx);
  if (!auth) return;
  const { value } = auth;
  const session = {
    apiKey: API_KEY,
    uid: value.uid,
    email: value.email ?? null,
    displayName: value.displayName ?? null,
    providers: (value.providerData ?? []).map((p) => p.providerId),
    refreshToken: value.stsTokenManager.refreshToken,
    idToken: value.stsTokenManager.accessToken,
    expiresAt: new Date(value.stsTokenManager.expirationTime).toISOString(),
    savedAt: new Date().toISOString(),
  };
  const identity = `${session.uid}:${session.refreshToken}`;
  if (identity === saved) return;
  writeFileSync(SESSION, JSON.stringify(session, null, 2) + "\n");
  chmodSync(SESSION, 0o600);
  saved = identity;
  console.log(`Signed in as ${session.displayName ?? session.email ?? session.uid} (${session.providers.join(", ") || "unknown provider"}); session written to ${SESSION}`);
}, 2000);

await new Promise<void>((resolve) => ctx.on("close", () => resolve()));
clearInterval(poll);
console.log("Browser closed.");
