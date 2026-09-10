# 0002 — Vertical editor

Status: conditional ProseMirror recommendation, 2026-09-10.

ProseMirror supports the tested vertical editing model without changes to its internals. Retain it for the next webview evaluation. Production readiness remains unproven because native Japanese IME candidate placement and reconversion have not been exercised in WebView2, WKWebView, or WebKitGTK.

## Implementation and evidence

`packages/markup` provides a concrete syntax tree with source spans, original construct spelling, and separate line-ending leaves. `serialize(parse(source))` preserves every UTF-16 code unit. Tests cover 3,228 captured text-field occurrences across all 11 matching fixture files, malformed and nested marks, empty segments, variation selectors, unpaired surrogates, and 2,000 deterministic generated strings. The same captured text fields pass `toMarkup(fromMarkup(source))`.

`packages/editor` projects each source line into a ProseMirror `column`. Ruby has two or three editable fields; warigaki has two to four; corrections and kenten have separate editable contents. Unknown syntax remains editable raw text. Source-bearing leaf nodes retain page dividers, gaps, references, comments, and editorial annotations; decorations display them. Return marks, okurigana, titles, boxes, places, and right-side lines retain their source notation.

Transactions produce patches whose offsets refer to the transaction's preceding source, measured in UTF-16 code units. Unchanged persistent columns anchor separate patches. Edits preserve other columns, reference numeral widths, legacy ruby spelling, and mixed CRLF/CR/LF endings. Source replacement and visual commands share ProseMirror history; textarea input maps its normalized line breaks back to the source. Toolbar and keyboard undo/redo use that history in either view.

The legacy `base（right｜left）` acceptance rules use the captured site's `KojiLexer.g4` character classes and `furiganaTarget`/`furigana` rules, extending the examples in [the API report](api-report.md#8-editormarkup-behaviour). Accepted bases include complete Kanji, Kana, or NonJp tokens, annotation nodes, and runs of ■ or □, with optional ／. Readings must be nonempty; malformed forms remain raw. `ひらカナ（x）` annotates only `カナ`. The captured lexer also accepts `abc（ruby）` and `説明（日本語）`; parenthetical prose cannot be distinguished by a kana-only heuristic.

The development-only route is `#/spike/editor`. It selects the most annotated captured 蝦夷語箋/藻汐草 page by recognized construct count: 蝦夷方言藻汐草・坤巻（国立国語研究所）, page 82, with 38 constructs in nine source columns. Its raw pane is maintained by source patches; 一致 compares that source against independent document serialization. The production build excludes the route implementation, ProseMirror view, fixture loader, and editor font.

## Input and layout results

The Chromium harness passes ordinary keyboard typing; grapheme-aware ↑↓; logical-column ←→; Home/End; Enter; ruby insertion through the reading popover; editing every ruby and warigaki field; misekechi insertion; gap, note, and character-palette insertion; raw-mode switching; mixed-ending raw edits; and undo/redo. Navigating all 20 columns of a wide page keeps the caret inside the scrolling editor pane.

CDP `Input.imeSetComposition` followed by `Input.insertText` produces `前日本後` from `前後`. Intermediate `にほん` becomes `日本` once, without duplication. Cancellation restores the precomposition source. Composition over selected `日本` commits `日本国`. Composition inside ruby readings and warigaki fields also survives serialization and undo. Synthetic compositionstart/update/end events with DOM text mutation preserve the composing column element and suppress cross-column movement. Decorations map through composing transactions; source replacement and toolbar commands defer or refuse while the visual editor is composing.

These observations are from headless Chromium 153.0.8010.12 on WSL2. No native IME candidate window was available in this test environment. Microsoft IME, macOS Japanese input, and Linux IME behavior in the actual Tauri webviews remain unmeasured.

