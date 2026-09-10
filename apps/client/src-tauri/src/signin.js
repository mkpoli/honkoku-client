(() => {
  const config = __SIGN_IN_CONFIG__;
  if (window.top !== window || location.origin !== config.origin) return;
  const key = `honkoku:signin:${config.eventId}`;
  const entry = location.pathname === "/__client_signin";
  const phase = sessionStorage.getItem(key);
  if (!entry && !phase) return;
  Object.defineProperty(window, "__HONKOKU_SIGNIN_READY__", { value: true });

  const providerName = config.provider === "google.com" ? "Google" : "X";
  let box = null;
  const show = (message, retry) => {
    const render = () => {
      document.title = "ログイン";
      if (!box) {
        document.body.innerHTML = "";
        box = document.createElement("div");
        box.setAttribute(
          "style",
          "min-height:100vh;display:flex;align-items:center;justify-content:center;background:#F4F2EE;color:#202632;font:16px/1.7 'Noto Sans JP','Hiragino Sans','Yu Gothic',sans-serif;padding:32px;text-align:center",
        );
        document.body.appendChild(box);
      }
      box.innerHTML = "";
      const text = document.createElement("p");
      text.textContent = message;
      box.appendChild(text);
      if (retry) {
        const button = document.createElement("button");
        button.textContent = "もう一度試す";
        button.setAttribute(
          "style",
          "display:block;margin:16px auto 0;padding:8px 16px;border:1px solid #D95936;border-radius:6px;background:#D95936;color:#fff;font:inherit;cursor:pointer",
        );
        button.onclick = () => {
          sessionStorage.removeItem(key);
          location.href = `${config.origin}/__client_signin?provider=${encodeURIComponent(config.provider)}`;
        };
        box.appendChild(button);
      }
    };
    if (document.body) render();
    else document.addEventListener("DOMContentLoaded", render, { once: true });
  };

  const claimsOf = (token) => {
    const encoded = token.split(".")[1].replace(/-/g, "+").replace(/_/g, "/");
    return JSON.parse(atob(encoded));
  };

  (async () => {
    show(`${providerName}のログインページを準備しています…`);
    const base = `https://www.gstatic.com/firebasejs/${config.sdkVersion}`;
    const [{ initializeApp }, auth] = await Promise.all([
      import(`${base}/firebase-app.js`),
      import(`${base}/firebase-auth.js`),
    ]);
    const app = initializeApp({ apiKey: config.apiKey, authDomain: config.authDomain }, "honkoku-client");
    const instance = auth.getAuth(app);
    await auth.setPersistence(instance, auth.browserSessionPersistence);
    const provider = config.provider === "google.com" ? new auth.GoogleAuthProvider() : new auth.TwitterAuthProvider();
    if (!phase) {
      sessionStorage.setItem(key, "redirecting");
      show(`${providerName}のログインページへ移動します…`);
      await auth.signInWithRedirect(instance, provider);
      return;
    }
    show("ログイン結果を確認しています…");
    const result = await auth.getRedirectResult(instance);
    const user = result?.user ?? instance.currentUser;
    if (!user) throw new Error("no-result");
    const idToken = await user.getIdToken();
    const claims = claimsOf(idToken);
    if (!Number.isFinite(claims.auth_time) || claims.auth_time < config.startedAt || claims.firebase?.sign_in_provider !== config.provider)
      throw new Error("stale-result");
    const tokenResult = await user.getIdTokenResult();
    await window.__TAURI_INTERNALS__.invoke("session_capture", {
      captured: {
        uid: user.uid,
        email: user.email ?? null,
        displayName: user.displayName ?? null,
        providers: (user.providerData ?? []).map((entry) => entry.providerId),
        refreshToken: user.refreshToken,
        idToken,
        expiresAt: new Date(tokenResult.expirationTime).toISOString(),
      },
    });
    sessionStorage.removeItem(key);
    show("ログインしました。このウィンドウは自動的に閉じます。");
  })().catch((error) => {
    const code = error?.code ?? error?.message ?? String(error);
    const message =
      code === "no-result" || code === "auth/no-auth-event"
        ? `${providerName}からログイン結果を受け取れませんでした。`
        : code === "stale-result"
          ? "以前のログイン情報が残っています。もう一度ログインしてください。"
          : `ログインできませんでした（${code}）。`;
    show(message, true);
  });
})();
