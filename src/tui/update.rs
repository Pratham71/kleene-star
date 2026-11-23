//! The screen of `kstar update` on a terminal: the installed version, the
//! latest stable and beta releases, what is new in the chosen one, a
//! question, then the install step by step with its output.
//!
//! The same can be done without the screen: `kstar update --check`,
//! `--beta`, `--yes`, `--no-vscode` and `--no-tui`.

use super::{centered, chrome, Flow, Palette, Screen};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;
use std::sync::mpsc::Receiver;

/// A release: its tag (`v0.8.0`) and its notes (Markdown).
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub tag: String,
    pub notes: String,
}

/// A version as numbers, `0.2.0` -> [0, 2, 0], ignoring a leading `v` and
/// anything after a `-` (a pre-release sorts before its release).
fn parse(version: &str) -> Option<(Vec<u64>, bool)> {
    let version = version.trim().trim_start_matches('v');
    let (core, pre) = match version.split_once('-') {
        Some((core, _)) => (core, true),
        None => (version, false),
    };
    let numbers = core.split('.').map(|n| n.parse().ok()).collect::<Option<Vec<u64>>>()?;
    Some((numbers, pre))
}

/// True if `candidate` is a later version than `current`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse(candidate), parse(current)) {
        (Some((a, a_pre)), Some((b, b_pre))) => a > b || (a == b && b_pre && !a_pre),
        _ => false,
    }
}

/// Stable releases, or the betas (pre-releases) too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
}

/// The release to install on a channel: on beta, the latest beta if it is
/// newer than the latest stable release, otherwise that.
pub fn target<'a>(channel: Channel, stable: Option<&'a Release>, beta: Option<&'a Release>) -> Option<&'a Release> {
    match (channel, stable, beta) {
        (Channel::Beta, Some(s), Some(b)) if is_newer(&b.tag, &s.tag) => Some(b),
        (Channel::Beta, None, Some(b)) => Some(b),
        (_, s, _) => s,
    }
}

/// The steps of an install, in order.
pub const STEPS: [&str; 4] =
    ["Set the old programs aside", "Download and install the release", "Check the new programs", "The VS Code extension"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Waiting,
    Running,
    Done,
    Skipped,
    Failed,
}

/// What an install reports as it goes.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    Step(usize, StepState),
    /// A line of the install script's output.
    Log(String),
    /// The end: a message saying what happened, or why it failed.
    Finished(Result<String, String>),
}

/// Starts installing a release in the background.
pub trait Installer {
    fn start(&mut self, tag: &str) -> Receiver<Progress>;
}

/// How the screen ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Quit without installing anything.
    Cancelled,
    Updated(String),
    Failed(String),
}

enum Phase {
    Choose,
    Confirm,
    Installing(Receiver<Progress>),
    Done,
}

pub struct UpdateScreen {
    current: String,
    stable: Option<Release>,
    beta: Option<Release>,
    channel: Channel,
    scroll: u16,
    phase: Phase,
    steps: [StepState; 4],
    log: Vec<String>,
    ticks: usize,
    installer: Box<dyn Installer>,
    pub outcome: Outcome,
}

impl UpdateScreen {
    /// `current` is this kstar's version, without a `v`.
    pub fn new(current: &str, stable: Option<Release>, beta: Option<Release>, channel: Channel, installer: Box<dyn Installer>) -> UpdateScreen {
        UpdateScreen {
            current: current.to_string(),
            stable,
            beta,
            channel,
            scroll: 0,
            phase: Phase::Choose,
            steps: [StepState::Waiting; 4],
            log: Vec::new(),
            ticks: 0,
            installer,
            outcome: Outcome::Cancelled,
        }
    }

    fn target(&self) -> Option<&Release> {
        target(self.channel, self.stable.as_ref(), self.beta.as_ref())
    }

    /// The release to update to, if it is newer than this kstar.
    fn offer(&self) -> Option<&Release> {
        self.target().filter(|r| is_newer(&r.tag, &self.current))
    }