Light and dark screenshots were inspected against `design/explorations/e/03-workbench-dark.png`. The editor retains the workbench's surfaces, warm dark palette, 26 px transcription, half-size annotations, hairline page divisions, and visible focus/selection. Noto Serif Hentaigana is bundled under its [SIL Open Font License](../../packages/editor/fonts/OFL.txt) for the supplementary kana palette and source display.

## Measurements

Environment: AMD Ryzen 7 9800X3D, Linux 6.18.33.2-microsoft-standard-WSL2, Bun 1.3.14, Playwright 1.63.0, Chromium 153.0.8010.12, headless, 1600 × 1000 viewport, device scale 1.

The page contains 20 source columns and 730 UTF-16 code units, including ruby and warigaki in each column. After fonts load, 60 single-character `page.keyboard.type` calls insert text at the beginning of the first column.

| Measurement | Median | p95 | Maximum |
| --- | ---: | ---: | ---: |
| Transaction, source patches, view update, forced layout | 0.20 ms | 0.40 ms | 0.50 ms |
| Playwright key call through the next animation-frame callback | 16.64 ms | 17.20 ms | 19.62 ms |

The first measurement uses `performance.now()` around dispatch plus `getBoundingClientRect()`. The second includes browser automation transport and frame scheduling; it does not measure display presentation or native IME latency. The measurement summary is recorded in `.local/shots/06-editor-metrics.json`; `bun tools/shots/shoot.ts` regenerates the measurements.

## Reproducible limitations

- **Logical versus visual columns:** paste a source line long enough to wrap, then press ←. The caret moves to the next source line rather than the adjacent wrapped visual column. Cross-column movement preserves the ordinal caret position, so ruby fields can produce a different visual height. Screen-coordinate navigation still needs evaluation.
- **Enter inside annotations:** load `《振り仮名：峰｜みね》続き`, place the caret inside `峰`, and press Enter. The result is `《振り仮名：峰｜みね》\n続き`; the annotation is kept whole. Splitting its individual fields across source columns has no representation in this schema.
- **Reserved syntax in editable fields:** entering `｜` inside a ruby reading, then editing an unrelated character in raw mode, changes the number of segments when the source is reparsed. Source characters survive, but the projected structure can change. There is no established escaping syntax; raw mode exposes the literal text.
- **Selection wrapping:** ruby, warigaki, and correction commands accept plain text within one source column. A selection crossing columns or existing annotations disables those commands. More complex wrapping requires a grammar and interaction decision.

## Recommendation

| Candidate | Recommendation | Deciding risk |
| --- | --- | --- |
| ProseMirror | Continue with the same corpus in the three target webviews | Native composition, candidate geometry, and vertical selection around nested editable annotations |
| Lexical | Reserve as the second prototype if the webview tests require invasive ProseMirror changes | Its behavior on this corpus is unmeasured; changing engines does not establish vertical IME correctness |
| Custom contenteditable | Reserve for failure of both structured editors | The application would own DOM reconciliation, composition, selection mapping, and undo correctness |

No Lexical or custom-contenteditable benchmark was run. The deciding acceptance test is native IME composition, cancellation, reconversion, selection, candidate placement, and scrolling in each target webview with zero lost or duplicated source characters.

## Verification

All five required commands exited 0: `bun install`, `bun test`, `bun run --cwd apps/client check`, `bun run --cwd apps/client build`, and `bun tools/shots/shoot.ts`. The unit suite has 45 passing tests; Svelte reports zero errors and warnings. The production build reports a bundle-size advisory for the existing main application chunk. The screenshot harness passes the existing screens' interaction checks alongside the editor checks and stops its owned devrun scope.

Screenshots in `.local/shots/`:

- `01-home-{light,dark}.png`
- `02-project-{light,dark}.png`
- `03-collection-{light,dark}.png`
- `04-entry-{light,dark}.png`
- `05-workbench-{light,dark}.png`
- `06-editor-{light,dark}.png`
- `06-editor-annotations-{light,dark}.png`
