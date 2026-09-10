I recommend **Svelte 5 with handwritten token-based CSS; a Rust-owned data layer and SQLite cache; OpenSeadragon backed by a native image cache; a ProseMirror editor prototype; the existing Python OCR engine; and Tantivy for concordance search**.

Three requirements need early proof: production authentication without backend changes, lossless vertical editing with Japanese IMEs, and complete corpus synchronization. The report does not establish all three.

Versions below use the brief’s supplied September 2026 registry inventory. **“Check”** means the exact dependency version or compatibility remains unverified. The prototypes and performance targets below are proposals; I did not execute them or change files.

**1. Frontend and styling.** Choose Svelte 5.57.0 with Vite 8.2.2. The reusable toolbox and the developer’s familiarity are stronger reasons than speculative framework performance differences. Keep the editor and image viewer outside Svelte’s reactive DOM ownership.

| Option | Verdict | Deciding risk |
|---|---|---|
| Svelte 5 | Choose | Reactive updates accidentally rebuild the editor |
| SolidJS | Decline | Migration effort without a demonstrated benefit |
| React | Decline | Additional adaptation of the existing Svelte tools |
| Handwritten CSS and design tokens | Choose | Inconsistent use of semantic tokens |
| Tailwind 4.3.3 / UnoCSS, version check | Defer | Utility conventions complicate specialized text layout |

Use a Vite SPA with a small, typed route table for library selections and the workbench. Hash routes are sufficient for desktop navigation, restored sessions, and back/forward history; translate incoming native links into those routes. SvelteKit 2.70.3 becomes worthwhile if nested routing and route-level loading grow substantially. It is unnecessary for the initial application.

Use no data-fetching library initially. Rust owns request deduplication, freshness, persistence, and cancellation; Svelte stores hold the current view of that data. Adding TanStack Svelte Query, version **check**, would introduce another cache lifecycle to coordinate. Reconsider it if a future browser application needs a substantial independent transport layer.

Organize tokens in three levels: primitive palette values; semantic roles such as canvas, panel, text, muted text, border, focus, selection, and status; then a few component-specific aliases. Light and dark themes assign the same semantic roles. Apply the saved theme before first paint, with a system-following option.

Use CSS logical properties throughout. Keep manuscript pixels unchanged in dark mode, as all four explorations specify. Status must have text or a symbol as well as color. Font fallbacks must cover supplementary-plane kana and variation sequences.

Prototype a populated library and mounted workbench on WebKitGTK first. Change the rendering approach only if profiling identifies framework work as the bottleneck after unnecessary updates have been removed.

**2. Data ownership.** Choose option **(b)**: Rust owns authentication, remote access, SQLite, and every write. Use `reqwest` 0.13.5 and `rusqlite` 0.40.2. Keep the core independent of Tauri so the CLI can call the same operations.

| Option | Verdict | Deciding risk |
|---|---|---|
| Firebase JS SDK with IndexedDB | Decline as primary architecture | Durable behavior becomes tied to a webview profile |
| Rust REST client and SQLite | Choose | Incorrect reproduction of the platform’s write protocol |
| Rust plus a page listener in JS | Reserve | Two transports deliver conflicting or stale state |

