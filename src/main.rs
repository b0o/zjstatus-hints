use ansi_term::{
    ANSIString, ANSIStrings,
    Colour::{Fixed, RGB},
    Style,
};
use std::collections::{BTreeMap, HashMap};
use zellij_tile::prelude::actions::Action;
use zellij_tile::prelude::actions::SearchDirection;
use zellij_tile::prelude::*;
use zellij_tile_utils::palette_match;

// ---------------------------------------------------------------------------
// Modifier style
// ---------------------------------------------------------------------------

#[derive(Default, Clone, Copy, PartialEq)]
enum ModifierStyle {
    #[default]
    Long,
    Short,
    Symbol,
}

impl ModifierStyle {
    fn from_str(s: &str) -> Self {
        match s {
            "short" => Self::Short,
            "symbol" => Self::Symbol,
            _ => Self::Long,
        }
    }
}

const DEFAULT_MODIFIER_STYLE: ModifierStyle = ModifierStyle::Long;
const DEFAULT_HINT_FORMAT: &str = "{key} {action}";
const DEFAULT_SEPARATOR: &str = "";

// ---------------------------------------------------------------------------
// Color configuration
// ---------------------------------------------------------------------------

/// Per-label color slots.  Each field is `None` when not configured, which
/// causes the caller to fall back to the Zellij palette.
#[derive(Default, Clone, Copy)]
struct LabelColors {
    key_fg: Option<ansi_term::Colour>,
    key_bg: Option<ansi_term::Colour>,
    label_fg: Option<ansi_term::Colour>,
    label_bg: Option<ansi_term::Colour>,
}

/// Full color configuration parsed from the plugin config block.
#[derive(Default, Clone)]
struct ColorConfig {
    /// Global defaults that override the palette for all labels.
    defaults: LabelColors,
    /// Per-label (and optionally per-mode) overrides.
    /// Keys: `"new"`, `"pane.new"`, `"split right"`, `"pane.split right"`, …
    overrides: HashMap<String, LabelColors>,
}

/// Parse `"#RRGGBB"` (or `"RRGGBB"`) into an `ansi_term::Colour`.
/// Returns `None` for any other format — invalid values are silently ignored.
fn parse_hex_color(s: &str) -> Option<ansi_term::Colour> {
    let hex = s.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(RGB(r, g, b))
}

/// Scan the config map for keys matching `*_key_fg`, `*_key_bg`, `*_label_fg`,
/// `*_label_bg` and build the per-label override table.
///
/// Naming convention:
/// - `select_key_bg`          → label `"select"`, all modes
/// - `split_right_key_bg`     → label `"split right"`, all modes
/// - `pane.new_key_bg`        → label `"new"`, mode `"pane"` only
/// - `pane.split_right_key_bg`→ label `"split right"`, mode `"pane"` only
///
/// Global defaults (`key_fg`, `key_bg`, `label_fg`, `label_bg`) are handled
/// separately in `load()` and are NOT inserted into this map.
fn parse_label_overrides(config: &BTreeMap<String, String>) -> HashMap<String, LabelColors> {
    // Suffixes we recognise, ordered longest-first so `_label_fg` is checked
    // before a hypothetical shorter suffix never conflicts.
    const SUFFIXES: &[&str] = &["_key_fg", "_key_bg", "_label_fg", "_label_bg"];

    // These bare keys are the global defaults — handled elsewhere.
    const GLOBAL_DEFAULTS: &[&str] = &["key_fg", "key_bg", "label_fg", "label_bg"];

    let mut overrides: HashMap<String, LabelColors> = HashMap::new();

    'outer: for (config_key, value) in config.iter() {
        // Skip the four global-default keys.
        if GLOBAL_DEFAULTS.contains(&config_key.as_str()) {
            continue;
        }

        for suffix in SUFFIXES {
            if !config_key.ends_with(suffix) {
                continue;
            }

            // Everything before the suffix is either `"label"` or
            // `"mode.label"` (with underscores standing in for spaces).
            let raw_prefix = &config_key[..config_key.len() - suffix.len()];
            if raw_prefix.is_empty() {
                continue 'outer;
            }

            // Build the lookup key used in `overrides`.
            let lookup_key = if let Some(dot) = raw_prefix.find('.') {
                let mode = &raw_prefix[..dot];
                let label_raw = &raw_prefix[dot + 1..];
                if mode.is_empty() || label_raw.is_empty() {
                    continue 'outer;
                }
                // Labels use underscores in config but spaces internally.
                format!("{}.{}", mode, label_raw.replace('_', " "))
            } else {
                // Label-only.
                let label = raw_prefix.replace('_', " ");
                label
            };

            // Guard against whitespace-only results.
            if lookup_key.trim().is_empty() {
                continue 'outer;
            }

            if let Some(color) = parse_hex_color(value) {
                let entry = overrides.entry(lookup_key).or_default();
                match *suffix {
                    "_key_fg" => entry.key_fg = Some(color),
                    "_key_bg" => entry.key_bg = Some(color),
                    "_label_fg" => entry.label_fg = Some(color),
                    "_label_bg" => entry.label_bg = Some(color),
                    _ => {}
                }
            }

            // Each config key can only match one suffix.
            continue 'outer;
        }
    }

    overrides
}

