# UI tokens

Primitive `--color-*` values define the palette. Components use semantic roles:

| Role | Use |
| --- | --- |
| `canvas` | Application background |
| `surface` | Panes and content panels |
| `surface-raised` | Controls and elevated panels |
| `text`, `text-muted` | Primary and secondary text |
| `border`, `border-strong` | Separators and control outlines |
| `accent`, `accent-text` | Active controls and their foreground |
| `focus` | Keyboard focus ring |
| `selection` | Selected rows and text |
| `status-default` | 未着手 |
| `status-initiated` | 翻刻中 |
| `status-editing` | 編集中 |
| `status-completed` | 完了 |
| `status-frozen` | 凍結 |

`:root` defines light colors; `:root[data-theme="dark"]` defines dark colors. An unset theme follows `prefers-color-scheme`; an explicit light choice overrides it. The client applies its saved choice in `theme.ts` before revealing the first frame. Every status also has a text label. Manuscript images retain their original pixels in both themes.
