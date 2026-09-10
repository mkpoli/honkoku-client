# みんなで翻刻クライアント

A desktop client for みんなで翻刻 (app.honkoku.org), the crowdsourced transcription platform for pre-modern Japanese documents, built with Tauri 2 and Svelte 5 on a Rust core. It signs in with a platform account and reads the same Firestore data as the website.

Phase 1 covers reading: a home dashboard (projects with grouping and filters, activity timeline, ranking, announcements), project and collection drill-down with progress at every level, an entry page grid, and a workbench that shows the facsimile through a local IIIF cache beside the transcription rendered vertically with its markup. Editing, OCR, and concordance search follow; the plan is in `docs/decisions/0001-architecture.md`, the backend contract in `docs/decisions/api-report.md` and `api-capture-notes.md`, and the visual system in `design/system.md`.

## Development

Requires Rust 1.96.0, Bun 1.3.14, and the Tauri 2 Linux prerequisites when building on Linux.

```sh
bun install
cargo test --workspace
bun test
bun run --cwd apps/client check
devrun bun run --cwd apps/client tauri dev     # the desktop application
devrun bun run --cwd apps/client dev           # the same screens in a browser, on fixture data
bun tools/shots/shoot.ts                       # browser screenshots of every screen, light and dark
tools/shots/tauri.sh out.png ['#/route']       # a screenshot of the real WebKitGTK window on a virtual display
```

Signing in: run `bun tools/session/login.ts`, sign in to the website in the window it opens, and press ログイン in the client (or `honkoku login --import` for the CLI). `tools/session/README.md` has the details.

The CLI (`cargo run -p honkoku-cli -- …`) offers `projects`, `project`, `collection`, `entry`, `pages`, `whoami`, `notifications`, `timeline`, `ranking`, and `announcements`, each with `--json`; cached responses are reused for ten minutes unless `--refresh` is given.

## Workspace

- `crates/core`: models, the Express and Firestore adapters, session and token refresh, home-screen queries, cache policy.
- `crates/storage`: SQLite storage with numbered migrations.
- `crates/iiif`: IIIF Presentation 2/3 normalisation, Image API 1/2/3 URLs, the fetcher and disk cache.
- `crates/text`: plain-text excerpts from the platform's markup.
- `apps/cli`: the `honkoku` command.
- `apps/client`: the Svelte application and its `src-tauri` shell, which serves cached images over the `honkoku-iiif` scheme.
- `packages/ui`: design tokens for light and dark themes.
- `packages/client-api`: wire types and typed command wrappers, with a fixture transport for browser development.
- `packages/markup`: the renderer for transcription markup (ruby, 割書, 見せ消ち, page halves, gaps, notes).
- `fixtures/`: captured public API responses used by tests and by browser mode.
- `tools/`: session capture and screenshot harnesses.

Documents keep unfamiliar fields and status strings; Express and Firestore timestamps normalise to RFC 3339 with nanosecond precision.