/// Resolve the effective colors for `label` in `mode`, applying the 4-level
/// priority independently for each color slot:
///
/// 1. `"{mode}.{label}"` override  (most specific)
/// 2. `"{label}"` override
/// 3. Global default
/// 4. Caller falls back to Zellij palette (when `None` is returned)
fn get_colors_for_label(config: &ColorConfig, mode: Option<&str>, label: &str) -> LabelColors {
    let mode_specific = mode.and_then(|m| config.overrides.get(&format!("{}.{}", m, label)));
    let label_only = config.overrides.get(label);

    // Each field resolved independently.
    LabelColors {
        key_fg: mode_specific.and_then(|o| o.key_fg)
            .or_else(|| label_only.and_then(|o| o.key_fg))
            .or(config.defaults.key_fg),
        key_bg: mode_specific.and_then(|o| o.key_bg)
            .or_else(|| label_only.and_then(|o| o.key_bg))
            .or(config.defaults.key_bg),
        label_fg: mode_specific.and_then(|o| o.label_fg)
            .or_else(|| label_only.and_then(|o| o.label_fg))
            .or(config.defaults.label_fg),
        label_bg: mode_specific.and_then(|o| o.label_bg)
            .or_else(|| label_only.and_then(|o| o.label_bg))
            .or(config.defaults.label_bg),
    }
}

// ---------------------------------------------------------------------------
// Plugin state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct State {
    initialized: bool,
    pipe_name: String,
    mode_info: ModeInfo,
    base_mode_is_locked: bool,
    max_length: usize,
    overflow_str: String,
    hide_in_base_mode: bool,
    modifier_style: ModifierStyle,
    hint_format: String,
    separator: String,
    color_config: ColorConfig,
    /// Maximum number of alternative keybindings shown per action (0 = unlimited).
    max_keys: usize,
    /// User-defined display aliases for action labels. Key = original label
    /// (e.g. `"split right"`), value = replacement string shown in the bar.
    action_aliases: HashMap<String, String>,
    /// User-defined display aliases for special bare keys.
    /// Key = lowercase key name (e.g. `"enter"`, `"space"`), value = replacement.
    key_aliases: HashMap<String, String>,
}

register_plugin!(State);

const TO_NORMAL: Action = Action::SwitchToMode(InputMode::Normal);

const PLUGIN_SESSION_MANAGER: &str = "session-manager";
const PLUGIN_CONFIGURATION: &str = "configuration";
const PLUGIN_MANAGER: &str = "plugin-manager";
const PLUGIN_ABOUT: &str = "zellij:about";

const KEY_PATTERNS_NO_SEPARATOR: &[&str] = &["HJKL", "hjkl", "←↓↑→", "←→", "↓↑", "[]"];

const DEFAULT_MAX_LENGTH: usize = 0;
const DEFAULT_MAX_KEYS: usize = 0;
const DEFAULT_OVERFLOW_STR: &str = "...";
const DEFAULT_PIPE_NAME: &str = "zjstatus_hints";

