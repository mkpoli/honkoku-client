(() => {
  const config = __SIGN_IN_CONFIG__;
  if (
    window.top !== window ||
    location.origin !== config.origin ||
    location.pathname !== "/__client_signin"
  )
    return;
  if (window.__HONKOKU_SIGNIN_READY__) return;
  Object.defineProperty(window, "__HONKOKU_SIGNIN_READY__", { value: true });
  const params = new URLSearchParams(location.search);
  const attemptId = params.get("event");
  const startedAt = Number(params.get("started"));
  const deadline = Number(params.get("deadline"));
  const key = `honkoku:signin:${attemptId}`;
  let phase;
  let instance;
  let auth;
  let persisted = false;

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
      for (const child of Array.from(document.body.children))
        child.style.display = "none";
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
      button.onclick = async () => {
        button.disabled = true;
        try {
          if (persisted) {
            await finish();
          } else {
            if (instance)
              await withTimeout(auth.signOut(instance), 15, "cleanup-timeout");
            await window.__TAURI_INTERNALS__.invoke("session_sign_in_retry", {
              attemptId,
            });
          }
        } catch {
          show(
            persisted
              ? "ログインは保存されました。ブラウザーの資格情報の消去を再試行してください。"
              : "再試行できませんでした。ウィンドウを閉じてログインし直してください。",
            true,
          );
        }
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
  window.addEventListener("error", () => note("script error"));
  window.addEventListener("unhandledrejection", () =>
    note("unhandled rejection"),
  );

  note(`page ${location.pathname}`);
  const errorCode = (error) => {
    const code = error?.code ?? error?.message;
    return new Set([
      "no-result",
      "stale-result",
      "invalid-attempt",
      "sdk-timeout",
      "auth-timeout",
      "result-timeout",
      "token-timeout",
      "cleanup-timeout",
      "auth/no-auth-event",
      "auth/network-request-failed",
      "auth/operation-not-allowed",
      "auth/unauthorized-domain",
      "auth/user-disabled",
    ]).has(code)
      ? code
      : "signin-failed";
  };
  const withTimeout = (promise, seconds, code) => {
    let timer;
    return Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(code)), seconds * 1000);
      }),
    ]).finally(() => clearTimeout(timer));
  };
  const checkAttempt = () => {
    if (
      !attemptId ||
      !Number.isFinite(startedAt) ||
      !Number.isFinite(deadline) ||
      startedAt <= 0 ||
      deadline - startedAt !== 600 ||
      Date.now() / 1000 >= deadline
    )
      throw new Error("invalid-attempt");
  };
  const finish = async () => {
    note("clearing browser credential");
    await withTimeout(auth.signOut(instance), 15, "cleanup-timeout");
    localStorage.setItem(key, "completed");
    show("ログインしました。このウィンドウは自動的に閉じます。");
    await window.__TAURI_INTERNALS__.invoke("session_sign_in_complete", {
      attemptId,
    });
  };

  (async () => {
    checkAttempt();
    phase = localStorage.getItem(key);
    note(
      `phase ${phase === "redirecting" ? "redirecting" : phase === null ? "initial" : "finished"}; provider ${config.provider}`,
    );
    if (phase !== null && phase !== "redirecting")
      throw new Error("invalid-attempt");
    show(`${providerName}のログインページを準備しています…`);
    const base = `https://www.gstatic.com/firebasejs/${config.sdkVersion}`;
    const [{ initializeApp }, authModule] = await withTimeout(
      Promise.all([
        import(`${base}/firebase-app.js`),
        import(`${base}/firebase-auth.js`),
      ]),
      20,
      "sdk-timeout",
    );
    note("SDK loaded");
    auth = authModule;
    const app = initializeApp(
      { apiKey: config.apiKey, authDomain: config.authDomain },
      "honkoku-client",
    );
    instance = auth.getAuth(app);
    await withTimeout(
      auth.setPersistence(instance, auth.browserLocalPersistence),
      30,
      "auth-timeout",
    );
    note("auth ready");
    const provider =
      config.provider === "google.com"
        ? new auth.GoogleAuthProvider()
        : new auth.TwitterAuthProvider();
    checkAttempt();
    if (phase === null) {
      await withTimeout(auth.signOut(instance), 15, "cleanup-timeout");
      checkAttempt();
      localStorage.setItem(key, "redirecting");
      show(`${providerName}のログインページへ移動します…`);
      await auth.signInWithRedirect(instance, provider);
      note("redirect requested");
      return;
    }
    show("ログイン結果を確認しています…");
    // The SDK only consults the auth iframe when its pending-redirect flag is
    // present in session storage, and the round trip through the provider
    // loses session storage in WebView2. The flag is restored here so the
    // result stored by the handler on this origin is actually collected.
    const pendingKey = `firebase:pendingRedirect:${config.apiKey}:honkoku-client`;
    note(
      `pending flag before: ${sessionStorage.getItem(pendingKey) === null ? "absent" : "present"}`,
    );
    sessionStorage.setItem(pendingKey, JSON.stringify("true"));
    const result = await withTimeout(
      auth.getRedirectResult(instance),
      30,
      "result-timeout",
    );
    note(
      `redirect result: ${result?.user ? "user present" : "none"}; current user: ${instance.currentUser ? "user present" : "none"}`,
    );
    const user = result?.user;
    if (!user) throw new Error("no-result");
    const tokenResult = await withTimeout(
      user.getIdTokenResult(),
      30,
      "token-timeout",
    );
    const claims = tokenResult.claims;
    if (
      !Number.isFinite(claims.auth_time) ||
      claims.auth_time < startedAt ||
      claims.firebase?.sign_in_provider !== config.provider
    )
      throw new Error("stale-result");
    checkAttempt();
    note("sending session to the application");
    await window.__TAURI_INTERNALS__.invoke("session_capture", {
      captured: {
        attemptId,
        uid: user.uid,
        refreshToken: user.refreshToken,
        idToken: tokenResult.token,
      },
    });
    persisted = true;
    await finish();
  })().catch((error) => {
    const code = errorCode(error);
    note(`failed: ${code}`);
    if (persisted) {
      show(
        "ログインは保存されました。ブラウザーの資格情報の消去を再試行してください。",
        true,
      );
      return;
    }
    const message =
      code === "no-result" || code === "auth/no-auth-event"
        ? `${providerName}からログイン結果を受け取れませんでした。`
        : code === "stale-result"
          ? "以前のログイン情報が残っています。もう一度ログインしてください。"
          : `ログインできませんでした（${code}）。`;
    show(message, true);
  });
})();
