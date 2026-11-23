//! The terminal UI: `kstar update` on a terminal, and `kstar tui`, a home
//! screen to pick a file and a machine and run, trace, draw or export it.
//!
//! Each screen is a plain value with a `draw` and a `key` method, so the
//! tests drive it with keys and compare what it draws (on ratatui's
//! `TestBackend`) with the snapshots in `src/tui/snapshots/`. Everything a
//! screen does to the world goes through a trait the `kstar` program
//! implements (`update::Installer`, `home::Backend`), and everything it
//! does can also be done with flags, for scripts and CI.
//!
//! The colours are the theme tokens of `crate::theme`. With `auto` the UI
//! keeps the terminal's own background and text colours and uses the
//! terminal's named colours, which every terminal theme tunes to be
//! readable; a named theme paints its own background.

pub mod home;
pub mod update;

use crate::theme::{Choice, Theme};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use std::io::IsTerminal;
use std::time::Duration;

/// Whether to show the terminal UI: standard input and output are
/// terminals, `--no-tui` was not given, and neither `NO_COLOR` nor a dumb
/// terminal (`TERM=dumb`) asks for plain output.
pub fn wanted(no_tui: bool) -> bool {
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let dumb = std::env::var("TERM").is_ok_and(|t| t == "dumb");
    !no_tui && !no_color && !dumb && std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// The styles a screen draws with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// The whole screen: the background and the ordinary text.
    pub base: Style,
    pub muted: Style,
    pub border: Style,
    /// Titles and the keys in the key bar.
    pub accent: Style,
    /// The selected row of a list.
    pub selected: Style,
    pub accepting: Style,
    pub current: Style,
    pub taken: Style,
    pub error: Style,
}

fn rgb(hex: &str) -> Color {
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
    Color::Rgb(channel(1), channel(3), channel(5))
}

impl Palette {
    pub fn of(choice: Choice) -> Palette {
        match choice {
            Choice::Auto => Palette {
                base: Style::default(),
                muted: Style::default().add_modifier(Modifier::DIM),
                border: Style::default().add_modifier(Modifier::DIM),
                accent: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                selected: Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD),
                accepting: Style::default().fg(Color::Green),
                current: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                taken: Style::default().fg(Color::Magenta),
                error: Style::default().fg(Color::Red),
            },
            other => Palette::from_theme(other.theme()),
        }
    }

    fn from_theme(t: &Theme) -> Palette {
        let base = Style::default().fg(rgb(t.text)).bg(rgb(t.background));
        Palette {
            base,
            muted: base.fg(rgb(t.muted)),
            border: base.fg(rgb(t.border)),
            accent: base.fg(rgb(t.accent)).add_modifier(Modifier::BOLD),
            selected: Style::default().fg(rgb(t.text)).bg(rgb(t.current_fill)).add_modifier(Modifier::BOLD),
            accepting: base.fg(rgb(t.accepting)),
            current: base.fg(rgb(t.current)).add_modifier(Modifier::BOLD),
            taken: base.fg(rgb(t.edge_taken)),
            error: base.fg(rgb(t.error)),
        }
    }
}

/// What a screen wants after a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

/// A screen of the terminal UI.
pub trait Screen {
    fn draw(&mut self, frame: &mut Frame, palette: &Palette);
    fn key(&mut self, key: KeyEvent) -> Flow;
    /// Called about ten times a second, for work in the background.
    fn tick(&mut self) -> Flow {
        Flow::Continue
    }
}