type ActionLabel = (Action, &'static str);
type ActionSequenceLabel = (&'static [Action], &'static str);

const NORMAL_MODE_ACTIONS: &[ActionLabel] = &[
    (Action::SwitchToMode(InputMode::Pane), "pane"),
    (Action::SwitchToMode(InputMode::Tab), "tab"),
    (Action::SwitchToMode(InputMode::Resize), "resize"),
    (Action::SwitchToMode(InputMode::Move), "move"),
    (Action::SwitchToMode(InputMode::Scroll), "scroll"),
    (Action::SwitchToMode(InputMode::Search), "search"),
    (Action::SwitchToMode(InputMode::Session), "session"),
    (Action::Quit, "quit"),
];

const PANE_MODE_ACTION_SEQUENCES: &[ActionSequenceLabel] = &[
    (&[Action::NewPane(None, None, false), TO_NORMAL], "new"),
    (&[Action::CloseFocus, TO_NORMAL], "close"),
    (&[Action::ToggleFocusFullscreen, TO_NORMAL], "fullscreen"),
    (&[Action::ToggleFloatingPanes, TO_NORMAL], "float"),
    (&[Action::TogglePaneEmbedOrFloating, TO_NORMAL], "embed"),
    (
        &[
            Action::NewPane(Some(Direction::Right), None, false),
            TO_NORMAL,
        ],
        "split right",
    ),
    (
        &[
            Action::NewPane(Some(Direction::Down), None, false),
            TO_NORMAL,
        ],
        "split down",
    ),
];

const TAB_MODE_ACTION_SEQUENCES: &[ActionSequenceLabel] = &[
    (
        &[
            Action::NewTab(None, vec![], None, None, None, true),
            TO_NORMAL,
        ],
        "new",
    ),
    (&[Action::CloseTab, TO_NORMAL], "close"),
    (&[Action::BreakPane, TO_NORMAL], "break pane"),
    (&[Action::ToggleActiveSyncTab, TO_NORMAL], "sync"),
];

// ---------------------------------------------------------------------------
// ZellijPlugin impl
// ---------------------------------------------------------------------------

fn get_common_modifiers(mut key_bindings: Vec<&KeyWithModifier>) -> Vec<KeyModifier> {
    if key_bindings.is_empty() {
        return vec![];
    }
    let mut common_modifiers = key_bindings.pop().unwrap().key_modifiers.clone();
    for key in key_bindings {
        common_modifiers = common_modifiers
            .intersection(&key.key_modifiers)
            .cloned()
            .collect();
    }
    common_modifiers.into_iter().collect()
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.initialized = false;

        // TODO: configuration validation
        self.max_length = configuration
            .get("max_length")
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_LENGTH);
        self.overflow_str = configuration
            .get("overflow_str")
            .cloned()
            .unwrap_or_else(|| DEFAULT_OVERFLOW_STR.to_string());
        self.pipe_name = configuration
            .get("pipe_name")
            .cloned()
            .unwrap_or_else(|| DEFAULT_PIPE_NAME.to_string());
        self.hide_in_base_mode = configuration
            .get("hide_in_base_mode")
            .map(|s| s.to_lowercase().parse::<bool>().unwrap_or(false))
            .unwrap_or(false);
        self.modifier_style = configuration
            .get("modifier_style")
            .map(|s| ModifierStyle::from_str(s.as_str()))
            .unwrap_or(DEFAULT_MODIFIER_STYLE);
        self.hint_format = configuration
            .get("hint_format")
            .cloned()
            .unwrap_or_else(|| DEFAULT_HINT_FORMAT.to_string());
        self.separator = configuration
            .get("separator")
            .cloned()
            .unwrap_or_else(|| DEFAULT_SEPARATOR.to_string());
        self.color_config = ColorConfig {
            defaults: LabelColors {
                key_fg: configuration.get("key_fg").and_then(|s| parse_hex_color(s)),
                key_bg: configuration.get("key_bg").and_then(|s| parse_hex_color(s)),
                label_fg: configuration.get("label_fg").and_then(|s| parse_hex_color(s)),
                label_bg: configuration.get("label_bg").and_then(|s| parse_hex_color(s)),
            },
            overrides: parse_label_overrides(&configuration),
        };
        self.max_keys = configuration
            .get("max_keys")
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_KEYS);
        self.action_aliases = configuration
            .iter()
            .filter_map(|(k, v)| {
                k.strip_prefix("alias_")
                    .map(|label_raw| (label_raw.replace('_', " "), v.clone()))
            })
            .collect();
        self.key_aliases = configuration
            .iter()
            .filter_map(|(k, v)| {
                k.strip_prefix("key_alias_")
                    .map(|key_raw| (key_raw.to_lowercase(), v.clone()))
            })
            .collect();

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::MessageAndLaunchOtherPlugins,
        ]);

        set_selectable(false);
        subscribe(&[EventType::ModeUpdate, EventType::SessionUpdate]);
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = !self.initialized;
        if let Event::ModeUpdate(mode_info) = event {
            if self.mode_info != mode_info {
                should_render = true;
            }
            self.mode_info = mode_info;
            self.base_mode_is_locked = self.mode_info.base_mode == Some(InputMode::Locked);
        };
        should_render
    }

    fn render(&mut self, _rows: usize, _cols: usize) {
        let mode_info = &self.mode_info;
        let output = if !(self.hide_in_base_mode && Some(mode_info.mode) == mode_info.base_mode) {
            let keymap = get_keymap_for_mode(mode_info);
            let parts = render_hints_for_mode(
                mode_info.mode,
                &keymap,
                &mode_info.style.colors,
                &self.color_config,
                self.modifier_style,
                &self.hint_format,
                &self.separator,
                self.max_keys,
                &self.action_aliases,
                &self.key_aliases,
            );

            let ansi_strings = ANSIStrings(&parts);
            let formatted = format!(" {}", ansi_strings);

            let visible_len = calculate_visible_length(&formatted);
            if self.max_length > 0 && visible_len > self.max_length {
                truncate_ansi_string(&formatted, &self.overflow_str, self.max_length)
            } else {
                formatted.to_string()
            }
        } else {
            String::new()
        };

        // HACK: Because we're not sure when zjstatus will be ready to receive messages,
        // we'll repeatedly send messages until the user has switched to a different mode,
        // at which point we'll assume that zjstatus has been initialized. The render function
        // does not seem to be called too frequently, so this should be fine.
        if !output.is_empty() && Some(mode_info.mode) != mode_info.base_mode {
            self.initialized = true;
        }

        pipe_message_to_plugin(MessageToPlugin::new("pipe").with_payload(format!(
            "zjstatus::pipe::pipe_{}::{}",
            self.pipe_name, output
        )));
        print!("{}", output);
    }
}

