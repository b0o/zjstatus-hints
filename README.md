# zjstatus-hints

A [Zellij](https://github.com/zellij-org/zellij) plugin that displays context-aware key bindings for each Zellij mode. Extends the functionality of [zjstatus](https://github.com/dj95/zjstatus).

![2025-06-06_16-23-55_region](https://github.com/user-attachments/assets/cfb93423-f37c-410a-aca9-a49290312d0e)

https://github.com/user-attachments/assets/940a31a0-86de-469d-89e2-dab18a1aaca8

## Rationale

Zjstatus is an excellent plugin, but it lacks the ability to display keybinding hints for your current mode, as the built-in Zellij status-bar plugin allows. This plugin adds that functionality to zjstatus, so you can have the best of both worlds.

## Features

- Shows context-aware key bindings for each Zellij mode (Normal, Pane, Tab, Resize, Move, Scroll, Search, Session)
- Integrates seamlessly with zjstatus via named pipes

## Installation

First, install and configure [zjstatus](https://github.com/dj95/zjstatus). Then, add the zjstatus-hints plugin to your Zellij configuration:

```kdl
plugins {
    zjstatus-hints location="https://github.com/b0o/zjstatus-hints/releases/latest/download/zjstatus-hints.wasm" {
        // Maximum number of characters to display
        max_length 0 // 0 = unlimited
        // String to append when truncated
        overflow_str "..." // default
        // Name of the pipe for zjstatus integration
        pipe_name "zjstatus_hints" // default
        // Hide hints in base mode (a.k.a. default mode)
        // E.g. if you have set default_mode to "locked", then
        // you can hide hints in the locked mode by setting this to true
        hide_in_base_mode false // default

        // How modifier keys are displayed. Options:
        //   "long"   — "Ctrl + n", "Alt + h"  (default)
        //   "short"  — "C-n", "A-h"
        //   "symbol" — "^n", "M-h"
        modifier_style "long" // default

        // Template for each hint. Available variables: {key}, {action}
        // Default matches current behavior: key block followed by action block
        hint_format "{key} {action}" // default

        // String inserted between hints (default: no separator)
        separator "" // default

        // Limit alternative keybindings shown per action (0 = unlimited)
        max_keys "0" // default

        // Rename action labels in the display (spaces → underscores in key names)
        // alias_fullscreen "full"
        // alias_split_right "S→"
        // alias_select "sel"

        // Rename special bare keys in the display
        // key_alias_enter "⏎"
        // key_alias_space "␣"

        // Optional: override palette-derived colors with hex values (#RRGGBB)
        // key_fg   "#ffffff"
        // key_bg   "#6e5fb7"
        // label_fg "#cccccc"
        // label_bg "#4c435c"
    }
}

load_plugins {
    // Load at startup
    zjstatus-hints
}
```

Finally, configure zjstatus to display the hints in your default layout (`layouts/default.kdl`):

```kdl
layout {
    default_tab_template {
        children
        pane size=1 borderless=true {
            plugin location="zjstatus" {
                format_left   "{mode} {tabs}"

                // You can put `{pipe_zjstatus_hints}` inside of format_left, format_center, or format_right.
                // The pipe name should match the pipe_name configuration option from above, which is zjstatus_hints by default.
                // e.g. pipe_<pipe_name>
                format_right  "{pipe_zjstatus_hints}{datetime} " 

                // Note: this is necessary or else zjstatus won't render the pipe:
                pipe_zjstatus_hints_format "{output}"
            }
        }
    }
}
```

## Configuration

| Option | Default | Description |
|---|---|---|
| `max_length` | `0` | Maximum characters to display. `0` = unlimited. |
| `overflow_str` | `"..."` | String appended when output is truncated. |
| `pipe_name` | `"zjstatus_hints"` | Pipe name used for zjstatus integration. |
| `hide_in_base_mode` | `false` | Hide hints when in the base (default) mode. |
| `modifier_style` | `"long"` | How modifier keys are rendered. See below. |
| `hint_format` | `"{key} {action}"` | Template for each hint. Variables: `{key}`, `{action}`. |
| `separator` | `""` | String inserted between hints. |
| `max_keys` | `0` | Max alternative keybindings shown per action. `0` = unlimited. |
| `alias_{label}` | _(none)_ | Rename an action label in the display. See below. |
| `key_alias_{key}` | _(none)_ | Rename a special bare key in the display. See below. |
| `key_fg` | _(palette)_ | Global foreground color for key blocks. |
| `key_bg` | _(palette)_ | Global background color for key blocks. |
| `label_fg` | _(palette)_ | Global foreground color for action label blocks. |
| `label_bg` | _(palette)_ | Global background color for action label blocks. |

### `modifier_style`

Controls how modifier keys such as Ctrl and Alt appear in key hints:

| Value | Example output |
|---|---|
| `"long"` (default) | `Ctrl + n`, `Alt + h` |
| `"short"` | `C-n`, `A-h` |
| `"symbol"` | `^n`, `M-h` |

### `hint_format`

A template string applied to each individual hint. Two variables are available:

- `{key}` — the formatted keybinding (respects `modifier_style`)
- `{action}` — the action label (e.g. `new`, `close`, `fullscreen`)

Examples:

```kdl
hint_format "{key} {action}"   // default: " Ctrl + n  new "
hint_format "{key}:{action}"   // compact: "^n:new"
hint_format "{action}[{key}]"  // action-first: "new[^n]"
```

### `separator`

A string inserted between hint groups. Empty by default (hints are concatenated).

```kdl
separator " | "   // e.g. " Ctrl + n  new  |  Ctrl + w  close "
separator " · "
```

### `max_keys`

Limits how many alternative keybindings are shown for each action. Zellij merges
`shared_except` blocks into every mode, so an action like "move focus" can accumulate
8+ bound keys (`h|j|k|l|←|↓|↑|→`). Setting `max_keys` trims this to the first N entries.

```kdl
max_keys "4"   // show at most 4 alternatives per action
```

Keys are shown in the order Zellij reports them, which typically puts mode-specific
bindings before shared ones. `0` (the default) shows all keys.

### `alias_{label}` — action label aliases

Rename any action label in the display. Config keys use the pattern `alias_{label}`,
where spaces in the label are replaced with underscores. The alias affects only the
displayed text; color lookups still use the original label name.

```kdl
alias_split_right  "S→"
alias_split_down   "S↓"
alias_fullscreen   "full"
alias_rename       "ren"
alias_half_page    "½pg"
alias_increase     "inc"
alias_decrease     "dec"
alias_select       "sel"
alias_break_pane   "break"
```

### `key_alias_{key}` — bare key display aliases

Replace how a special bare key is displayed. Config keys use `key_alias_{name}`,
where `{name}` is the lowercased default representation of the key.

```kdl
key_alias_enter  "⏎"
key_alias_space  "␣"
key_alias_esc    "⎋"
key_alias_tab    "⇥"
```

### Compact config recipe

Combining all options for maximum space savings:

```kdl
zjstatus-hints location="..." {
    pipe_name "zjstatus_hints"
    modifier_style "symbol"
    hint_format " {key} {action} "
    separator "·"

    max_keys "2"

    alias_split_right "S→"
    alias_split_down  "S↓"
    alias_fullscreen  "full"
    alias_rename      "ren"
    alias_half_page   "½pg"
    alias_increase    "inc"
    alias_decrease    "dec"
    alias_select      "sel"
    alias_break_pane  "break"

    key_alias_enter "⏎"
    key_alias_space "␣"
}
```

This produces output similar to:

```
 n new · x close · f full · w float · e embed · r S→ · d S↓ · ←→ move · ⏎ sel
```

## Color customization

zjstatus-hints supports full color customization so you can match hint colors to your zjstatus theme.
All color values are hex strings (`"#RRGGBB"`).  If a color option is omitted the plugin falls back
to the Zellij palette (backward-compatible default behavior).

### Global color defaults

Override the palette-derived colors for every hint:

```kdl
zjstatus-hints location="..." {
    key_fg   "#ffffff"   // foreground of the key (keybinding) block
    key_bg   "#6e5fb7"   // background of the key block
    label_fg "#cccccc"   // foreground of the action label block
    label_bg "#4c435c"   // background of the action label block
}
```

### Per-label color overrides

Override colors for a specific action label across all modes.  Labels with spaces use underscores
in the config key (`split_right_key_bg` → label `"split right"`).

```kdl
zjstatus-hints location="..." {
    quit_key_bg    "#ff0000"
    quit_label_bg  "#cc0000"
    select_key_bg  "#00aa00"
    split_right_key_bg "#0066ff"
}
```

### Mode-specific overrides

Prefix the label key with `{mode}.` to target a single mode only:

```kdl
zjstatus-hints location="..." {
    pane.new_key_bg    "#00ffcc"
    tab.close_key_bg   "#ff6666"
    pane.split_right_key_fg "#ffffff"
}
```

### Lookup priority (per field, independently)

For each color field (`key_fg`, `key_bg`, `label_fg`, `label_bg`) the plugin resolves the value in
this order, stopping at the first match:

1. Mode-specific label override — `pane.new_key_bg`
2. Global label override — `new_key_bg`
3. Global default — `key_bg`
4. Zellij palette (built-in fallback)

Fields are merged independently: a mode-specific override can set `key_bg` while a global label
override provides `label_fg`.

### Full color config example

```kdl
zjstatus-hints location="..." {
    pipe_name "zjstatus_hints"

    // Global color defaults (override palette for all hints)
    key_fg   "#ffffff"
    key_bg   "#6e5fb7"
    label_fg "#cccccc"
    label_bg "#4c435c"

    // Per-label overrides (all modes)
    quit_key_bg   "#ff0000"
    quit_label_bg "#cc0000"
    select_key_bg "#00aa00"

    // Mode-specific overrides
    pane.new_key_bg  "#00ffcc"
    tab.close_key_bg "#ff6666"
}
```

### Color configuration reference

| Option pattern | Applies to | Slot |
|---|---|---|
| `key_fg` | all labels, all modes | key foreground |
| `key_bg` | all labels, all modes | key background |
| `label_fg` | all labels, all modes | action label foreground |
| `label_bg` | all labels, all modes | action label background |
| `{label}_key_fg` | named label, all modes | key foreground |
| `{label}_key_bg` | named label, all modes | key background |
| `{label}_label_fg` | named label, all modes | action label foreground |
| `{label}_label_bg` | named label, all modes | action label background |
| `{mode}.{label}_key_fg` | named label, named mode | key foreground |
| `{mode}.{label}_key_bg` | named label, named mode | key background |
| `{mode}.{label}_label_fg` | named label, named mode | action label foreground |
| `{mode}.{label}_label_bg` | named label, named mode | action label background |

Valid mode names: `normal`, `pane`, `tab`, `resize`, `move`, `scroll`, `search`, `session`.

## TODO

- [ ] more advanced mode-specific configuration
- [ ] improved handling of long outputs
- [ ] ability to enable/disable specific hints

## License

&copy; 2025 Maddison Hellstrom

Adapted from the built-in Zellij status-bar plugin by Brooks J Rady.

MIT License
