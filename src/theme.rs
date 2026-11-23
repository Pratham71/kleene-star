//! The colour themes, defined once: the run player (`draw M on "w"`), the
//! Graphviz drawings (`draw`, `std.visual`) and the terminal UI all take
//! their colours from these tokens.
//!
//! There are three themes, `light`, `dark` and `high-contrast`, and a
//! fourth choice, `auto`: the player then follows the reader's system
//! (dark mode, more contrast), and everything else, which may be printed,
//! is light. A program picks one with `kstar run --theme <name>`, the
//! `KSTAR_THEME` environment variable, or `theme = "<name>"` in the config
//! file (`~/.config/kstar/config.toml`, or `%APPDATA%\kstar\config.toml`
//! on Windows), in that order.

use std::cell::Cell;
use std::path::PathBuf;

/// The colours of one theme, as `#rrggbb`. Every pair that is read
/// together meets WCAG AA (see the tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    /// The page, and behind a drawing.
    pub background: &'static str,
    /// Panels and boxes on the page.
    pub surface: &'static str,
    pub text: &'static str,
    /// Secondary text: edge labels, captions.
    pub muted: &'static str,
    /// The edges of panels, buttons and cells.
    pub border: &'static str,
    /// State outlines and edges.
    pub line: &'static str,
    /// Focus rings and controls.
    pub accent: &'static str,
    /// The outline of an accepting state, and "ACCEPTED".
    pub accepting: &'static str,
    /// The outline of the state(s) the machine is in.
    pub current: &'static str,
    /// The inside of the state(s) the machine is in.
    pub current_fill: &'static str,
    /// The moves just taken.
    pub edge_taken: &'static str,
    /// Rejecting states, "REJECTED" and errors.
    pub error: &'static str,
    /// Input, stack and tape cells.
    pub cell: &'static str,
}

pub const LIGHT: Theme = Theme {
    name: "light",
    background: "#ffffff",
    surface: "#f6f8fa",
    text: "#1f2328",
    muted: "#59636e",
    border: "#818b98",
    line: "#59636e",
    accent: "#0969da",
    accepting: "#1a7f37",
    current: "#0969da",
    current_fill: "#ddf4ff",
    edge_taken: "#bc4c00",
    error: "#d1242f",
    cell: "#ffffff",
};

pub const DARK: Theme = Theme {
    name: "dark",
    background: "#0d1117",
    surface: "#161b22",
    text: "#e6edf3",
    muted: "#9198a1",
    border: "#656c76",
    line: "#9198a1",
    accent: "#4493f8",
    accepting: "#3fb950",
    current: "#4493f8",
    current_fill: "#12305a",
    edge_taken: "#f0883e",
    error: "#f85149",
    cell: "#0d1117",
};

pub const HIGH_CONTRAST: Theme = Theme {
    name: "high-contrast",
    background: "#000000",
    surface: "#0a0a0a",
    text: "#ffffff",
    muted: "#e0e0e0",
    border: "#ffffff",
    line: "#ffffff",
    accent: "#ffd60a",
    accepting: "#7ee787",
    current: "#ffd60a",
    current_fill: "#3d3200",
    edge_taken: "#00e5ff",
    error: "#ff9492",
    cell: "#000000",
};

pub const THEMES: [&Theme; 3] = [&LIGHT, &DARK, &HIGH_CONTRAST];

/// What the user chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Choice {
    /// Follow the system in the player; light elsewhere.
    #[default]
    Auto,
    Light,
    Dark,
    HighContrast,
}

impl Choice {
    pub const NAMES: &'static str = "auto, light, dark or high-contrast";

    pub fn parse(name: &str) -> Option<Choice> {
        match name.trim() {
            "auto" => Some(Choice::Auto),
            "light" => Some(Choice::Light),
            "dark" => Some(Choice::Dark),
            "high-contrast" => Some(Choice::HighContrast),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Choice::Auto => "auto",
            Choice::Light => "light",
            Choice::Dark => "dark",
            Choice::HighContrast => "high-contrast",
        }
    }

