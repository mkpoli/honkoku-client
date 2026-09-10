(() => {
  if (window.top !== window || location.origin !== "https://app.honkoku.org") return;
  const { apiKey, provider, startedAt, eventId } = __SIGN_IN_CONFIG__;
  const marker = `honkoku:redirect:${eventId}`;
  if (!sessionStorage.getItem(marker)) {
    // Firebase consumes this JSON-encoded string before completing the redirect.
    sessionStorage.setItem(`firebase:pendingRedirect:${apiKey}:[DEFAULT]`, JSON.stringify("true"));
    sessionStorage.setItem(marker, "true");
  }
  Object.defineProperty(window, "__HONKOKU_SIGNIN_READY__", { value: true });
  let capturing = false;
  const readUser = () => new Promise((resolve) => {
    const open = indexedDB.open("firebaseLocalStorageDb");
    open.onerror = () => resolve(null);
    open.onupgradeneeded = () => open.transaction.abort();
    open.onsuccess = () => {
      const db = open.result;
      try {
        const transaction = db.transaction("firebaseLocalStorage", "readonly");
        transaction.oncomplete = transaction.onabort = () => db.close();
        const request = transaction.objectStore("firebaseLocalStorage").get(`firebase:authUser:${apiKey}:[DEFAULT]`);
        request.onsuccess = () => resolve(request.result?.value ?? null);
        request.onerror = () => resolve(null);
      } catch {
        db.close();
        resolve(null);
      }
    };
  });
  const timer = setInterval(async () => {
    if (capturing) return;
    capturing = true;
    try {
      const user = await readUser();
      const tokens = user?.stsTokenManager;
      if (!tokens?.refreshToken || !tokens.accessToken) return;
      const encoded = tokens.accessToken.split(".")[1].replace(/-/g, "+").replace(/_/g, "/");
      const claims = JSON.parse(atob(encoded));
      // A persisted account from an earlier sign-in must not win the redirect race.
      if (!Number.isFinite(claims.auth_time) || claims.auth_time < startedAt || claims.firebase?.sign_in_provider !== provider) return;
      await window.__TAURI_INTERNALS__.invoke("session_capture", {
        captured: {
          uid: user.uid,
          email: user.email ?? null,
          displayName: user.displayName ?? null,
          providers: (user.providerData ?? []).map((entry) => entry.providerId),
          refreshToken: tokens.refreshToken,
          idToken: tokens.accessToken,
          expiresAt: new Date(tokens.expirationTime).toISOString(),
        },
      });
      clearInterval(timer);
    } catch {
      // Navigation, database creation, or persistence can race this poll; retry.
    } finally {
      capturing = false;
    }
  }, 1000);
})();
