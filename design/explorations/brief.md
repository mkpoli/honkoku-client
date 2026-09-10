# Design exploration brief: desktop client for みんなで翻刻

You are exploring visual directions for a new desktop application. Use your image generation tool to render mockups and save every PNG under the output directory named in your instructions. Write a short `notes.md` there with the colour tokens (hex, light and dark), typefaces, and radius/spacing values you used. Do not touch any other file in this repository.

## The product

みんなで翻刻 (Minna de Honkoku) is a Japanese crowdsourced transcription platform. Volunteers read くずし字 (cursive pre-modern Japanese) in scanned manuscripts and type the text. This application is a native desktop client (Windows, macOS, Linux) that talks to the same backend. Its users are volunteers, historians, philologists, and linguists who transcribe for hours at a time.

Two screens are being explored.

### Screen 1 — Library

Browsing goes project → collection → entry (a physical book or document) → page. The library screen shows the list of projects with their progress, and beside it the collections and entries of the selected project. Real data to use:

Projects (title, transcribed images / total images, characters transcribed):

- アイヌ関連資料 — 549/5573 — 161,892字
- 翻刻！べらぼう関連資料 — 1854/6418 — 698,840字
- 奥州市所蔵資料翻刻プロジェクト — 608/5581 — 111,109字
- 地誌・紀行・寺社縁起 — 168/12024 — 65,911字
- 様々な塵劫記 — 125/5553 — 58,816字
- 和算拾遺 — 159/5396 — 52,644字
- 国宝　医心方 — 18/1590 — 13,940字
- 刀剣書を翻刻しよう — 4/111 — 7,993字

Collections inside アイヌ関連資料: 蝦夷方言藻汐草 (3 entries), 蝦夷語箋 (1 entry), 蝦夷島奇観 (2 entries), 蝦夷草紙 (4 entries), 北海随筆 (1 entry).

Entries inside 蝦夷方言藻汐草: 藻汐草 乾巻（国立国語研究所） 68コマ, 藻汐草 坤巻（国立国語研究所） 72コマ, 藻汐草（函館市中央図書館） 131コマ. Each entry shows a thumbnail of its first page, page count, how many pages are 未着手 / 翻刻中 / 完了, and the last editor's display name and time.

### Screen 2 — Workbench (dark theme)

The transcription screen. Left: the facsimile of one page (a scanned Edo-period manuscript, vertical columns of brushed kana and kanji on aged paper) with zoom and rotate controls, and a translucent highlight on the column currently being edited. Right: a vertical-writing text editor (縦書き, columns run right to left) showing the transcription with its markup rendered as what it means rather than as raw symbols: furigana small beside the base text, 割書 as two half-size lines, 見せ消ち with a strike, 【右丁】/【左丁】 as a column divider, ■ as a blank glyph box. Below or beside the editor: a palette for inserting special marks (踊り字 ゝゞ〻〳〵, 合字 ゟ〆ヿ, 変体仮名, 返り点 レ一二三, 割書, 振り仮名, 注記), OCR suggestions for the current column with per-character confidence, a page strip (thumbnails of neighbouring pages with status colour), and the notes panel. A compact status area shows the logged-in user's points, level, and today's streak, and a save state. Sample transcription text for the editor (from 藻汐草):

峯　シリキタイ
高　リイ
浜ゟ出し風　ケシカクベシ
泊ゟ出し風　トマクベシ
峠　ルチシ
岡の道　マクベカ
燃る　アベセベク
天河　ベツノカ

## Hard constraints

- Not Material Design. No floating action buttons, no elevated card shadows, no ripple, no Roboto, no app bar in primary blue. The current site looks like a stock Material template; every direction must look unlike it.
- Japanese is the primary UI language. Labels are Japanese (プロジェクト, コレクション, 資料, コマ, 翻刻中, 完了, 保存済み, 注記, 変体仮名 …). Real density: real rows, real numbers, readable labels, no lorem ipsum, no garbled pseudo-Japanese. If a glyph will not render correctly, simplify the text rather than invent characters.
- Vertical Japanese text in the editor must be legible and run right to left.
- The facsimile keeps its original colours; the dark theme darkens the interface around it, never the scan.
- Both screens are desktop, 1600×1000 landscape. Native-app framing (a thin custom title bar is fine), no browser chrome.
- No slogans, taglines, or marketing copy. No mascot. No invented logos: the wordmark is the plain text みんなで翻刻.
- Aim for something a real team could build: real UI density, consistent spacing, one type system.

## Deliverables (per direction)

1. `01-library-light.png` — Screen 1, light theme.
2. `02-workbench-dark.png` — Screen 2, dark theme.
3. `notes.md` — tokens and typefaces, plain prose, no commentary about this brief.