Firestore REST accepts Firebase ID tokens and evaluates Security Rules for those requests. This architecture does not require distributing a service account. [Firestore REST authentication](https://firebase.google.com/docs/firestore/use-rest-api)

Cache remote documents with their original fields, Firestore document `updateTime`, application `updatedAt`, fetch time, and account/visibility scope. Keep local drafts separately from cached server snapshots. Use bounded queries for the visible hierarchy, adjacent-page prefetch, and immediate cached rendering followed by revalidation. Offline availability means downloaded material; it does not imply the whole platform is present.

For phase 1, poll only the active page while visible, initially every few seconds, with backoff and suspension in the background. Refresh on focus and navigation. Polling controls display freshness; transactions control write safety. If later live-preview latency or measured read volume makes polling unsuitable, add option (c) as a narrow invalidation source. Public transcription listeners could operate anonymously under the stated rules; authenticated listeners need a supported credential bridge. A Firebase ID token is not a Firebase custom token.

Phase 2 must implement the report’s editing protocol:

- Acquire the lock transactionally, including the project checks and initialization of `prevStatus`, `tempText`, `tempNotes`, `tempEditedBy`, and `syncMode`.
- Persist local drafts promptly. Send coalesced remote draft updates at approximately the site’s three-second cadence while ownership remains valid.
- Save through a transaction that rechecks ownership and preserves the site’s status, review, approval, diff-count, and timeline behavior.
- Discard through the corresponding transaction. Preserve unknown fields and nullable note positions.

**`updatedAt` alone is insufficient for conflict detection.** The report’s draft updates do not necessarily change it; OCR writes do. Use Firestore’s document `updateTime` as an atomic write precondition, together with transaction reads and semantic checks against the acknowledged base. Preserve timestamp precision. [Firestore preconditions](https://firebase.google.com/docs/firestore/reference/rest/v1/Precondition)

The write queue should contain durable intentions, base revisions, and operation IDs. Reconnection first refreshes permissions, ownership, and content. Conflicts retain the draft for reconciliation. A timeout after a final save requires checking whether that save committed before retrying. Persist one timeline-event ID per save attempt and use it for recovery, subject to the actual rules.

No lock lease or expiration is evidenced. Do not infer abandonment from elapsed time. The lock identifies a UID, so two devices signed into the same account also require careful draft-change detection.

The deciding prototype is a two-client editing exercise covering simultaneous acquisition, disconnects, same-account devices, concurrent OCR updates, and a lost commit response. Any silent overwrite or duplicate timeline event blocks phase-2 writes. Switching transports would not remove that obligation.

**3. Authentication.** Choose the system browser, with Rust retaining the Firebase session. The production callback path is an unresolved prerequisite.

| Option | Verdict | Deciding risk |
|---|---|---|
| System browser and provider credential exchange | Preferred | Existing OAuth configuration may not permit the callback |
| Explicit browser-session bridge/import | Phase-1 fallback | Dependence on browser integration and Firebase storage details |
| Email/password REST | Unavailable on present evidence | The platform exposes only Google and X login |
| Embedded login or webview popup/redirect | Decline | Provider and webview restrictions |
| OS credential storage / Stronghold | OS storage first | Availability of a secure unlock mechanism |

`signInWithIdp` exchanges an already obtained provider credential for a Firebase session. It does not itself solve browser authorization. Google requires a suitable OAuth client and redirect configuration; Firebase’s documented Twitter flow uses an OAuth access token and token secret. Do not assume a generic PKCE flow covers both providers. [Firebase Auth REST](https://firebase.google.com/docs/reference/rest/auth), [Twitter authentication](https://firebase.google.com/docs/auth/web/twitter-login)

Prefer an authorization-code callback through a desktop loopback listener where supported. Validate state, use PKCE where applicable, and send only short-lived handoff material through links. Google documents loopback redirects for desktop OAuth clients; mobile needs its own supported callback mechanism. [Google native-app OAuth](https://developers.google.com/identity/protocols/oauth2/native-app)

Under the strict no-backend-change constraint, a browser companion can provide an explicit session handoff from the already signed-in site. `tools/session/login.ts` implements a development version using a dedicated Chromium profile and Firebase IndexedDB inspection. It does **not** reuse the user’s ordinary browser profile. A production bridge needs an installed browser integration and an authenticated, one-use handoff; merely opening the website cannot transfer its session.

The current helper writes plaintext session JSON and captures request headers and bodies. Those are development facilities. Reuse the refresh logic, while replacing token persistence and excluding credentials from diagnostics.

Store refresh tokens through Windows Credential Manager, macOS Keychain, and Linux Secret Service using `keyring`, version **check**. Where Secret Service is unavailable, including some WSL setups, offer Stronghold 2.3.2 with a user-unlocked vault. Stronghold’s encryption still needs an unlock secret; embedding that secret beside the vault provides little protection. Mobile should use native Keychain/Keystore adapters behind the same core interface.

Refresh in Rust through one serialized refresh operation, persist any replacement refresh token atomically, and expose only account state to the UI. Initialize `users/{uid}` only when absent, preserving the report’s defaults.

Prototype both Google and X login, restart, refresh, revocation, account switching, and browser-session handoff. If neither existing callback configuration nor an acceptable companion bridge works, seamless phase-1 login requires platform-owner configuration work. Password login is not an evidenced fallback.

**4. Facsimile pipeline.** Choose OpenSeadragon 6.1.1 for viewing, with Rust fetching and caching manifests, image information, thumbnails, and tiles.

| Option | Verdict | Deciding risk |
|---|---|---|
| OpenSeadragon fetching institutions directly | Useful baseline | CORS prevents consistent image access |
| OpenSeadragon with Rust cache and custom protocol | Choose | Protocol delivery performs poorly in a target webview |
| Native viewer replacement | Defer | Separate rendering implementations expand scope |

Keep IIIF Presentation parsing separate from Image API negotiation. Resolve manifests into ordered canvases and image services, then join transcription documents using verified index conventions and `canvasId` where available. Preserve language-map labels and handle missing images explicitly.

Retain upstream service metadata. Translate image requests into opaque local resource URLs backed by a Rust service registry. Serve binary responses through a Tauri custom protocol, avoiding base64 image transfers through commands. Verify origin headers, CSP, cancellation, and canvas readback on all three webviews.

OpenSeadragon documents older IIIF 1.x support as well as modern tile sources. Exercise actual fixtures for Image API versions 1–3, static/level-0 services, encoded identifiers, and restricted image sizes. [OpenSeadragon IIIF support](https://openseadragon.github.io/examples/tilesource-iiif/)

Use per-host concurrency limits, request deduplication, HTTP validators, `Retry-After`, and a bounded disk cache. Prioritize the viewport over prefetch. Restrict the image fetcher to registered HTTP(S) resources and validate redirects; arbitrary manifests must not turn it into unrestricted local-network access.

A tile cache guarantees fast access only to cached resolutions and regions. Explicit offline downloads should include a useful whole-page rendition. OCR should request an appropriate full-area image separately and retain its dimensions and transform to source coordinates. IIIF 3’s `max` may still be constrained by service limits. [IIIF image sizing](https://iiif.io/api/image/3.0/#42-size)

Prototype a large spread with overlays on WebKitGTK, testing cold access, cached revisit, pan/zoom, and canvas extraction. Target a cached useful image within 200 ms and responsive interaction at least around 30 fps on the baseline machine. If custom-protocol overhead is the cause of failure, test direct fetching for compatible hosts while retaining native caching for offline images and OCR.

**5. Vertical editor.** Prototype **direct ProseMirror** first, using `prosemirror-view` 1.42.3. Treat this as a conditional choice.

| Option | Verdict | Deciding risk |
|---|---|---|
| Direct ProseMirror | Prototype first | Vertical selection and IME behavior |
| Tiptap 3.31.3 | Defer wrapper | Its abstractions may complicate lossless source mapping |
| Lexical, version check | Second candidate | Unproven vertical composition behavior |
| CodeMirror 6.0.2 | Raw-source mode | Its viewport geometry assumes horizontally written lines |
| Slate, version check | Decline | Custom behavior plus React integration |
| Custom `contenteditable` | Last resort | Owning composition, selection, and undo correctness |

Neither ProseMirror nor a wrapper establishes vertical correctness merely by accepting `writing-mode: vertical-rl`. ProseMirror contains direction-specific coordinate logic; CodeMirror’s viewport interfaces are organized around document height and top-to-bottom line positions. These justify an early prototype, rather than a promise of compatibility. [ProseMirror coordinate implementation](https://github.com/ProseMirror/prosemirror-view/blob/master/src/domcoords.ts), [CodeMirror viewport implementation](https://github.com/codemirror/view/blob/main/src/editorview.ts)

The first prototype should contain editable ruby, two-to-four-part warigaki, corrections, notes, page-half boundaries, gaps, rare kana, and ordinary Japanese text. Exercise Windows Microsoft IME, macOS Japanese input, and the intended Linux IME inside the actual Tauri webviews.

Its acceptance criteria are:

- No lost or duplicated text through composition, reconversion, cancellation, undo, or redo.
- Correct caret movement, selection, candidate-window placement, and scrolling across vertical columns.
- Reliable insertion from the palette without losing the selection.
- Exact unchanged-source round trips, including malformed and unfamiliar markup.
- Localized edits that preserve unrelated source spelling, punctuation, whitespace, and line endings.

Use the raw source and a **lossless concrete syntax tree** as the persistent representation. The ProseMirror document is its editable projection. Translate editor transactions into localized source patches, retaining original source slices for unchanged syntax. Source patches, semantic edits, and selection changes need one coordinated undo history.

Do not rebuild the editor DOM during composition. Ruby and warigaki must permit editing their constituent text; treating every construct as an indivisible widget would leave much of the required editing experience unfinished.

The brief and report disagree on some spellings. The report gives `未（いまだ｜ズ）` for ruby and `＃１０` for footnotes; the brief names additional forms. Preserve every encountered form, and establish insertion syntax from verified grammar fixtures. The design explorations’ expansions of `ゟ` and `ヿ` must remain display transformations with source mappings.

Put parsing and normalization in a framework-independent Rust text crate, with a WASM interface for local renderer use. Editing must not wait for a Tauri command per keystroke. Additional ProseMirror modules and WASM tooling are **check**.

Align text through stable logical-line anchors mapped to image-space boxes. Visual wrapping can split one manuscript line across several display columns. Track that relationship explicitly, including page halves, rotation, and scale. Keep confidence and alignment metadata local until a compatible backend shape is established.

If ProseMirror needs a broad rewrite of its selection/composition internals, test Lexical against the same fixtures. If neither passes, phase 2 requires dedicated editor engineering. A raw editor with a vertical preview can assist development, but does not fulfill the requested WYSIWYG milestone.

**6. OCR.** Ship the existing Python engine first, behind a replaceable Rust job interface.

| Option | Verdict | Deciding risk |
|---|---|---|
| Python sidecar | Choose initially | Distribution of runtime and GPU dependencies |
| Rust `ort` 2.0.0-rc.13 | Prototype later | Model and decoding parity across providers |
| `onnxruntime-web`, version check | Optional limited fallback | No WebGPU on the required Linux webview |

Run a persistent worker so model loading is amortized. Rust owns jobs, cancellation, model versions, image references, and result persistence. Exchange control messages over a versioned local protocol and pass image references without repeatedly serializing pixels through JavaScript.

Package the Python runtime independently of a user’s system Python. Make models and GPU runtimes optional components with checksums and explicit disk requirements. The reader should start without loading OCR dependencies.

Use CUDA where available and validated. Offer CPU fallback elsewhere; test DirectML and CoreML against the actual detector, encoder, and decoder before advertising acceleration. Provider availability does not establish full graph support or useful performance. ONNX Runtime also now describes DirectML as being in sustained engineering, so evaluate its Windows deployment guidance when beginning a port. [Execution providers](https://onnxruntime.ai/docs/execution-providers/), [DirectML guidance](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)

For the Rust prototype, pin the brief’s ONNX Runtime 1.28 pairing and compare final text, boxes, confidence, reading order, cold start, peak memory, and steady-state latency against `honkoku-ocr-py`. Switch when parity is acceptable and packaging or resource use materially improves. A successful model load is insufficient.

Keep mobile behind the same OCR capability interface, initially unavailable or limited. A later native implementation can use platform-appropriate providers without changing the editor. The Python sidecar itself is not the mobile portability plan.

**7. Concordance.** Choose Tantivy 0.26.2 with a purpose-built tokenizer and query planner.

| Option | Verdict | Deciding risk |
|---|---|---|
| SQLite FTS5 trigram | Decline as primary index | One- and two-character searches |
| Tantivy with CJK n-grams | Choose | Incorrect positional and normalization semantics |
| Fully custom Rust index | Reserve | Maintaining its storage and update machinery |

One-character queries are fundamental to 集字 and philological search. FTS5 trigram full-text queries do not match substrings shorter than three Unicode characters; relevant `LIKE` queries can fall back to scans. [SQLite FTS5 trigram behavior](https://www.sqlite.org/fts5.html#the_trigram_tokenizer)

Start with positional unigram and bigram fields, exact page identifiers, and project/entry filters. Use grams to find candidates, then verify exact occurrences against the normalized page text. Tantivy’s built-in `NgramTokenizer` assigns position zero to its grams, so phrase semantics require additional implementation. [Tantivy n-gram tokenizer](https://docs.rs/tantivy/latest/tantivy/tokenizer/struct.NgramTokenizer.html)

Normalize in the shared Rust text crate during indexing and querying. Keep original text, searchable text, and mappings back to original spans. Version the equivalence tables and index format together. Strict and variant-folded search should remain distinguishable. Iteration-mark expansion is contextual; historical variants and kana expansions cannot safely be reduced to blanket Unicode normalization.

Define whether searches include base text, ruby, annotations, or comments. Parse these fields structurally. Preserve byte, Unicode, and JavaScript UTF-16 offset conversions, including supplementary kana and variation sequences.

Import the dump at a recorded Git commit, with per-page hashes. Apply subsequent dump changes incrementally. Maintain a live overlay from confirmed saves and refreshed pages. `timelineEvents` can identify candidates for refreshing, but the report does not establish a complete change stream.

Probe an `updatedAt` query with a document-ID tie-breaker and overlapping checkpoints. Its index and rule compatibility remain **check**. Such a query still misses hard deletions and documents without the field. Periodic inventory reconciliation is necessary. Without a trustworthy dump snapshot timestamp, the initial gap between dump and live state also cannot be declared fully covered. Display freshness and coverage honestly.

Render KWIC from original text using mapped match spans. Offer virtualized vertical strips with aligned keywords, alongside a compact horizontal result list for scanning. Opening a hit must identify the exact page and occurrence.

Benchmark the complete supplied corpus, including common single-character queries, rare kana, variant expansion, and live replacements. Proposed targets are first results under 100 ms warm and a configurable build-memory budget around 1 GB. Index size must be measured separately from the dump’s 833 MB filesystem footprint. Consider custom storage only if Tantivy cannot meet measured size or latency requirements after tuning.

**8. Repository, builds, and releases.** Use one Cargo workspace and one Bun workspace, with a thin Tauri shell.

| Option | Verdict | Deciding risk |
|---|---|---|
| Shared core with thin desktop and CLI adapters | Choose | Platform dependencies leak into the core |
| Business logic inside Tauri commands | Decline | Headless and mobile reuse becomes difficult |
| Single universal OCR package | Decline | Installer size and unsupported GPU dependencies |

Pin Rust 1.96.0 and Bun 1.3.14. Use `tauri` 2.11.5, `@tauri-apps/cli` 2.11.4, and `@tauri-apps/api` 2.11.1 from the brief. Commit both lockfiles. Bun supplies package management and build tooling; it is not an application runtime dependency.

Generate TypeScript transport types from Rust definitions, with the generator version **check**. The CLI calls the same core services directly. Keep token storage, paths, OCR engines, and notifications behind platform interfaces. Coordinate desktop and CLI access so they cannot independently dispatch conflicting writes.

CI should build Linux x86-64, Windows x86-64, and macOS arm64/x86-64 releases, plus run core and text tests without Tauri. Test the stated WebKitGTK baseline explicitly and document the Linux support floor. Browser-only tests are useful but do not establish installed-webview behavior.

Use actual Tauri application tests where practical, followed by manual IME acceptance. Current Tauri documentation describes WebdriverIO’s Tauri service across all three desktop platforms, including an embedded server for macOS; direct `tauri-driver` remains limited to Windows/Linux. Testing package versions are **check**. [Tauri WebDriver guidance](https://v2.tauri.app/develop/tests/webdriver/)

Release Windows installers with Authenticode signing and timestamping, and macOS applications with Developer ID signing and notarization. Use signed Tauri updater artifacts and an HTTPS update feed; the updater signature is separate from OS code signing. Updater plugin versions are **check**. Apply updates only after drafts are durable and active editing can close cleanly. [Tauri updater](https://v2.tauri.app/plugin/updater/)

Keep OCR models independently versioned. Mobile releases use their platform distribution mechanisms. Test interrupted updates and database migration recovery before enabling automatic updates.

**9. Backend constraints and missed capabilities.** Preserve the report’s uncertainty boundaries.

| Contract area | Verdict | Deciding risk |
|---|---|---|
| Public Express hierarchy API | Read adapter after fixtures | Response shapes are unconfirmed |
| Firestore editing | Implement the full workflow | Partial saves omit platform effects |
| Notes and annotations | Preserve; defer unfamiliar mutations | Their object schemas are incomplete |
| Platform-wide live synchronization | Conditional | No complete change/deletion feed is evidenced |
| Corpus glyph extraction | Build additional local metadata | Shared character-box data is not established |

Several details materially affect the architecture.

The five transcription states must survive the data layer. `default` maps provisionally to 未着手, `initiated` to 翻刻中, and `completed` to 完了. Treat `editing` as an ownership state layered over `prevStatus`; retain `frozen` as a distinct read-only state until its workflow is understood. A failed fetch must not appear as an unstarted page. Prefer backend aggregates for project totals and label locally computed partial counts.

Phase-1 browsing should expose project guidelines and preserve `display`, project type, membership, blocked-user information, and feature flags. Public transcription reads do not establish public access to every related collection. Account-scoped caches need separation and appropriate invalidation.

The existing 集字-adjacent feature is especially valuable: `imageCollectionEntries` already stores IIIF crops, readings, tags, comments, privacy, and source coordinates. Reuse that contract for user-curated examples. CODH character pages are linked references in the report, not an evidenced API. Likewise, the confirmed `ocr.ndl` and `ocr.minna` fields contain text; the report does not establish a corpus-wide OCR-box collection. Locally generated line boxes can produce line examples, while exact character crops require additional alignment or detection.

Other existing capabilities worth planning for include transcription history and activity, two-reviewer approval, shared live drafts, proofreading/translations with separate locks and history, exports, notifications, project announcements, forums, likes, and image bookmarks. These deserve service boundaries or links back to the site; they need not all enter phase 1.

Points and levels should reflect acknowledged platform state. The report does not establish the accrual rule for stones or persisted translation points. Streaks and celebrations can be local additions, with configurable sound and reduced-motion behavior.

Callable functions are accessible from Rust through their documented JSON `data` envelope, authentication headers, and response/error protocol. There is no client-side “Firebase-signed body” step to invent. App Check enforcement remains a contract check. [Callable protocol](https://firebase.google.com/docs/functions/callable-reference)

A proposed directory layout is:

```text
honkoku-client/
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  package.json
  bun.lock

  apps/
    client/
      src/                  Svelte routes and application composition
      src-tauri/            Commands, native integrations, capabilities
    cli/                    Headless commands using the Rust core

  crates/
    core/                   Domain operations, auth, remote adapters, sync
    storage/                SQLite, migrations, drafts, pending operations
    iiif/                   Manifest normalization, fetching, image cache
    text/                   Lossless grammar, normalization, source mappings
    search/                 Tantivy schema, queries, corpus synchronization
    ocr/                    Job protocol and interchangeable engine adapters

  packages/
    ui/                     Tokens, Japanese UI components, keyboard commands
    editor/                 ProseMirror integration and facsimile alignment
    text-wasm/              Generated bindings and renderer adapter
    client-api/             Generated transport types and Tauri adapter

  sidecars/
    ocr-python/             Pinned honkoku-ocr-py packaging

  fixtures/
    api/                    Sanitized contract examples
    iiif/                   Presentation and Image API cases
    markup/                 Lossless text and editing cases

  tools/
    session/                Development login and contract investigation
    corpus/                 Import, reconciliation, benchmark tooling

  docs/
    decisions/
  .github/
    workflows/
```

Add phase-2 and phase-3 packages when their work begins. The phase-1 task list, ordered by dependency, is:

1. **Establish contract fixtures.** Confirm hierarchy response shapes, pagination, page indexing, canvas mapping, status interpretation, and required read permissions. Record unknowns without inventing defaults.
2. **Resolve authentication feasibility.** Prove Google and X callbacks or an explicit companion handoff. Establish whether phase 1 can deliver fresh-install login under the fixed backend constraint.
3. **Run the two architectural spikes.** Test vertical IME editing and the cached IIIF protocol in actual target webviews before building their surrounding UI.
4. **Create the workspaces and CI baseline.** Pin supplied versions, separate core/platform dependencies, establish generated transport types, and build a minimal application on all desktop targets.
5. **Implement session storage and refresh.** Cover restart, revocation, account switching, and first-user initialization.
6. **Implement the read adapters and SQLite cache.** Add scoped persistence, request deduplication, cancellation, freshness, and offline state.
7. **Normalize IIIF manifests and page identity.** Establish one authoritative page selection shared by transcription text, image, status, and navigation.
8. **Build the image cache and viewer.** Add thumbnails, bounded prefetch, resource limits, and accurate offline coverage.
9. **Build the Japanese library and read-only workbench.** Add keyboard navigation, themes, progress, guidelines, and restored location. Reject stale asynchronous results so rapid page changes cannot mix text and images.
10. **Verify the complete milestone and release path.** Test sign-in through restart, cached offline reading, missing resources, rapid navigation, long labels, rare characters, and real-webview performance. Exercise signing, installation, updater verification, and migration recovery. Phase 1 is complete when this reading workflow passes on Linux, Windows, and macOS.