// ---------------------------------------------------------------------------
// ANSI helpers
// ---------------------------------------------------------------------------

struct AnsiParser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> AnsiParser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            chars: text.chars().peekable(),
        }
    }

    fn next_segment(&mut self) -> Option<AnsiSegment> {
        let ch = self.chars.next()?;

        if ch == '\x1b' {
            let mut escape_seq = String::from(ch);
            for escape_ch in self.chars.by_ref() {
                escape_seq.push(escape_ch);
                if escape_ch == 'm' {
                    break;
                }
            }
            Some(AnsiSegment::EscapeSequence(escape_seq))
        } else {
            Some(AnsiSegment::VisibleChar(ch))
        }
    }
}

enum AnsiSegment {
    EscapeSequence(String),
    VisibleChar(char),
}

fn calculate_visible_length(text: &str) -> usize {
    let mut parser = AnsiParser::new(text);
    let mut len = 0;

    while let Some(segment) = parser.next_segment() {
        if matches!(segment, AnsiSegment::VisibleChar(_)) {
            len += 1;
        }
    }

    len
}

fn truncate_ansi_string(text: &str, overflow_str: &str, max_len: usize) -> String {
    let visible_len = calculate_visible_length(text);
    let overflow_len = overflow_str.len();

    if visible_len <= max_len {
        return text.to_string();
    }

    if max_len <= overflow_len {
        return overflow_str.to_string();
    }

    let target_len = max_len - overflow_len;
    let mut result = String::new();
    let mut visible_count = 0;
    let mut parser = AnsiParser::new(text);

    while let Some(segment) = parser.next_segment() {
        match segment {
            AnsiSegment::EscapeSequence(seq) => {
                result.push_str(&seq);
            }
            AnsiSegment::VisibleChar(ch) => {
                if visible_count >= target_len {
                    break;
                }
                result.push(ch);
                visible_count += 1;
            }
        }
    }

    result.push_str(overflow_str);
    result
}

// ---------------------------------------------------------------------------
// Key-finding helpers
// ---------------------------------------------------------------------------

fn find_keys_for_actions(
    keymap: &[(KeyWithModifier, Vec<Action>)],
    target_actions: &[Action],
    exact_match: bool,
) -> Vec<KeyWithModifier> {
    keymap
        .iter()
        .filter_map(|(key, key_actions)| {
            if exact_match {
                let matching = key_actions
                    .iter()
                    .zip(target_actions)
                    .filter(|(a, b)| a.shallow_eq(b))
                    .count();
                if matching == key_actions.len() && matching == target_actions.len() {
                    Some(key.clone())
                } else {
                    None
                }
            } else if key_actions.iter().next() == target_actions.iter().next() {
                Some(key.clone())
            } else {
                None
            }
        })
        .collect()
}

fn find_keys_for_action_groups(
    keymap: &[(KeyWithModifier, Vec<Action>)],
    action_groups: &[&[Action]],
) -> Vec<KeyWithModifier> {
    action_groups
        .iter()
        .flat_map(|actions| find_keys_for_actions(keymap, actions, true))
        .collect()
}

// ---------------------------------------------------------------------------
// Modifier rendering
// ---------------------------------------------------------------------------

fn modifier_name(modifier: &KeyModifier, style: ModifierStyle) -> String {
    match style {
        ModifierStyle::Long => modifier.to_string(),
        ModifierStyle::Short => match modifier {
            KeyModifier::Ctrl => "C".to_string(),
            KeyModifier::Alt => "A".to_string(),
            KeyModifier::Shift => "S".to_string(),
            _ => modifier.to_string(),
        },
        ModifierStyle::Symbol => match modifier {
            KeyModifier::Ctrl => "^".to_string(),
            KeyModifier::Alt => "M-".to_string(),
            KeyModifier::Shift => "S-".to_string(),
            _ => modifier.to_string(),
        },
    }
}

fn format_modifier_string(modifiers: &[KeyModifier], style: ModifierStyle) -> String {
    if modifiers.is_empty() {
        String::new()
    } else {
        modifiers
            .iter()
            .map(|m| modifier_name(m, style))
            .collect::<Vec<_>>()
            .join("-")
    }
}

