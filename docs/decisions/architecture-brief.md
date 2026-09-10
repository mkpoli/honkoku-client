# Architecture brief: honkoku-client

honkoku-client is a native desktop application for みんなで翻刻 (Minna de Honkoku, app.honkoku.org), a Japanese crowdsourced transcription platform for pre-modern documents written in くずし字. The application talks to the platform's existing backend, so a user who signs in with their platform account edits the same data as on the website. The platform's author has approved the work, and the aim is for successful ideas to flow back into the official site later.

Read `api-report.md` in this directory before answering: it is a reverse-engineered description of the backend contract (REST routes, Firestore collections, auth, storage) taken from the site's production JavaScript bundle. Treat it as the ground truth for what the backend offers.

## What the application must do

Phase 1 (first milestone)

- Sign in with the platform's Firebase account (Firebase project honkoku3-c466c) and hold the session across restarts.
- Browse projects → collections → entries (books or documents) → pages, with progress figures, thumbnails, and status per page (未着手 / 翻刻中 / 完了).
- Show one page: the IIIF facsimile (each holding institution runs its own IIIF Image API server, versions 1 to 3) beside the transcription text, with page navigation.
- Light and dark themes, Japanese UI, keyboard-first.

Phase 2

- Edit transcriptions. The text uses a lightweight markup (the platform's own, close to the Koji language: 《割書：a｜b》, 《振り仮名：base｜ruby》, 《見せ消ち：a｜b》, 《圏点：x｜◯》, 【右丁】/【左丁】 page-half markers, 【注記】, ■□〓 gaps, ＿レ ＿一 返り点, ￣ 送り仮名, ※ comments, #1 note references). The editor must be vertical (縦書き, columns right to left), render the markup as what it means while editing (WYSIWYG), round-trip the raw text exactly, insert special marks and 変体仮名 quickly, and keep the edited column aligned with the facsimile. Saving must be safe against another user editing the same page.
- Notes, annotations, page status changes, review requests.

Phase 3

- Local OCR on the GPU. A Python port of the platform's OCR already exists (honkoku-ocr-py: RTMDet line detection, ConvNeXt V2 encoder, RoBERTa decoder, onnxruntime, CUDA; about 1.5 s per spread on an RTX 5070 Ti). Options are a Python sidecar, a Rust port on the `ort` crate, or in-webview onnxruntime-web.
- Concordance search (KWIC) over the whole platform corpus: the public text dump is 46.87 million characters (GitHub yuta1984/honkoku-data, v3, one txt per page in project/entry/page hierarchy) plus live changes. Needs CJK-aware indexing, normalisation of variant characters (旧字体/新字体, 変体仮名, 踊り字), and a fast local index.
- A 集字 tool: for a character, show attested glyph images from the corpus (CODH's 日本古典籍くずし字データセット char-shape pages and the platform's own OCR line boxes) beside the editor.
- Gamification: points, levels, streaks with sound effects and small celebrations; the platform already has points and levels.

Later: the same code on iOS and Android through Tauri's mobile targets, so decisions now should not close that door.

## Fixed constraints

- Tauri 2 shell. Rust 1.96, Bun 1.3 are the toolchains. Linux (WSL2 with WebKitGTK 2.52), Windows (WebView2), macOS (WKWebView) all matter; Linux WebKitGTK has no WebGPU and lags on some CSS.
- The backend cannot be changed in phase 1. Everything goes through what the bundle shows: Firebase Auth, Firestore (client-side reads and writes under security rules), Firebase Storage, and the Express REST API.
- Firestore has public read on `transcriptions`; writes need an ID token. The REST API answers unauthenticated for public data.
- The user's earlier code that can be reused: a Svelte 5 userscript with the special-mark palette and 変体仮名 / 異体字 tables (honkoku-toolbox); a Bun harvester that already walks project → collection → entry → Firestore pages; a Python collation tool with a vertical-text web viewer and a variant-equivalence table; honkoku-ocr-py.
- The application must feel fast: the website is criticised for slowness, inconsistent UI, an editor that mis-inserts marks and loses highlights, and pages that fall out of sync with the image.

## Versions on the build machine (September 2026)

Rust 1.96.0, Bun 1.3.14, WebKitGTK 2.52.6, CUDA-capable RTX 5070 Ti with 16 GB. Registry versions: tauri 2.11.5, @tauri-apps/cli 2.11.4, @tauri-apps/api 2.11.1, svelte 5.57.0, @sveltejs/kit 2.70.3, vite 8.2.2, tailwindcss 4.3.3, prosemirror-view 1.42.3, @tiptap/core 3.31.3, codemirror 6.0.2, openseadragon 6.1.1, ort 2.0.0-rc.13 (ONNX Runtime 1.28), tantivy 0.26.2, rusqlite 0.40.2, reqwest 0.13.5, tauri-plugin-stronghold 2.3.2. The local copy of the text dump is 833 MB in 281,032 page files.

## Decisions to make

Give a decision for each, the reasoning, and the single risk that decides it. Where a prototype would settle it faster than argument, say what to prototype and what result would change the decision.

1. Frontend framework and styling: Svelte 5 (the user's habit) versus SolidJS or React; hand-written CSS with design tokens versus Tailwind 4 / UnoCSS; routing and data-fetching libraries; how the design tokens for light/dark are organised.
2. Where the data layer lives. Options: (a) Firebase JS SDK in the webview with IndexedDB persistence and real-time listeners; (b) a Rust core (reqwest, Firestore REST, local SQLite cache) exposing Tauri commands, no Firebase SDK; (c) a hybrid where the Rust core owns the cache and writes and the webview keeps a listener only on the page being edited. Consider offline reading, write queues, conflict detection with `updatedAt`, and reuse of the core by a future CLI.
3. Authentication inside a Tauri webview: Firebase JS SDK popup/redirect flows are unreliable in webviews; alternatives are the system browser with a deep link back (`signInWithIdp` on the Identity Toolkit REST API), email/password through REST, or an embedded login page. Also how to store the refresh token per platform (keyring/stronghold) and how to reuse a session from an already signed-in browser on the same machine.
4. Facsimile pipeline: OpenSeadragon in the webview hitting IIIF servers directly versus a Rust-side tile fetcher with a disk cache served over a custom URI scheme, so pages open instantly on revisit and OCR runs on cached full images. Consider institutions' rate limits and CORS, and WebKitGTK's canvas performance.
5. The vertical WYSIWYG editor: ProseMirror/Tiptap, Lexical, CodeMirror 6, Slate, or a custom contenteditable, given `writing-mode: vertical-rl`, IME composition for Japanese, exact round-trip to the platform's markup, decorations for OCR confidence, and alignment with detected line boxes on the image. Name what to prototype first.
6. OCR execution: Python sidecar (honkoku-ocr-py, works today), Rust port on `ort` with CUDA/DirectML/CoreML execution providers, or onnxruntime-web. Consider packaging size, GPU availability per platform, and the mobile future.
7. Concordance index: SQLite FTS5 (trigram tokenizer) versus tantivy versus a purpose-built n-gram index in Rust; where normalisation happens; how the 47M-character dump plus live updates are kept current; KWIC rendering for vertical text.
8. Repository layout, build and release: monorepo shape (Rust crates for core/ocr/search, the Tauri app, shared TypeScript packages), CI matrix, updater, code signing, how a headless CLI shares the core.
9. Anything in the backend contract that makes one of the above impossible or risky, and any capability the report suggests the site has that the plan above has missed.

Answer in plain prose with a short table per decision (option, verdict, deciding risk), then a proposed directory layout and a phase-1 task list ordered by dependency. Name exact crate and package versions current as of September 2026 where you are confident; mark the rest as "check". Do not write any code or create files.
