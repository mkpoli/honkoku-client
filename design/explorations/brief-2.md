# Design exploration, round two: desktop client for みんなで翻刻

Read `brief.md` first for the product, the data, and the hard constraints; they all still apply. This round refines the direction after review of the first four explorations (`a/`–`d/`). Use your image generation tool, save PNGs under the output directory named in your instructions, and write `notes.md` there with the colour tokens (light and dark, hex), typefaces, radius and spacing. Do not touch any other file.

## What the review said

- Direction D read as corporate. Direction B read as dull. Direction A was calm and simple. Direction C was likeable, and its colours resemble the platform's landing page at honkoku.org: a warm, faintly textured light grey ground, white cards, near-black text, a persimmon accent (#D95936), sumi-ink illustrations, Noto Sans JP. Reference images of that landing page and of the liked explorations are attached; they are taste references, never templates to copy.
- None of the four dark themes convinced. A's lamp-lit paper came closest. The dark theme needs its own idea this round, and it must look good on the browsing screens, not only in the workbench.
- The general ideas of the workbench were right: facsimile beside a vertical editor, mark palette, OCR candidates, notes, page strip, save state.
- Gamification stays out of the editor. It appears on the home screen (ranking, own level and points) and at the moment a page is completed.

## The screens this round

### 01-home-light — the home dashboard

The home works like a developer's dashboard: three columns.

- Left: the project list with grouping and filtering that the current site lacks. Filter chips 公式 / ユーザー / 非公開 / 参加中; a keyword field; sort 更新順 / 進捗順 / 文字数順; group by 主催 (organizer) or by キーワード tag, with collapsible groups; each row shows the project's own progress (翻刻済みコマ / 総コマ, 字数) as a thin bar with figures. Real project rows are in `brief.md`.
- Centre: the activity timeline. Each event: who, what, where, when, and a short excerpt of the transcribed text, with a small thumbnail of the page. A control switches すべて / 参加中 / このプロジェクト. Sample events:
  - 山田花子 が 蝦夷語箋（国立国語研究所） コマ17 で 177字 を翻刻 · 約2時間前 · 「蝦夷語箋　〇天地之部 天（てん）《割書：リキク｜りきく》 地（ち）《割書：シリカ｜しりか》…」
  - 佐藤太郎 が 藻汐草 乾巻（国立国語研究所） コマ12 を 完了 · 3時間前
  - 鈴木文 が 蝦夷紀行（龍谷大学図書館） コマ4 で 312字 を翻刻 · 昨日 · 「しと夫ゟ酒杯あたへ色〳〵の耳言恵教を以て其心を慰めけれは漸に解心して…」
- Right: ranking (top contributors with points and level, tabs 今日 / 今週 / 累計), the signed-in user's own card (level, points, streak), and the platform's announcements. Ranking rows use invented names (山田花子, 佐藤太郎, 鈴木文, 高橋一郎, 田中美咲, 伊藤健) with points like 4,655,904 pt · Lv.524 down to 1,384,886 pt · Lv.197.

### 02-project-light — drilling into a project

Clicking a project pushes its detail view in place of the list, with a breadcrumb (ホーム › アイヌ関連資料 › 蝦夷方言藻汐草) that returns to any level. Header: title, organizer, project progress at every level (549/5573コマ, 161,892字, 67コレクション, 79資料), tabs 概要 / コレクション / タイムライン / お知らせ / メンバー / 凡例. Body: collection list with per-collection progress, and, for the selected collection 蝦夷方言藻汐草, its entries with thumbnails and per-entry counts 未着手 / 翻刻中 / 完了 and last editor. The right column now shows the project's own timeline and its top contributors, so the dashboard structure persists while the content narrows. Design the transition as a smooth push of panes; render one still of the arrived state.

### 03-workbench-dark — the transcription workbench

- The facsimile sits on the right and the vertical editor on the left, so reading continues from the original's right-to-left columns into the transcription. A small control swaps the sides.
- A 見開き spread is split into 右丁 and 左丁: the facsimile shows the split line and which half is active, and the editor's 【右丁】/【左丁】 divider matches.
- The page strip at the bottom behaves like a video editor's timeline: a thin strip of small slides with status colour, one per page; on hover it expands to show thumbnails of every page with numbers. Render the expanded state.
- Keep the mark palette, OCR candidates with per-character confidence, notes, and save state. No points, level, or streak on this screen.

### 04-home-dark — 01 in the dark theme

The same dashboard in dark, to judge the palette on a text-heavy browsing screen. Thumbnails and facsimiles keep their original colours.

## Variants

Each run produces one variant. The light theme in all three follows the landing page's warmth: warm light grey ground, white surfaces, near-black text, persimmon accent, 1px borders, radius 6–8px, no elevation shadows.

- **E 墨**: dark theme on warm near-black (#1B1816) with warm grey panels, paper-white text, persimmon kept as the accent, a second muted accent in indigo for links and selection. Noto Serif JP for document titles and the transcription, Noto Sans JP for everything else.
- **F 藍**: dark theme on deep indigo (#131A29) with slate panels, persimmon and a restrained gold for accents. Noto Sans JP throughout; serif only inside the transcription.
- **G 灰**: dark theme on neutral graphite (#171717 / #222) with persimmon as the single accent and imagery doing the colour work; the density of a developer dashboard, tabular numerals, compact rows. Noto Sans JP throughout; serif only inside the transcription.

## Deliverables (per variant)

1. `01-home-light.png`
2. `02-project-light.png`
3. `03-workbench-dark.png`
4. `04-home-dark.png`
5. `notes.md`