fn modifier_separator(style: ModifierStyle) -> &'static str {
    match style {
        ModifierStyle::Long => " + ",
        ModifierStyle::Short => "-",
        ModifierStyle::Symbol => "",
    }
}

// ---------------------------------------------------------------------------
// Key display formatting
// ---------------------------------------------------------------------------

/// Return the display string for a bare key, applying `key_aliases` when present.
/// The alias map uses lowercase key names as keys (e.g. `"enter"`, `"space"`).
fn bare_key_display(bare_key: &BareKey, key_aliases: &HashMap<String, String>) -> String {
    let default = format!("{}", bare_key);
    // Look up by lowercased default representation.
    key_aliases
        .get(&default.to_lowercase())
        .cloned()
        .unwrap_or(default)
}

fn format_key_display(
    key_bindings: &[KeyWithModifier],
    common_modifiers: &[KeyModifier],
    key_aliases: &HashMap<String, String>,
) -> Vec<String> {
    key_bindings
        .iter()
        .map(|key| {
            if common_modifiers.is_empty() {
                // Full key with modifiers already rendered by Display; we only
                // substitute the bare-key portion so modifier prefixes are kept.
                let bare = bare_key_display(&key.bare_key, key_aliases);
                if key.key_modifiers.is_empty() {
                    bare
                } else {
                    let mods = key
                        .key_modifiers
                        .iter()
                        .map(|m| m.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("{} {}", mods, bare)
                }
            } else {
                let unique_modifiers = key
                    .key_modifiers
                    .iter()
                    .filter(|m| !common_modifiers.contains(m))
                    .map(|m| m.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                let bare = bare_key_display(&key.bare_key, key_aliases);
                if unique_modifiers.is_empty() {
                    bare
                } else {
                    format!("{} {}", unique_modifiers, bare)
                }
            }
        })
        .collect()
}

fn get_key_separator(key_display: &[String]) -> &'static str {
    let key_string = key_display.join("");
    if KEY_PATTERNS_NO_SEPARATOR.contains(&&key_string[..]) {
        ""
    } else {
        "|"
    }
}

// ---------------------------------------------------------------------------
// Styled rendering
// ---------------------------------------------------------------------------

fn style_key_with_modifier(
    key_bindings: &[KeyWithModifier],
    palette: &Styling,
    color_config: &ColorConfig,
    mode: Option<&str>,
    label: &str,
    modifier_style: ModifierStyle,
    key_aliases: &HashMap<String, String>,
) -> Vec<ANSIString<'static>> {
    if key_bindings.is_empty() {
        return vec![];
    }

    let resolved = get_colors_for_label(color_config, mode, label);
    let fg = resolved
        .key_fg
        .unwrap_or_else(|| palette_match!(palette.ribbon_unselected.base));
    let bg = resolved
        .key_bg
        .unwrap_or_else(|| palette_match!(palette.ribbon_unselected.background));

    let mut styled_parts = vec![];

    let common_modifiers = get_common_modifiers(key_bindings.iter().collect());
    let modifier_str = format_modifier_string(&common_modifiers, modifier_style);
    let key_display = format_key_display(key_bindings, &common_modifiers, key_aliases);
    let key_separator = get_key_separator(&key_display);

    styled_parts.push(Style::new().paint(" "));

    if !modifier_str.is_empty() {
        let sep = modifier_separator(modifier_style);
        styled_parts.push(
            Style::new()
                .fg(fg)
                .on(bg)
                .bold()
                .paint(format!(" {}{}", modifier_str, sep)),
        );
    } else {
        styled_parts.push(Style::new().fg(fg).on(bg).paint(" "));
    }

    for (idx, key) in key_display.iter().enumerate() {
        if idx > 0 && !key_separator.is_empty() {
            styled_parts.push(Style::new().fg(fg).on(bg).paint(key_separator));
        }
        styled_parts.push(Style::new().fg(fg).on(bg).bold().paint(key.clone()));
    }

    styled_parts.push(Style::new().fg(fg).on(bg).paint(" "));

    styled_parts
}

fn style_description(
    description: &str,
    palette: &Styling,
    color_config: &ColorConfig,
    mode: Option<&str>,
    label: &str,
) -> Vec<ANSIString<'static>> {
    let resolved = get_colors_for_label(color_config, mode, label);
    let fg = resolved
        .label_fg
        .unwrap_or_else(|| palette_match!(palette.text_unselected.base));
    let bg = resolved
        .label_bg
        .unwrap_or_else(|| palette_match!(palette.text_unselected.background));

    vec![Style::new()
        .fg(fg)
        .on(bg)
        .paint(format!(" {} ", description))]
}