    fn draw_choose(&self, frame: &mut Frame, p: &Palette) {
        let keys: &[(&str, &str)] = if self.offer().is_some() {
            &[("Enter", "update"), ("←/→", "stable or beta"), ("↑/↓", "scroll"), ("q", "quit")]
        } else {
            &[("←/→", "stable or beta"), ("↑/↓", "scroll"), ("q", "quit")]
        };
        let inner = chrome(frame, p, "kstar update", keys);
        let [facts, _, notes] = Layout::vertical([Constraint::Length(5), Constraint::Length(1), Constraint::Min(3)]).areas(inner);
        let release_line = |label: &'static str, release: Option<&Release>| {
            let mut spans = vec![Span::styled(format!(" {label:<11}"), p.muted)];
            match release {
                Some(r) => {
                    spans.push(Span::styled(r.tag.clone(), p.base));
                    if is_newer(&r.tag, &self.current) {
                        spans.push(Span::styled("  newer", p.accepting));
                    }
                }
                None => spans.push(Span::styled("none", p.muted)),
            }
            Line::from(spans)
        };
        let choice = |channel: Channel, name: &'static str| {
            if self.channel == channel {
                Span::styled(format!(" ● {name} "), p.selected)
            } else {
                Span::styled(format!(" ○ {name} "), p.base)
            }
        };
        let lines = vec![
            Line::from(vec![Span::styled(" Installed  ", p.muted), Span::styled(format!("v{}", self.current), p.base)]),
            release_line("Stable", self.stable.as_ref()),
            release_line("Beta", self.beta.as_ref()),
            Line::raw(""),
            Line::from(vec![Span::styled(" Channel    ", p.muted), choice(Channel::Stable, "Stable"), Span::raw("  "), choice(Channel::Beta, "Beta")]),
        ];
        frame.render_widget(Paragraph::new(lines).style(p.base), facts);

        let (title, text) = match (self.offer(), self.target()) {
            (Some(r), _) => (format!(" What's new in {} ", r.tag), notes_lines(&r.notes, p)),
            (None, Some(r)) => (
                " Up to date ".to_string(),
                vec![Line::styled(format!("kstar v{} is up to date: the latest {} release is {}.", self.current, channel_name(self.channel), r.tag), p.base)],
            ),
            (None, None) => (" No releases ".to_string(), vec![Line::styled("GitHub lists no release on this channel.", p.base)]),
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(p.border)
            .title(Span::styled(title, p.accent))
            .style(p.base);
        frame.render_widget(Paragraph::new(text).block(block).wrap(Wrap { trim: false }).scroll((self.scroll, 0)), notes);
    }

    fn draw_confirm(&self, frame: &mut Frame, p: &Palette) {
        let Some(r) = self.offer() else { return };
        let area = centered(frame.area(), 54, 7);
        frame.render_widget(Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(p.accent)
            .title(Span::styled(" Update? ", p.accent))
            .style(p.base);
        let lines = vec![
            Line::raw(""),
            Line::styled(format!(" Update kstar v{} to {}?", self.current, r.tag), p.base),
            Line::raw(""),
            Line::from(vec![Span::raw(" "), Span::styled("y", p.accent), Span::styled(" or Enter: update   ", p.muted), Span::styled("n", p.accent), Span::styled(" or Esc: not now", p.muted)]),
        ];
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn draw_progress(&self, frame: &mut Frame, p: &Palette) {
        let done = matches!(self.phase, Phase::Done);
        let keys: &[(&str, &str)] = if done { &[("any key", "close")] } else { &[("Installing", "please wait")] };
        let tag = self.offer().map(|r| r.tag.clone()).unwrap_or_default();
        let inner = chrome(frame, p, &format!("kstar update to {tag}"), keys);
        let [steps, gauge, _, log, result] = Layout::vertical([
            Constraint::Length(STEPS.len() as u16),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .areas(inner);
        let spinner = ["|", "/", "-", "\\"][self.ticks % 4];
        let lines: Vec<Line> = STEPS
            .iter()
            .zip(self.steps)
            .map(|(name, state)| {
                let (mark, style) = match state {
                    StepState::Waiting => ("   ".to_string(), p.muted),
                    StepState::Running => (format!(" {spinner} "), p.current),
                    StepState::Done => (" ✓ ".to_string(), p.accepting),
                    StepState::Skipped => (" - ".to_string(), p.muted),
                    StepState::Failed => (" ✗ ".to_string(), p.error),
                };
                let label = if state == StepState::Skipped { format!("{name} (skipped)") } else { name.to_string() };
                Line::from(vec![Span::styled(mark, style), Span::styled(label, if state == StepState::Waiting { p.muted } else { p.base })])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines).style(p.base), steps);
        let finished = self.steps.iter().filter(|s| matches!(s, StepState::Done | StepState::Skipped)).count();
        frame.render_widget(
            Gauge::default()
                .ratio(finished as f64 / STEPS.len() as f64)
                .label(format!("{finished} of {}", STEPS.len()))
                .gauge_style(p.accent)
                .style(p.base),
            gauge,
        );
        let block = Block::default().borders(Borders::TOP).border_style(p.border).title(Span::styled(" Output ", p.muted)).style(p.base);
        let room = log.height.saturating_sub(1) as usize;
        let shown: Vec<Line> = self.log.iter().skip(self.log.len().saturating_sub(room)).map(|l| Line::styled(l.clone(), p.muted)).collect();
        frame.render_widget(Paragraph::new(shown).block(block), log);
        let message = match &self.outcome {
            Outcome::Updated(m) => Line::styled(format!(" ✓ {m}"), p.accepting),
            Outcome::Failed(m) => Line::styled(format!(" ✗ {m}"), p.error),
            Outcome::Cancelled => Line::raw(""),
        };
        frame.render_widget(Paragraph::new(message).wrap(Wrap { trim: true }), result);
    }
}

fn channel_name(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable => "stable",
        Channel::Beta => "beta",
    }
}

/// Release notes as lines: headings without their `#`, in the accent
/// colour; everything else as written.
fn notes_lines(notes: &str, p: &Palette) -> Vec<Line<'static>> {
    notes
        .replace("\r\n", "\n")
        .lines()
        .map(|line| match line.strip_prefix('#') {
            Some(heading) => Line::styled(heading.trim_start_matches('#').trim().to_string(), p.accent),
            None => Line::styled(line.to_string(), p.base),
        })
        .collect()
}

impl Screen for UpdateScreen {
    fn draw(&mut self, frame: &mut Frame, palette: &Palette) {
        match self.phase {
            Phase::Choose => self.draw_choose(frame, palette),
            Phase::Confirm => {
                self.draw_choose(frame, palette);
                self.draw_confirm(frame, palette);
            }
            Phase::Installing(_) | Phase::Done => self.draw_progress(frame, palette),
        }
    }

    fn key(&mut self, key: KeyEvent) -> Flow {
        match self.phase {
            Phase::Choose => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Flow::Quit,
                KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::Char('s') | KeyCode::Char('b') => {
                    self.channel = match (key.code, self.channel) {
                        (KeyCode::Char('s'), _) => Channel::Stable,
                        (KeyCode::Char('b'), _) => Channel::Beta,
                        (_, Channel::Stable) => Channel::Beta,
                        (_, Channel::Beta) => Channel::Stable,
                    };
                    self.scroll = 0;
                }
                KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
                KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
                KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
                KeyCode::PageDown => self.scroll = self.scroll.saturating_add(10),
                KeyCode::Enter if self.offer().is_some() => self.phase = Phase::Confirm,
                _ => {}
            },
            Phase::Confirm => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    let tag = self.offer().map(|r| r.tag.clone()).unwrap_or_default();
                    self.phase = Phase::Installing(self.installer.start(&tag));
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.phase = Phase::Choose,
                _ => {}
            },
            Phase::Installing(_) => {}
            Phase::Done => return Flow::Quit,
        }
        Flow::Continue
    }

    fn tick(&mut self) -> Flow {
        self.ticks += 1;
        let Phase::Installing(events) = &self.phase else { return Flow::Continue };
        let mut finished = None;
        while let Ok(event) = events.try_recv() {
            match event {
                Progress::Step(i, state) => {
                    if let Some(step) = self.steps.get_mut(i) {
                        *step = state;
                    }
                }
                Progress::Log(line) => self.log.push(line),
                Progress::Finished(result) => finished = Some(result),
            }
        }
        if let Some(result) = finished {
            self.outcome = match result {
                Ok(message) => Outcome::Updated(message),
                Err(message) => Outcome::Failed(message),
            };
            self.phase = Phase::Done;
        }
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{press, snapshot};
    use super::*;
    use std::sync::mpsc;

    /// Installs nothing: plays back a fixed list of progress events.
    struct Recorded(Vec<Progress>, std::rc::Rc<std::cell::RefCell<Vec<String>>>);

    impl Installer for Recorded {
        fn start(&mut self, tag: &str) -> Receiver<Progress> {
            self.1.borrow_mut().push(tag.to_string());
            let (send, receive) = mpsc::channel();
            for event in self.0.drain(..) {
                send.send(event).unwrap();
            }
            receive
        }
    }

    fn release(tag: &str, notes: &str) -> Release {
        Release { tag: tag.into(), notes: notes.into() }
    }

    fn screen(events: Vec<Progress>) -> (UpdateScreen, std::rc::Rc<std::cell::RefCell<Vec<String>>>) {
        let started = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let stable = release("v0.8.0", "Help, man pages, themes and this screen.\n\n### Added\n\n- `kstar tui`\n- themes");
        let beta = release("v0.9.0-beta", "Lists.");
        let s = UpdateScreen::new("0.7.0", Some(stable), Some(beta), Channel::Stable, Box::new(Recorded(events, started.clone())));
        (s, started)
    }

    #[test]
    fn versions_and_channels() {
        assert!(is_newer("v0.8.0", "0.7.0"));
        assert!(is_newer("0.8.0", "0.8.0-beta"));
        assert!(!is_newer("v0.8.0-beta", "0.8.0"));
        let (s, b) = (release("v0.8.0", ""), release("v0.8.1-beta", ""));
        assert_eq!(target(Channel::Stable, Some(&s), Some(&b)), Some(&s));
        assert_eq!(target(Channel::Beta, Some(&s), Some(&b)), Some(&b));
        let old_beta = release("v0.8.0-beta", "");
        assert_eq!(target(Channel::Beta, Some(&s), Some(&old_beta)), Some(&s));
    }

    #[test]
    fn choosing_a_release() {
        let (mut s, _) = screen(Vec::new());
        snapshot("update_stable", &mut s);
        s.key(press(KeyCode::Right));
        snapshot("update_beta", &mut s);
        s.key(press(KeyCode::Char('s')));
        s.key(press(KeyCode::Enter));
        snapshot("update_confirm", &mut s);
        assert_eq!(s.key(press(KeyCode::Esc)), Flow::Continue);
        assert_eq!(s.key(press(KeyCode::Char('q'))), Flow::Quit);
        assert_eq!(s.outcome, Outcome::Cancelled);
    }

    #[test]
    fn up_to_date() {
        let installer = Recorded(Vec::new(), Default::default());
        let mut s = UpdateScreen::new("0.8.0", Some(release("v0.8.0", "notes")), None, Channel::Stable, Box::new(installer));
        snapshot("update_up_to_date", &mut s);
        // Enter does nothing when there is nothing newer.
        s.key(press(KeyCode::Enter));
        assert!(matches!(s.phase, Phase::Choose));
    }

    #[test]
    fn installing_shows_each_step_and_the_output() {
        let events = vec![
            Progress::Step(0, StepState::Done),
            Progress::Step(1, StepState::Running),
            Progress::Log("Downloading kstar-x86_64-unknown-linux-gnu.tar.gz".into()),
            Progress::Log("Checksum verified.".into()),
        ];
        let (mut s, started) = screen(events);
        s.key(press(KeyCode::Enter));
        s.key(press(KeyCode::Char('y')));
        assert_eq!(*started.borrow(), vec!["v0.8.0".to_string()]);
        s.tick();
        snapshot("update_installing", &mut s);
    }

    #[test]
    fn the_end_of_an_install() {
        let events = vec![
            Progress::Step(0, StepState::Done),
            Progress::Step(1, StepState::Done),
            Progress::Step(2, StepState::Done),
            Progress::Step(3, StepState::Skipped),
            Progress::Finished(Ok("Updated kstar to v0.8.0.".into())),
        ];
        let (mut s, _) = screen(events);
        s.key(press(KeyCode::Enter));
        s.key(press(KeyCode::Enter));
        s.tick();
        snapshot("update_done", &mut s);
        assert_eq!(s.outcome, Outcome::Updated("Updated kstar to v0.8.0.".into()));
        assert_eq!(s.key(press(KeyCode::Char('x'))), Flow::Quit);
    }
}