/// Runs `screen` full-screen until it quits. The terminal is put back as
/// it was afterwards, also after a panic (ratatui's `init` installs a hook
/// for that).
pub fn run(screen: &mut dyn Screen, palette: &Palette) -> std::io::Result<()> {
    let mut terminal = ratatui::try_init()?;
    let result = (|| loop {
        terminal.draw(|frame| screen.draw(frame, palette))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                // Windows also reports releases; only presses count.
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let quit = key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
                    if quit || screen.key(key) == Flow::Quit {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
        if screen.tick() == Flow::Quit {
            terminal.draw(|frame| screen.draw(frame, palette))?;
            return Ok(());
        }
    })();
    ratatui::try_restore()?;
    result
}

/// The frame of a screen: the background, a border with `title`, and a key
/// bar at the bottom. Returns the area inside.
pub fn chrome(frame: &mut Frame, palette: &Palette, title: &str, keys: &[(&str, &str)]) -> Rect {
    let area = frame.area();
    frame.render_widget(Block::default().style(palette.base), area);
    let [body, bar] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(palette.border)
        .title(Span::styled(format!(" {title} "), palette.accent))
        .style(palette.base);
    let inner = block.inner(body);
    frame.render_widget(block, body);
    frame.render_widget(Paragraph::new(key_bar(keys, palette)).style(palette.base), bar);
    inner
}

/// `Enter update · q quit`, the keys in the accent colour.
pub fn key_bar<'a>(keys: &[(&'a str, &'a str)], palette: &Palette) -> Line<'a> {
    let mut spans = vec![Span::raw(" ")];
    for (i, (key, what)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", palette.muted));
        }
        spans.push(Span::styled(*key, palette.accent));
        spans.push(Span::styled(format!(" {what}"), palette.muted));
    }
    Line::from(spans)
}

/// A box of `width` by `height` in the middle of `area`.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(height.min(area.height))]).flex(Flex::Center).areas(area);
    let [cell] = Layout::horizontal([Constraint::Length(width.min(area.width))]).flex(Flex::Center).areas(row);
    cell
}

/// A one-line text field: typing, Backspace, Left and Right.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Input {
    pub text: String,
    /// The cursor, in characters.
    pub cursor: usize,
}

impl Input {
    /// Handles an editing key; returns false for keys it does not use.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let chars: Vec<char> = self.text.chars().collect();
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                let mut chars = chars;
                chars.insert(self.cursor, c);
                self.text = chars.into_iter().collect();
                self.cursor += 1;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let mut chars = chars;
                chars.remove(self.cursor - 1);
                self.text = chars.into_iter().collect();
                self.cursor -= 1;
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(chars.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = chars.len(),
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! Drawing a screen on a test terminal and comparing it with its
    //! snapshot. `UPDATE_TUI=1 cargo test --lib tui` writes the snapshots
    //! again; read the diff before committing them.
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    pub fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    pub fn type_text(screen: &mut dyn Screen, text: &str) {
        for c in text.chars() {
            screen.key(press(KeyCode::Char(c)));
        }
    }

    /// The screen as text, one line per row, trailing spaces removed.
    pub fn render(screen: &mut dyn Screen, palette: &Palette, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| screen.draw(frame, palette)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..height {
            let mut line = String::new();
            for x in 0..width {
                line.push_str(buffer[(x, y)].symbol());
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }

    /// Compares the screen with `src/tui/snapshots/<name>.txt`.
    pub fn snapshot(name: &str, screen: &mut dyn Screen) {
        let text = render(screen, &Palette::of(Choice::Auto), 80, 24);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui/snapshots").join(format!("{name}.txt"));
        if std::env::var_os("UPDATE_TUI").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &text).unwrap();
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
        assert_eq!(text, expected, "the screen differs from {}; UPDATE_TUI=1 writes it again", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    #[test]
    fn palettes_follow_the_theme_tokens() {
        let dark = Palette::of(Choice::Dark);
        assert_eq!(dark.base.bg, Some(rgb(theme::DARK.background)));
        assert_eq!(dark.accepting.fg, Some(rgb(theme::DARK.accepting)));
        assert_eq!(rgb("#0a1b2c"), Color::Rgb(0x0a, 0x1b, 0x2c));
        // Auto keeps the terminal's own colours.
        assert_eq!(Palette::of(Choice::Auto).base, Style::default());
    }

    #[test]
    fn the_input_edits_text() {
        let mut input = Input::default();
        for c in "0110".chars() {
            input.key(testing::press(KeyCode::Char(c)));
        }
        input.key(testing::press(KeyCode::Left));
        input.key(testing::press(KeyCode::Backspace));
        assert_eq!(input.text, "010");
        assert_eq!(input.cursor, 2);
        assert!(!input.key(testing::press(KeyCode::Enter)));
    }
}