/// Plain-text key representation used by custom `hint_format` templates.
fn format_key_plain(
    key_bindings: &[KeyWithModifier],
    modifier_style: ModifierStyle,
    key_aliases: &HashMap<String, String>,
) -> String {
    if key_bindings.is_empty() {
        return String::new();
    }
    let common_modifiers = get_common_modifiers(key_bindings.iter().collect());
    let modifier_str = format_modifier_string(&common_modifiers, modifier_style);
    let key_display = format_key_display(key_bindings, &common_modifiers, key_aliases);
    let key_separator = get_key_separator(&key_display);
    let keys_str = key_display.join(key_separator);
    if modifier_str.is_empty() {
        keys_str
    } else {
        let sep = modifier_separator(modifier_style);
        format!("{}{}{}", modifier_str, sep, keys_str)
    }
}

fn plugin_key(
    keymap: &[(KeyWithModifier, Vec<Action>)],
    plugin_name: &str,
) -> Option<KeyWithModifier> {
    keymap.iter().find_map(|(key, key_actions)| {
        if key_actions
            .iter()
            .any(|action| action.launches_plugin(plugin_name))
        {
            Some(key.clone())
        } else {
            None
        }
    })
}

fn get_select_key(keymap: &[(KeyWithModifier, Vec<Action>)]) -> Vec<KeyWithModifier> {
    let to_normal_keys = find_keys_for_actions(keymap, &[TO_NORMAL], true);
    if to_normal_keys.contains(&KeyWithModifier::new(BareKey::Enter)) {
        vec![KeyWithModifier::new(BareKey::Enter)]
    } else {
        to_normal_keys.into_iter().take(1).collect()
    }
}

// ---------------------------------------------------------------------------
// Hint assembly
// ---------------------------------------------------------------------------

fn add_hint(
    parts: &mut Vec<ANSIString<'static>>,
    keys: &[KeyWithModifier],
    description: &str,
    palette: &Styling,
    color_config: &ColorConfig,
    mode: Option<&str>,
    modifier_style: ModifierStyle,
    hint_format: &str,
    separator: &str,
    max_keys: usize,
    action_aliases: &HashMap<String, String>,
    key_aliases: &HashMap<String, String>,
) {
    if keys.is_empty() {
        return;
    }

    // Apply max_keys truncation (0 = unlimited).
    let keys = if max_keys > 0 && keys.len() > max_keys {
        &keys[..max_keys]
    } else {
        keys
    };

    // Apply key_aliases to the key slice by producing rewritten keys — we do
    // this at the display level inside format_key_plain / style_key_with_modifier
    // by passing the alias map through.

    // Displayed label: alias overrides display only; color lookups use original.
    let display_label: &str = action_aliases
        .get(description)
        .map(|s| s.as_str())
        .unwrap_or(description);

    if !separator.is_empty() && !parts.is_empty() {
        parts.push(Style::new().paint(separator.to_string()));
    }

    if hint_format == DEFAULT_HINT_FORMAT {
        // Default layout: two distinct styled blocks (key + action).
        // Color lookup uses original `description` label.
        let styled_keys = style_key_with_modifier(
            keys,
            palette,
            color_config,
            mode,
            description,
            modifier_style,
            key_aliases,
        );
        parts.extend(styled_keys);
        let styled_desc =
            style_description(display_label, palette, color_config, mode, description);
        parts.extend(styled_desc);
    } else {
        // Custom template: render as a single plain-text string.
        let key_plain = format_key_plain(keys, modifier_style, key_aliases);
        let rendered = hint_format
            .replace("{key}", &key_plain)
            .replace("{action}", display_label);

        // Color lookup uses original `description` label.
        let resolved = get_colors_for_label(color_config, mode, description);
        let fg = resolved
            .key_fg
            .unwrap_or_else(|| palette_match!(palette.ribbon_unselected.base));
        let bg = resolved
            .key_bg
            .unwrap_or_else(|| palette_match!(palette.ribbon_unselected.background));

        parts.push(Style::new().fg(fg).on(bg).paint(rendered));
    }
}

// ---------------------------------------------------------------------------
// Mode → string
// ---------------------------------------------------------------------------

