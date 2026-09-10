(() => {
  const config = __SIGN_IN_CONFIG__;
  if (window.top !== window || location.origin !== config.origin) return;
  const key = `honkoku:signin:${config.eventId}`;
  const entry = location.pathname === "/__client_signin";
  const phase = localStorage.getItem(key);
  if (!entry && !phase) return;
  Object.defineProperty(window, "__HONKOKU_SIGNIN_READY__", { value: true });

  const providerName = config.provider === "google.com" ? "Google" : "X";
  const lines = [];
  let box = null;
  let headline = "";
  let retry = false;
  const stamp = () => new Date().toISOString().slice(11, 19);
  const render = () => {
    if (!document.body) return;
    document.title = "ログイン";
    if (!box) {
      document.body.innerHTML = "";
      box = document.createElement("div");
      box.setAttribute(
        "style",
        "min-height:100vh;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:16px;background:#F4F2EE;color:#202632;font:16px/1.7 'Noto Sans JP','Hiragino Sans','Yu Gothic',sans-serif;padding:32px;text-align:center",
      );
      document.body.appendChild(box);
    }
    box.innerHTML = "";
    const text = document.createElement("p");
    text.textContent = headline;
    box.appendChild(text);
    if (retry) {
      const button = document.createElement("button");
      button.textContent = "もう一度試す";
      button.setAttribute(
        "style",
        "padding:8px 16px;border:1px solid #D95936;border-radius:6px;background:#D95936;color:#fff;font:inherit;cursor:pointer",
      );
      button.onclick = () => {
        localStorage.removeItem(key);
        location.href = `${config.origin}/__client_signin?provider=${encodeURIComponent(config.provider)}`;
      };
      box.appendChild(button);
    }
    const log = document.createElement("pre");
    log.setAttribute(
      "style",
      "max-width:100%;overflow:auto;text-align:left;font:12px/1.5 Consolas,'Courier New',monospace;color:#68717D;background:#fff;border:1px solid #DDDCD6;border-radius:6px;padding:12px;white-space:pre-wrap;overflow-wrap:anywhere",
    );
    log.textContent = lines.join("\n");
    box.appendChild(log);
  };
  const note = (message) => {
    lines.push(`${stamp()} ${message}`);
    render();
  };
  const show = (message, withRetry) => {
    headline = message;
    retry = Boolean(withRetry);
    note(message);
  };
  document.addEventListener("DOMContentLoaded", render, { once: true });
  window.addEventListener("error", (event) => note(`error: ${event.message}`));
  window.addEventListener("unhandledrejection", (event) => note(`rejection: ${event.reason?.code ?? event.reason?.message ?? event.reason}`));

  note(`page ${location.href}`);
  note(`phase ${phase ?? "(none)"}; provider ${config.provider}`);

  const claimsOf = (token) => {
    const encoded = token.split(".")[1].replace(/-/g, "+").replace(/_/g, "/");
    return JSON.parse(atob(encoded));
  };
  const withTimeout = (promise, seconds, label) =>
    Promise.race([
      promise,
      new Promise((_, reject) => setTimeout(() => reject(new Error(`${label} timed out after ${seconds}s`)), seconds * 1000)),
    ]);

  (async () => {
    show(`${providerName}のログインページを準備しています…`);
    const base = `https://www.gstatic.com/firebasejs/${config.sdkVersion}`;
    const [{ initializeApp }, auth] = await withTimeout(
      Promise.all([import(`${base}/firebase-app.js`), import(`${base}/firebase-auth.js`)]),
      20,
      "SDK load",
    );
    note("SDK loaded");
    const app = initializeApp({ apiKey: config.apiKey, authDomain: config.authDomain }, "honkoku-client");
    const instance = auth.getAuth(app);
    await auth.setPersistence(instance, auth.browserLocalPersistence);
    note("auth ready");
    const provider = config.provider === "google.com" ? new auth.GoogleAuthProvider() : new auth.TwitterAuthProvider();
    if (phase !== "redirecting") {
      localStorage.setItem(key, "redirecting");
      show(`${providerName}のログインページへ移動します…`);
      await auth.signInWithRedirect(instance, provider);
      note("redirect requested");
      return;
    }
    show("ログイン結果を確認しています…");
    const result = await withTimeout(auth.getRedirectResult(instance), 30, "getRedirectResult");
    note(`redirect result: ${result ? "user " + result.user?.uid : "null"}; current user: ${instance.currentUser?.uid ?? "none"}`);
    const user = result?.user ?? instance.currentUser;
    if (!user) throw new Error("no-result");
    const idToken = await user.getIdToken();
    const claims = claimsOf(idToken);
    note(`token provider ${claims.firebase?.sign_in_provider}; auth_time ${claims.auth_time}; started ${config.startedAt}`);
    if (!Number.isFinite(claims.auth_time) || claims.auth_time < config.startedAt || claims.firebase?.sign_in_provider !== config.provider)
      throw new Error("stale-result");
    const tokenResult = await user.getIdTokenResult();
    note("sending session to the application");
    await window.__TAURI_INTERNALS__.invoke("session_capture", {
      captured: {
        uid: user.uid,
        email: user.email ?? null,
        displayName: user.displayName ?? null,
        providers: (user.providerData ?? []).map((item) => item.providerId),
        refreshToken: user.refreshToken,
        idToken,
        expiresAt: new Date(tokenResult.expirationTime).toISOString(),
      },
    });
    localStorage.removeItem(key);
    show("ログインしました。このウィンドウは自動的に閉じます。");
  })().catch((error) => {
    const code = error?.code ?? error?.message ?? String(error);
    note(`failed: ${code}${error?.customData?.message ? " " + error.customData.message : ""}`);
    const message =
      code === "no-result" || code === "auth/no-auth-event"
        ? `${providerName}からログイン結果を受け取れませんでした。`
        : code === "stale-result"
          ? "以前のログイン情報が残っています。もう一度ログインしてください。"
          : `ログインできませんでした（${code}）。`;
    show(message, true);
  });
})();
