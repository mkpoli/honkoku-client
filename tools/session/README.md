# Session tools

Two Bun scripts for working with a platform account during development. Both keep their state under `~/.local/share/honkoku-client/`.

## login.ts

```
bun tools/session/login.ts [--url URL]
```

Opens a visible Chromium with a persistent profile on app.honkoku.org (the login page by default, or `URL`). Once Firebase Auth holds a user, the account's refresh token and profile are written to `session.json` (mode 0600), and every request the site makes to its backends is appended to `capture.jsonl` until the window is closed. The capture covers app.honkoku.org/api, Firestore, Identity Toolkit, Secure Token, Storage, Cloud Functions and Cloud Run hosts, and the Metom recognition API, with request and response headers and bodies.

The CLI imports the session with `honkoku login --import`; the desktop client imports it through its ログイン button until it has a sign-in flow of its own.

## token.ts

```
bun tools/session/token.ts [--claims]
```

Prints a fresh ID token for the saved session, refreshing through the Secure Token API when fewer than five minutes remain, or the decoded token payload with `--claims`.

## Playwright

The scripts use the `playwright` package pinned in `package.json`; its Chromium build must be present in `~/.cache/ms-playwright`. `bunx playwright install chromium` fetches it.