    /// The colours, where one theme must be picked (a drawing, the
    /// terminal): `auto` is light, which also prints well.
    pub fn theme(self) -> &'static Theme {
        match self {
            Choice::Auto | Choice::Light => &LIGHT,
            Choice::Dark => &DARK,
            Choice::HighContrast => &HIGH_CONTRAST,
        }
    }
}

thread_local! {
    /// The choice of the run that is calling a `std.visual` function.
    static CURRENT: Cell<Choice> = const { Cell::new(Choice::Auto) };
}

/// Runs `f` (a standard-library call) with `choice` as the theme.
pub fn with_choice<T>(choice: Choice, f: impl FnOnce() -> T) -> T {
    let before = CURRENT.with(|c| c.replace(choice));
    let result = f();
    CURRENT.with(|c| c.set(before));
    result
}

/// The theme of the run in progress.
pub fn current() -> Choice {
    CURRENT.with(Cell::get)
}

/// Where the config file is: `$XDG_CONFIG_HOME/kstar/config.toml` or
/// `~/.config/kstar/config.toml`, and `%APPDATA%\kstar\config.toml` on
/// Windows.
pub fn config_path() -> Option<PathBuf> {
    if cfg!(windows) {
        return std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("kstar").join("config.toml"));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("kstar").join("config.toml"))
}

/// The `theme` setting of a config file: `theme = "dark"`. Other settings
/// and comments are ignored; this is the only setting so far.
pub fn theme_in_config(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some((key, value)) = line.split_once('=') {
            if key.trim() == "theme" {
                return Some(value.trim().trim_matches(|c| c == '"' || c == '\'').to_string());
            }
        }
    }
    None
}

/// The theme to use: the `--theme` option if given, else `KSTAR_THEME`,
/// else the config file, else `auto`. A name that is not a theme is an
/// error that says where it came from.
pub fn resolve(option: Option<&str>) -> Result<Choice, String> {
    let env = std::env::var("KSTAR_THEME").ok().filter(|v| !v.trim().is_empty());
    let config = || {
        let path = config_path()?;
        let text = std::fs::read_to_string(&path).ok()?;
        Some((theme_in_config(&text)?, path))
    };
    let (name, from) = if let Some(name) = option {
        (name.to_string(), "`--theme`".to_string())
    } else if let Some(name) = env {
        (name, "KSTAR_THEME".to_string())
    } else if let Some((name, path)) = config() {
        (name, format!("the theme in {}", path.display()))
    } else {
        return Ok(Choice::Auto);
    };
    Choice::parse(&name).ok_or_else(|| format!("{from} is `{name}`; it must be {}", Choice::NAMES))
}

/// The CSS custom properties of a theme, for the player.
pub fn css_variables(theme: &Theme) -> String {
    format!(
        "--bg: {}; --panel: {}; --fg: {}; --muted: {}; --border: {}; --line: {}; --accent: {}; \
         --accept: {}; --active: {}; --active-fill: {}; --taken: {}; --reject: {}; --cell: {};",
        theme.background,
        theme.surface,
        theme.text,
        theme.muted,
        theme.border,
        theme.line,
        theme.accent,
        theme.accepting,
        theme.current,
        theme.current_fill,
        theme.edge_taken,
        theme.error,
        theme.cell
    )
}

