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

- `max_length`: Maximum number of characters to display (default: 0 = unlimited)
- `overflow_str`: String to append when truncated (default: "...")
- `pipe_name`: Name of the pipe for zjstatus integration (default: "zjstatus_hints")
- `hide_in_base_mode`: Hide hints in base mode (a.k.a. default mode) (default: false)

## TODO

- [ ] configurable colors/formatting
- [ ] more advanced mode-specific configuration
- [ ] improved handling of long outputs
- [ ] ability to enable/disable specific hints

## License

&copy; 2025 Maddison Cohodas

Adapted from the built-in Zellij status-bar plugin by Brooks J Rady.

MIT License