fn mode_to_str(mode: InputMode) -> Option<&'static str> {
    match mode {
        InputMode::Normal => Some("normal"),
        InputMode::Pane => Some("pane"),
        InputMode::Tab => Some("tab"),
        InputMode::Resize => Some("resize"),
        InputMode::Move => Some("move"),
        InputMode::Scroll => Some("scroll"),
        InputMode::Search => Some("search"),
        InputMode::Session => Some("session"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Per-mode hint rendering
// ---------------------------------------------------------------------------

fn render_hints_for_mode(
    mode: InputMode,
    keymap: &[(KeyWithModifier, Vec<Action>)],
    palette: &Styling,
    color_config: &ColorConfig,
    modifier_style: ModifierStyle,
    hint_format: &str,
    separator: &str,
    max_keys: usize,
    action_aliases: &HashMap<String, String>,
    key_aliases: &HashMap<String, String>,
) -> Vec<ANSIString<'static>> {
    let mut parts = vec![];
    let select_keys = get_select_key(keymap);
    let mode_str = mode_to_str(mode);

    macro_rules! hint {
        ($keys:expr, $label:expr) => {
            add_hint(
                &mut parts,
                $keys,
                $label,
                palette,
                color_config,
                mode_str,
                modifier_style,
                hint_format,
                separator,
                max_keys,
                action_aliases,
                key_aliases,
            )
        };
    }

    match mode {
        InputMode::Normal => {
            for (action, label) in NORMAL_MODE_ACTIONS {
                let keys = find_keys_for_actions(keymap, &[action.clone()], true);
                hint!(&keys, label);
            }
        }
        InputMode::Pane => {
            for (actions, label) in PANE_MODE_ACTION_SEQUENCES {
                let keys = find_keys_for_actions(keymap, actions, true);
                hint!(&keys, label);
            }

            let focus_keys_full = find_keys_for_action_groups(
                keymap,
                &[
                    &[Action::MoveFocusOrTab(Direction::Left)],
                    &[Action::MoveFocusOrTab(Direction::Right)],
                    &[Action::MoveFocus(Direction::Left)],
                    &[Action::MoveFocus(Direction::Down)],
                    &[Action::MoveFocus(Direction::Up)],
                    &[Action::MoveFocus(Direction::Right)],
                ],
            );
            let focus_keys = if focus_keys_full.contains(&KeyWithModifier::new(BareKey::Left))
                && focus_keys_full.contains(&KeyWithModifier::new(BareKey::Right))
            {
                vec![
                    KeyWithModifier::new(BareKey::Left),
                    KeyWithModifier::new(BareKey::Right),
                ]
            } else {
                focus_keys_full
            };
            hint!(&focus_keys, "move");
            let rename_keys = find_keys_for_actions(
                keymap,
                &[
                    Action::SwitchToMode(InputMode::RenamePane),
                    Action::PaneNameInput(vec![0]),
                ],
                false,
            );
            if !rename_keys.is_empty() {
                hint!(&rename_keys, "rename");
            }
            hint!(&select_keys, "select");
        }
        InputMode::Tab => {
            for (actions, label) in TAB_MODE_ACTION_SEQUENCES {
                let keys = find_keys_for_actions(keymap, actions, true);
                hint!(&keys, label);
            }

            let focus_keys_full = find_keys_for_action_groups(
                keymap,
                &[&[Action::GoToPreviousTab], &[Action::GoToNextTab]],
            );
            let focus_keys = if focus_keys_full.contains(&KeyWithModifier::new(BareKey::Left))
                && focus_keys_full.contains(&KeyWithModifier::new(BareKey::Right))
            {
                vec![
                    KeyWithModifier::new(BareKey::Left),
                    KeyWithModifier::new(BareKey::Right),
                ]
            } else {
                focus_keys_full
            };
            hint!(&focus_keys, "move");
            let rename_keys = find_keys_for_actions(
                keymap,
                &[
                    Action::SwitchToMode(InputMode::RenameTab),
                    Action::TabNameInput(vec![0]),
                ],
                false,
            );
            if !rename_keys.is_empty() {
                hint!(&rename_keys, "rename");
            }
            hint!(&select_keys, "select");
        }
        InputMode::Resize => {
            let resize_keys = find_keys_for_action_groups(
                keymap,
                &[
                    &[Action::Resize(Resize::Increase, None)],
                    &[Action::Resize(Resize::Decrease, None)],
                ],
            );
            hint!(&resize_keys, "resize");

            let increase_keys = find_keys_for_action_groups(
                keymap,
                &[
                    &[Action::Resize(Resize::Increase, Some(Direction::Left))],
                    &[Action::Resize(Resize::Increase, Some(Direction::Down))],
                    &[Action::Resize(Resize::Increase, Some(Direction::Up))],
                    &[Action::Resize(Resize::Increase, Some(Direction::Right))],
                ],
            );
            hint!(&increase_keys, "increase");

            let decrease_keys = find_keys_for_action_groups(
                keymap,
                &[
                    &[Action::Resize(Resize::Decrease, Some(Direction::Left))],
                    &[Action::Resize(Resize::Decrease, Some(Direction::Down))],
                    &[Action::Resize(Resize::Decrease, Some(Direction::Up))],
                    &[Action::Resize(Resize::Decrease, Some(Direction::Right))],
                ],
            );
            hint!(&decrease_keys, "decrease");
            hint!(&select_keys, "select");
        }
        InputMode::Move => {
            let move_keys = find_keys_for_action_groups(
                keymap,
                &[
                    &[Action::MovePane(Some(Direction::Left))],
                    &[Action::MovePane(Some(Direction::Down))],
                    &[Action::MovePane(Some(Direction::Up))],
                    &[Action::MovePane(Some(Direction::Right))],
                ],
            );
            hint!(&move_keys, "move");
            hint!(&select_keys, "select");
        }
        InputMode::Scroll => {
            let search_keys = find_keys_for_actions(
                keymap,
                &[
                    Action::SwitchToMode(InputMode::EnterSearch),
                    Action::SearchInput(vec![0]),
                ],
                true,
            );
            hint!(&search_keys, "search");

            let scroll_keys =
                find_keys_for_action_groups(keymap, &[&[Action::ScrollDown], &[Action::ScrollUp]]);
            hint!(&scroll_keys, "scroll");

            let page_scroll_keys = find_keys_for_action_groups(
                keymap,
                &[&[Action::PageScrollDown], &[Action::PageScrollUp]],
            );
            hint!(&page_scroll_keys, "page");

            let half_page_scroll_keys = find_keys_for_action_groups(
                keymap,
                &[&[Action::HalfPageScrollDown], &[Action::HalfPageScrollUp]],
            );
            hint!(&half_page_scroll_keys, "half page");

            let edit_keys =
                find_keys_for_actions(keymap, &[Action::EditScrollback, TO_NORMAL], false);
            if !edit_keys.is_empty() {
                hint!(&edit_keys, "edit");
            }
            hint!(&select_keys, "select");
        }
        InputMode::Search => {
            let search_keys = find_keys_for_actions(
                keymap,
                &[
                    Action::SwitchToMode(InputMode::EnterSearch),
                    Action::SearchInput(vec![0]),
                ],
                true,
            );
            hint!(&search_keys, "search");

            let scroll_keys =
                find_keys_for_action_groups(keymap, &[&[Action::ScrollDown], &[Action::ScrollUp]]);
            hint!(&scroll_keys, "scroll");

            let page_scroll_keys = find_keys_for_action_groups(
                keymap,
                &[&[Action::PageScrollDown], &[Action::PageScrollUp]],
            );
            hint!(&page_scroll_keys, "page");

            let half_page_scroll_keys = find_keys_for_action_groups(
                keymap,
                &[&[Action::HalfPageScrollDown], &[Action::HalfPageScrollUp]],
            );
            hint!(&half_page_scroll_keys, "half page");

            let down_keys =
                find_keys_for_actions(keymap, &[Action::Search(SearchDirection::Down)], true);
            hint!(&down_keys, "down");

            let up_keys =
                find_keys_for_actions(keymap, &[Action::Search(SearchDirection::Up)], true);
            hint!(&up_keys, "up");

            hint!(&select_keys, "select");
        }
        InputMode::Session => {
            let detach_keys = find_keys_for_actions(keymap, &[Action::Detach], true);
            hint!(&detach_keys, "detach");

            if let Some(manager_key) = plugin_key(keymap, PLUGIN_SESSION_MANAGER) {
                hint!(&[manager_key], "manager");
            }

            if let Some(config_key) = plugin_key(keymap, PLUGIN_CONFIGURATION) {
                hint!(&[config_key], "config");
            }

            if let Some(plugin_key_val) = plugin_key(keymap, PLUGIN_MANAGER) {
                hint!(&[plugin_key_val], "plugins");
            }

            if let Some(about_key) = plugin_key(keymap, PLUGIN_ABOUT) {
                hint!(&[about_key], "about");
            }

            hint!(&select_keys, "select");
        }
        _ => {
            let keys =
                find_keys_for_actions(keymap, &[Action::SwitchToMode(InputMode::Normal)], true);
            hint!(&keys, "normal");
        }
    }

    parts
}

fn get_keymap_for_mode(mode_info: &ModeInfo) -> Vec<(KeyWithModifier, Vec<Action>)> {
    match mode_info.mode {
        InputMode::Normal => mode_info.get_keybinds_for_mode(InputMode::Normal),
        InputMode::Pane => mode_info.get_keybinds_for_mode(InputMode::Pane),
        InputMode::Tab => mode_info.get_keybinds_for_mode(InputMode::Tab),
        InputMode::Resize => mode_info.get_keybinds_for_mode(InputMode::Resize),
        InputMode::Move => mode_info.get_keybinds_for_mode(InputMode::Move),
        InputMode::Scroll => mode_info.get_keybinds_for_mode(InputMode::Scroll),
        InputMode::Search => mode_info.get_keybinds_for_mode(InputMode::Search),
        InputMode::Session => mode_info.get_keybinds_for_mode(InputMode::Session),
        _ => mode_info.get_mode_keybinds(),
    }
}
