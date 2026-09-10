# 0001 — Architecture

Status: adopted 2026-09-10. Basis: `architecture-brief.md`, `api-report.md`, `architecture-decision.md` (the consult's full reasoning).

## Adopted

| Area | Decision |
| --- | --- |
| Shell | Tauri 2.11 desktop application; mobile targets later through the same core |
| Frontend | Svelte 5 (runes) with Vite, hash-routed SPA, no SvelteKit, no CSS framework; handwritten CSS on three levels of tokens (primitive, semantic role, component alias) with light and dark assigning the same roles |
| Data | A Rust core owns authentication, remote access, the SQLite cache, and every write; the webview holds only the current view. Firestore is reached over its REST API with Firebase ID tokens; the Express `/api` routes serve public reads |
| Editing writes | The site's editing protocol is reproduced exactly: transactional lock acquisition, draft writes every three seconds, save and discard transactions, `timelineEvents` append. Firestore document `updateTime` is the write precondition; `updatedAt` alone is not enough |
| Facsimile | OpenSeadragon in the webview; a Rust fetcher with a bounded disk cache serves manifests, tiles, and thumbnails over a Tauri custom protocol |
| Editor | Vertical (`vertical-rl`) WYSIWYG projection of a lossless concrete syntax tree of the platform's markup; ProseMirror prototyped first, Lexical second; CodeMirror only for a raw-source mode. Parsing and normalisation live in a Rust text crate with a WASM build for the renderer |
| OCR | honkoku-ocr-py as a persistent Python sidecar behind a Rust job interface; a Rust port on `ort` is evaluated later against parity benchmarks |
| Search | Tantivy with positional unigram and bigram fields over normalised text, candidates verified against the page text; the public dump imported at a recorded commit with a live overlay |
| Repository | One Cargo workspace and one Bun workspace: `crates/{core,storage,iiif,text,search,ocr}`, `apps/{client,cli}`, `packages/{ui,editor,text-wasm,client-api}`, `sidecars/ocr-python`, `fixtures/`, `tools/` |

## Login

The platform offers Google and X sign-in only, through Firebase Auth popups, and the backend is fixed. Phase 1 proves login in this order:

1. A sign-in window inside the application that loads the platform's own login and hands the resulting Firebase session to the Rust core.
2. The system browser with a loopback callback and `signInWithIdp`, which needs the platform owner to register a desktop OAuth client.
3. An explicit handoff from a browser session (`tools/session/login.ts` is the development form).

Refresh tokens live in the OS credential store (Windows Credential Manager, macOS Keychain, Secret Service) with a Stronghold vault as the fallback.

## Product decisions carried into the design

- The home screen is a dashboard: project list with grouping and filters, activity timeline, ranking and own status. Progress is shown at every level of the hierarchy.
- Drill-down (project → collection → entry → page) pushes panes with a breadcrumb; project pages carry their own timeline.
- The workbench places the facsimile on the right and the editor on the left by default, so reading continues right-to-left from the original into the transcription; the sides can be swapped. Spreads split into 右丁 and 左丁. Pages form a filmstrip that expands on hover.
- Points, levels, and streaks appear on the home screen and at page completion only.

## Spikes before the surrounding UI is built

1. Two-client editing exercise against the real lock protocol (acquire, disconnect, same account on two devices, concurrent OCR write, lost commit response).
2. Vertical IME editing in the actual WebKitGTK, WebView2, and WKWebView webviews with the fixture set of ruby, 割書, corrections, gaps, and rare kana.
3. Cached IIIF delivery over the custom protocol: cold open, cached revisit, pan and zoom, canvas readback.
4. The three login routes above.
