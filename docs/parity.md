# Feature parity with app.honkoku.org

The site's user-facing features, taken from the 898 Japanese UI strings in its bundle (`index-CU1rQ6xi.js`, string table `R4e`) and the behaviour documented in `decisions/api-report.md`, compared with what the client offers. Administration screens (`admin.*`, 41 to 12 strings per screen for user search, project info, entries, members, announcements, join requests, blocked users, functions, progress, dashboard, guidelines) and project creation are outside the client's scope for now.

Status: ○ present, △ partial, × absent.

## Transcription screen

| Feature | Site | Client | Direction |
|---|---|---|---|
| Start, finish, discard editing with the server lock | ○ | ○ | Editing survives page changes; no 編集を再開 step. |
| Realtime sharing mode at start (`syncMode`) | ○ | △ shown, not selectable | Offer it as a toggle on 編集開始. |
| Save options: 翻刻完了, タイムラインで共有, コメント | ○ | ○ | Inline popover instead of a dialog. |
| Save option 添削希望 (`requestReview`) | ○ | × | Add to the popover; show 添削希望 on timeline items. |
| Two-reviewer approval (`isApproval`, `approvedBy`, at most two) | ○ | × | Show approval state on the page and offer the check in the popover when the project uses it. |
| Tabs 画像 / 閲覧 / 入力 | ○ | ○ | Both panes visible at once; facsimile side selectable. |
| OCR tab: NDL, みんなで翻刻OCR, copy result, saved result | ○ | ○ plus local GPU OCR | |
| 編集履歴 tab: past saves from `timelineEvents`, diff dialog | ○ | × | Side panel with inline character diff and restore into the editor. |
| 注釈 tab: notes of type 注釈 / メモ / 翻刻 with optional image region, create, edit, delete, kept in `tempNotes` while editing | ○ | × (the client's 注記 are inline 【】 marks) | Notes panel, overlays on the facsimile, region capture with the same rectangle gesture as clips. |
| 翻訳 tab (crowd translation module) | ○ | × | Later, with the proofreading module. |
| Viewer modes: normal, 画像を保存 (clip), 注釈・翻刻を追加 (region note), AI recognition (range select, Metom or in-browser model) | ○ | △ clip only | Add 認識 and 注釈 modes; candidates appear beside the facsimile with keyboard picks, no dialog. |
| Toggle annotation overlays | ○ | × | With the notes panel. |
| Full-size image download | ○ | × | Native save dialog through the IIIF fetcher. |
| Special-mark palette: 振り仮名, 踊り字, 縦横切替 | ○ | ○ plus 合字, 変体仮名, 欠字, 返り点, 送り仮名, 常用字, 常用句, 記号, 集字 | |
| Input templates 【注記】 and 《場所：》 | ○ | ○ / △ | 場所 is parsed and rendered; add it to the palette. |
| Drawer: navigation, search, project top, collection top, timeline, forum, home | ○ | ○ breadcrumb and search | Forum pending. |
| Drawer: 特殊記法の解説 | ○ | × | In-app reference generated from the markup package. |
| Drawer: 翻刻ガイドライン (standard and project-specific) | ○ | × | Project screen and workbench link. |
| Drawer: 翻刻文のダウンロード (text, LaTeX, Word, XML templates; zip for many) | ○ | × | Page and entry export as text, TEI XML, and LaTeX. |
| Drawer: 書誌情報 | ○ | × | Manifest metadata panel. |
| Status labels (自分／他人が編集中, 未着手, 着手済み, 翻刻完了) | ○ | ○ | |
| Tutorial bubbles | ○ | × | Not planned. |

## Home and projects

| Feature | Site | Client | Direction |
|---|---|---|---|
| Project list with 公式 / ユーザー / 非公開 / 参加中, search | ○ | ○ plus grouping and sort | |
| Timeline: すべて / 参加中 | ○ | ○ | |
| Timeline filter 添削希望のみ | ○ | × | Add with the 添削希望 save option. |
| Ranking by ポイント / 文字数 / いいね | ○ | ○ plus own row and 自分の順位 | |
| Profile panel: level, points, 文字数, いいね, 石 | ○ | △ no 石 | Add 石. |
| Project progress totals (総入力文字数, 翻刻完了画像, 翻刻完了史料, ユーザー数) | ○ | × | Small panel on the home dashboard. |
| Announcements (site and project) | ○ | △ site only | Project announcements page. |
| Project join, leave, join request | ○ | × | Project screen actions. |
| Project guidelines page | ○ | × | Project screen. |
| Project forum | ○ | × | See social. |
| Create project, admin menu | ○ | × | Out of scope. |

## Image collection

| Feature | Site | Client | Direction |
|---|---|---|---|
| Own clips: list, search by reading, comment, tags | ○ | ○ | |
| Clip edit (reading, comment, tags, private), copy IIIF URI | ○ | × | Inline edit on the card. |
| Other users' public clips, per-user collection page, recent tags | ○ | × | Include-others toggle and a user page. |
| Clip detail page | ○ | × | Card expands in place. |

## Social and account

| Feature | Site | Client | Direction |
|---|---|---|---|
| Forum (global and per project): posts with images, comments, edit, delete | ○ | × | Forum screens with the same Firestore collections. |
| Notifications list and read state | ○ | △ unread count | Notification panel. |
| Direct messages | ○ | × | Messages screen. |
| Likes on timeline events and posts | ○ | × | Like buttons. |
| User profile page: stats, send message, public clips | ○ | × | Profile screen. |
| Email settings, data migration from honkoku2 | ○ | × | Link to the site. |
| Sign in with Google or X | ○ | △ X only (Google blocks embedded webviews) | Needs a desktop OAuth client from the platform owner. |

## Other modules

| Feature | Site | Client | Direction |
|---|---|---|---|
| 現代語訳の校正 (proofreading of generated translations: line grid, status, diff, history, TSV) | ○ | × | Later. |
| Site search (Cloud Run) | ○ | ○ local concordance with KWIC | The client's index answers offline. |
| Wiki, まなぶ, ご案内 links | ○ | × | External links in the menu. |
| Gamification: level and points, completion effects | ○ | ○ with chime and celebration | |
| 集字 | × | ○ | |