/// The player's style rules for every theme: `data-theme` on the page
/// picks one, and `auto` follows `prefers-color-scheme` and
/// `prefers-contrast`.
pub fn player_css() -> String {
    let mut css = String::new();
    css.push_str(&format!("  :root, :root[data-theme=\"light\"] {{ {} }}\n", css_variables(&LIGHT)));
    css.push_str(&format!("  :root[data-theme=\"dark\"] {{ {} color-scheme: dark; }}\n", css_variables(&DARK)));
    css.push_str(&format!(
        "  :root[data-theme=\"high-contrast\"] {{ {} color-scheme: dark; }}\n",
        css_variables(&HIGH_CONTRAST)
    ));
    css.push_str(&format!(
        "  @media (prefers-color-scheme: dark) {{ :root[data-theme=\"auto\"] {{ {} color-scheme: dark; }} }}\n",
        css_variables(&DARK)
    ));
    css.push_str(&format!(
        "  @media (prefers-contrast: more) {{ :root[data-theme=\"auto\"] {{ {} color-scheme: dark; }} }}\n",
        css_variables(&HIGH_CONTRAST)
    ));
    css
}

/// `#rrggbb` as red, green and blue from 0 to 1.
fn rgb(hex: &str) -> [f64; 3] {
    let hex = hex.trim_start_matches('#');
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).expect("a #rrggbb colour") as f64 / 255.0;
    [channel(0), channel(2), channel(4)]
}

/// The relative luminance of a colour, as WCAG 2 defines it.
fn luminance(hex: &str) -> f64 {
    let [r, g, b] = rgb(hex).map(|c| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// The WCAG contrast ratio of two colours, from 1 to 21.
pub fn contrast(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_matches_wcag() {
        assert!((contrast("#000000", "#ffffff") - 21.0).abs() < 1e-9);
        assert!((contrast("#777777", "#ffffff") - 4.48).abs() < 0.01);
        assert_eq!(contrast("#123456", "#123456"), 1.0);
    }

    /// Text needs 4.5:1 (AA) against what it sits on, and lines, outlines
    /// and other graphics 3:1 (WCAG 1.4.11). The high-contrast theme asks
    /// 7:1 (AAA) for text.
    #[test]
    fn every_theme_meets_wcag_aa() {
        for t in THEMES {
            let text_min = if t.name == "high-contrast" { 7.0 } else { 4.5 };
            for bg in [t.background, t.surface] {
                for (what, fg, min) in [
                    ("text", t.text, text_min),
                    ("muted", t.muted, 4.5),
                    ("accent", t.accent, 4.5),
                    ("accepting", t.accepting, 4.5),
                    ("error", t.error, 4.5),
                    ("edge taken", t.edge_taken, 4.5),
                    ("current", t.current, 4.5),
                    ("line", t.line, 3.0),
                    ("border", t.border, 3.0),
                ] {
                    let ratio = contrast(fg, bg);
                    assert!(ratio >= min, "{}: {what} {fg} on {bg} is {ratio:.2}:1, under {min}:1", t.name);
                }
            }
            for (what, fg, bg, min) in [
                ("state name in the current state", t.text, t.current_fill, text_min),
                ("current outline on its fill", t.current, t.current_fill, 3.0),
                ("cell text", t.text, t.cell, text_min),
            ] {
                let ratio = contrast(fg, bg);
                assert!(ratio >= min, "{}: {what} is {ratio:.2}:1, under {min}:1", t.name);
            }
        }
    }

    #[test]
    fn choices_and_config() {
        for name in ["auto", "light", "dark", "high-contrast"] {
            assert_eq!(Choice::parse(name).unwrap().name(), name);
        }
        assert_eq!(Choice::parse("blue"), None);
        assert_eq!(Choice::Auto.theme().name, "light");
        assert_eq!(theme_in_config("# mine\ntheme = \"dark\"  # at night\n"), Some("dark".into()));
        assert_eq!(theme_in_config("other = 1\n"), None);
        assert_eq!(with_choice(Choice::Dark, current), Choice::Dark);
        assert_eq!(current(), Choice::Auto);
    }

    #[test]
    fn the_player_css_has_every_theme() {
        let css = player_css();
        for t in THEMES {
            assert!(css.contains(&css_variables(t)), "{}", t.name);
        }
        assert!(css.contains("prefers-color-scheme: dark") && css.contains("prefers-contrast: more"));
    }
}
