# みんなで翻刻クライアント

A read-only desktop and CLI client for the public みんなで翻刻 library.

Rust owns the Express and Firestore adapters and SQLite caching. The Svelte SPA calls six typed Tauri commands. The desktop reader has three panes: projects and collections, entries and pages, and the selected transcription and image.

## Development

Requires Rust 1.96.0, Bun 1.3.14, and Tauri 2 Linux prerequisites when building on Linux.

```sh
bun install
bun run --cwd apps/client check
bun run --cwd apps/client build
cargo test --workspace
cargo build -p honkoku-client
cargo run -p honkoku-cli -- projects
cargo run -p honkoku-cli -- pages 0916dafb80cdc48ca7687afcad4a4f35 --status
```

The CLI serves cached responses for ten minutes. Add `--refresh` before or after a subcommand to request fresh data. Its database is `$XDG_CACHE_HOME/honkoku-client/cache.sqlite`, with the platform cache directory as the fallback. The desktop uses Tauri's application cache directory. The current cache is for anonymous public reads; authenticated persistence is outside this reader's scope.

Use `devrun bun run --cwd apps/client dev` for the frontend development server. Remote data requires the Tauri shell; a plain browser has no invoke transport.

## Workspace

- `crates/core`: models, HTTP and Firestore adapters, async cache policy.
- `crates/storage`: synchronous SQLite storage and numbered migrations.
- `crates/iiif`: Presentation 2/3 normalization and Image API 1/2/3 URLs.
- `crates/text`: plain-text excerpts from platform markup.
- `apps/cli`: the `honkoku` command.
- `apps/client`: Svelte frontend and `src-tauri` native shell.
- `packages/ui`: semantic light/dark tokens.
- `packages/client-api`: hand-written wire types and invoke functions.
- `fixtures/api`: captured public API responses used by offline tests.

The reader preserves unfamiliar document fields and status strings. Missing and null metadata remain optional. Express timestamps and Firestore timestamps normalize to RFC3339 strings with nanosecond precision. Firestore server revision metadata is retained under `_firestore`.

Entry transcriptions embedded by Express and pages fetched from Firestore have independent freshness records. List and detail snapshots are cached separately because their fields differ. Successful empty responses are cached; failed requests are reported.
