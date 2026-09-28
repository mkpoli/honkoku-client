// ==UserScript==
// @name         みんなで翻刻クライアントで開く
// @namespace    https://github.com/mkpoli/honkoku-client
// @version      1.0.0
// @description  みんなで翻刻の画面に、同じ資料やコマをデスクトップクライアントで開くボタンを加えます。
// @match        https://app.honkoku.org/*
// @grant        none
// @run-at       document-idle
// @homepageURL  https://github.com/mkpoli/honkoku-client
// @downloadURL  https://raw.githubusercontent.com/mkpoli/honkoku-client/main/apps/userscript/open-in-client.user.js
// @updateURL    https://raw.githubusercontent.com/mkpoli/honkoku-client/main/apps/userscript/open-in-client.user.js
// ==/UserScript==

(() => {
  "use strict";
  // The client maps each website address to its own screen, so the link is
  // the current address under the client's scheme.
  const clientLink = () =>
    location.href.replace(/^https:\/\//, "honkoku-client://");

  const style = document.createElement("style");
  style.textContent = `
    #honkoku-client-open {
      --bg: #fff; --fg: #1f1b16; --line: rgba(0,0,0,.18); --hover: #f3eee6;
      position: fixed; left: 16px; bottom: 16px; z-index: 1200;
      display: inline-flex; align-items: center; gap: 6px;
      padding: 6px 12px 6px 10px; border: 1px solid var(--line); border-radius: 999px;
      background: var(--bg); color: var(--fg);
      font: 500 13px/1.4 system-ui, "Hiragino Sans", "Yu Gothic UI", sans-serif;
      text-decoration: none; box-shadow: 0 2px 6px rgba(0,0,0,.12); cursor: pointer;
    }
    #honkoku-client-open:hover { background: var(--hover); }
    #honkoku-client-open:focus-visible { outline: 2px solid #1976d2; outline-offset: 2px; }
    #honkoku-client-open svg { width: 16px; height: 16px; flex: none; }
    @media print { #honkoku-client-open { display: none; } }
  `;
  document.head.append(style);

  const link = document.createElement("a");
  link.id = "honkoku-client-open";
  link.href = clientLink();
  link.title = "この画面をみんなで翻刻クライアントで開く";
  link.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 21h8M12 17v4"/></svg><span>クライアントで開く</span>`;
  // The site changes its address without reloading, so refresh the target
  // whenever the button may be used.
  for (const event of ["pointerenter", "focus", "click"])
    link.addEventListener(event, () => (link.href = clientLink()));
  document.body.append(link);
})();
