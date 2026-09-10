# Design system

The client's visual system combines the layout of exploration E (`explorations/e`), the light palette of exploration F (`explorations/f`), and the dark palette of E. Facsimile pixels keep their original colours in both themes.

## Colour

| Role | Light | Dark | Use |
| --- | --- | --- | --- |
| canvas | #F4F2EE | #1B1816 | window ground |
| surface | #FFFFFF | #272320 | panels, cards, rows |
| surface-inset | #F7F5F1 | #302B27 | inputs, quoted excerpts, filmstrip |
| text | #202632 | #F1EAE0 | primary text |
| text-muted | #68717D | #BDB1A5 | secondary text, captions |
| border | #DDDCD6 | #494039 | dividers, panel edges |
| border-strong | #B9B7B0 | #6A6058 | input edges, focused borders |
| accent | #D95936 | #D95936 | primary actions, active tab, work in progress |
| accent-text | #FFFFFF | #FFFFFF | text on accent |
| accent-soft | #FBEDE7 | #3A2A24 | selected row, active chip ground |
| link | #596785 | #8F9FBD | links, selection colour in text |
| selection | #EDF0F5 | #303643 | selected text ground, active column in the editor |
| gold | #AE8842 | #C1A16A | completed pages, leading contributors |
| success | #3E9150 | #7FBF8A | connected, saved |
| status-default | #9A9790 | #7C756D | 未着手 |
| status-initiated | #D95936 | #D95936 | 翻刻中 |
| status-editing | #596785 | #8F9FBD | 編集中 (someone holds the lock) |
| status-completed | #AE8842 | #C1A16A | 完了 |
| status-frozen | #765496 | #C6A4E8 | 凍結 |
| focus | #D95936 | #ED8968 | keyboard focus ring |
| track | #E3E5E7 | #3A3430 | progress bar track |

Every status has a label and a distinct symbol beside its colour.

## Typography

| Role | Family | Size / line height | Weight |
| --- | --- | --- | --- |
| Screen title | Noto Sans JP | 28 / 36 px | 700 |
| Section heading | Noto Sans JP | 20 / 28 px | 700 |
| Document title | Noto Serif JP | 18 / 26 px | 600 |
| Controls, metadata | Noto Sans JP | 15 / 22 px | 400–500 |
| Captions | Noto Sans JP | 13 / 18 px | 400 |
| Counts, identifiers | Noto Sans JP, tabular numerals | 15 / 22 px | 500 |
| Transcription (vertical) | Noto Serif JP | 26 / 44 px | 400 |
| Furigana, 割書 | Noto Serif JP | half the transcription size | 400 |

Japanese text takes no space before or after Latin letters and digits. Numerals use `font-variant-numeric: tabular-nums`. Fallbacks: "Noto Sans JP", "Hiragino Sans", "Yu Gothic", sans-serif; "Noto Serif JP", "Hiragino Mincho ProN", "Yu Mincho", serif. Rare kana (ゟ, ヿ, 𬼂) and variation sequences must render; a fallback that covers the Kana Supplement block is required.

## Shape and spacing

Panels and inputs have 7 px corners; small controls 6 px. Borders are 1 px. Spacing steps are 4, 8, 12, 16, 24, 32 px; pane gaps are 16 px; panel padding is 16 px. No elevation shadows; hierarchy comes from surface and border tokens. The light canvas carries a faint paper grain outside panels.

## Layout

- Home: three columns, 36 / 40 / 24 percent. Left: project list with filter chips (公式 / ユーザー / 非公開 / 参加中), sort (更新順 / 進捗順 / 文字数順), grouping (主催 / キーワード) with collapsible groups, and per-project progress. Centre: activity timeline. Right: ranking, own record, announcements.
- Project: the left and centre columns are replaced by the project header (title, organizer, progress at project level, tabs) and the collections and entries panes; the right column shows the project's own timeline and contributors. Entering a project pushes the panes leftward; the breadcrumb returns to any level.
- Workbench: editor left, facsimile right, swap control in the toolbar. A 見開き spread shows its 左丁 / 右丁 halves as selectable regions; the editor's page-half divider mirrors them. The filmstrip at the bottom is a thin status strip that expands on hover into thumbnails with numbers and status. Points, level, and streak never appear here.

## Motion

Pane pushes and breadcrumb returns slide 200 ms with an ease-out curve; the filmstrip expands in 150 ms. `prefers-reduced-motion` disables the slides and keeps fades under 100 ms.
