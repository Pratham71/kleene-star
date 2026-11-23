//! `kstar tui`: pick a `.kstar` file, then a machine in it, then what to
//! do: trace it on a word step by step, draw it, draw a run of it, or
//! export it. Or run the whole file. Each of these is also a command:
//! `kstar run`, `kstar trace`, `draw` in a program, `kstar export`.

use super::{chrome, Flow, Input, Palette, Screen};
use crate::lang::player::Play;
use crate::theme::Choice;
use crate::{Captured, Session};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;
use std::path::{Path, PathBuf};

/// A machine (or grammar) defined at the top level of a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    pub name: String,
    /// Its type: `Dfa`, `Nfa`, `Regex`, `Pda`, `Tm` or `Grammar`.
    pub kind: String,
}

/// What the home screen does to the world.
pub trait Backend {
    /// The `.kstar` files to choose from, as shown.
    fn files(&mut self) -> Vec<String>;
    fn machines(&mut self, file: &str) -> Result<Vec<Machine>, String>;
    /// What `kstar run <file>` prints.
    fn run(&mut self, file: &str) -> String;
    fn play(&mut self, file: &str, machine: &str, word: &str) -> Result<Play, String>;
    /// Draws the machine, or a run of it on `word`; says what was written.
    fn draw(&mut self, file: &str, machine: &str, word: Option<&str>) -> String;
    /// What `kstar export --format <format> <file> <machine>` says.
    fn export(&mut self, file: &str, machine: &str, format: &str) -> String;
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Action {
    Trace,
    Draw,
    DrawRun,
    /// A grammar's parse trees of a word.
    DrawTrees,
    Export(&'static str),
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Action::Trace => "Trace a word, step by step",
            Action::Draw => "Draw it (a .dot file, and .svg with Graphviz)",
            Action::DrawRun => "Draw a run (a page that plays it)",
            Action::DrawTrees => "Draw the parse trees of a word",
            Action::Export("rust") => "Export as a Rust function",
            Action::Export("python") => "Export as a Python function",
            Action::Export(_) => "Export as JSON",
        }
    }

    /// The actions that make sense for a kind of machine.
    fn for_kind(kind: &str) -> Vec<Action> {
        let regular = matches!(kind, "Dfa" | "Nfa" | "Regex");
        let mut actions = Vec::new();
        if kind != "Grammar" && kind != "Regex" {
            actions.push(Action::Trace);
        }
        actions.push(Action::Draw);
        match kind {
            "Regex" => {}
            "Grammar" => actions.push(Action::DrawTrees),
            _ => actions.push(Action::DrawRun),
        }
        if regular {
            actions.extend([Action::Export("rust"), Action::Export("python")]);
        }
        actions.push(Action::Export("json"));
        if kind == "Grammar" {
            // A grammar is drawn as the parse trees of a word only.
            actions.retain(|a| *a != Action::Draw);
        }
        actions
    }
}

enum View {
    Files { files: Vec<String>, state: ListState },
    Machines { file: String, machines: Vec<Machine>, state: ListState },
    Actions { file: String, machine: Machine, actions: Vec<Action>, state: ListState },
    Word { file: String, machine: String, action: Action, input: Input },
    Text { title: String, text: String, scroll: u16 },
    Trace { title: String, play: Box<Play>, at: usize },
}

pub struct HomeScreen {
    backend: Box<dyn Backend>,
    /// The views open, the last one shown; Esc goes back one.
    views: Vec<View>,
}

fn list_state() -> ListState {
    ListState::default().with_selected(Some(0))
}

impl HomeScreen {
    pub fn new(mut backend: Box<dyn Backend>) -> HomeScreen {
        let files = backend.files();
        HomeScreen { backend, views: vec![View::Files { files, state: list_state() }] }
    }

    fn open(&mut self, view: View) {
        self.views.push(view);
    }

    fn text(&mut self, title: String, text: String) {
        self.open(View::Text { title, text, scroll: 0 });
    }

    /// Enter on the view shown.
    fn choose(&mut self) {
        let Some(view) = self.views.last_mut() else { return };
        match view {
            View::Files { files, state } => {
                let Some(file) = state.selected().and_then(|i| files.get(i)).cloned() else { return };
                match self.backend.machines(&file) {
                    Ok(machines) => self.open(View::Machines { file, machines, state: list_state() }),
                    Err(message) => self.text(format!("{file} has errors"), message),
                }
            }
            View::Machines { file, machines, state } => {
                let file = file.clone();
                match state.selected() {
                    Some(0) | None => {
                        let output = self.backend.run(&file);
                        self.text(format!("kstar run {file}"), output);
                    }
                    Some(i) => {
                        let Some(machine) = machines.get(i - 1).cloned() else { return };
                        let actions = Action::for_kind(&machine.kind);
                        self.open(View::Actions { file, machine, actions, state: list_state() });
                    }
                }
            }
            View::Actions { file, machine, actions, state } => {
                let (file, name) = (file.clone(), machine.name.clone());
                let Some(action) = state.selected().and_then(|i| actions.get(i)).copied() else { return };
                match action {
                    Action::Trace | Action::DrawRun | Action::DrawTrees => {
                        self.open(View::Word { file, machine: name, action, input: Input::default() })
                    }
                    Action::Draw => {
                        let said = self.backend.draw(&file, &name, None);
                        self.text(format!("draw {name}"), said);
                    }
                    Action::Export(format) => {
                        let said = self.backend.export(&file, &name, format);
                        self.text(format!("kstar export --format {format} {file} {name}"), said);
                    }
                }
            }
            View::Word { file, machine, action, input } => {
                let (file, machine, action, word) = (file.clone(), machine.clone(), *action, input.text.clone());
                self.views.pop();
                if action == Action::Trace {
                    match self.backend.play(&file, &machine, &word) {
                        Ok(play) => {
                            let title = format!("{machine} on \"{word}\"");
                            self.open(View::Trace { title, play: Box::new(play), at: 0 })
                        }
                        Err(message) => self.text(format!("trace {machine}"), message),
                    }
                } else {
                    let said = self.backend.draw(&file, &machine, Some(&word));
                    self.text(format!("draw {machine} on \"{word}\""), said);
                }
            }
            View::Text { .. } | View::Trace { .. } => {}
        }
    }

    fn draw_list(frame: &mut Frame, p: &Palette, title: &str, hint: &str, items: Vec<ListItem>, state: &mut ListState, keys: &[(&str, &str)]) {
        let inner = chrome(frame, p, title, keys);
        let [hint_area, list] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(inner);
        frame.render_widget(Paragraph::new(Line::styled(format!(" {hint}"), p.muted)), hint_area);
        let list_widget = List::new(items).highlight_style(p.selected).highlight_symbol(" ▶ ").style(p.base);
        frame.render_stateful_widget(list_widget, list, state);
    }
}

/// The cells of the input: read symbols dimmed, the next one marked.
fn input_cells<'a>(word: &str, read: usize, p: &Palette) -> Vec<Span<'a>> {
    if word.is_empty() {
        return vec![Span::styled("the empty word", p.muted)];
    }
    word.chars()
        .enumerate()
        .map(|(k, c)| {
            if k < read {
                Span::styled(format!(" {c} "), p.muted)
            } else if k == read {
                Span::styled(format!("[{c}]"), p.current)
            } else {
                Span::styled(format!(" {c} "), p.base)
            }
        })
        .collect()
}

fn draw_trace(frame: &mut Frame, p: &Palette, title: &str, play: &Play, at: usize) {
    let inner = chrome(frame, p, title, &[("←/→", "step"), ("Home/End", "first, last"), ("Esc", "back")]);
    let f = &play.frames[at];
    let last = play.frames.len() - 1;
    let label = |text: &str| Span::styled(format!(" {text:<9}"), p.muted);
    let mut lines = vec![
        Line::from(vec![label("Step"), Span::styled(format!("{at} of {last}"), p.base), Span::styled(format!("   ({})", play.kind), p.muted)]),
        Line::raw(""),
    ];
    // The states, the ones the machine is in marked; accepting ones (( )).
    let mut states = vec![label("States")];
    for (s, name) in play.diagram.states.iter().enumerate() {
        let accepting = play.diagram.accept.contains(&s);
        let shown = if accepting { format!("(({name}))") } else { format!("({name})") };
        let style = if f.active.contains(&s) {
            p.current
        } else if accepting {
            p.accepting
        } else {
            p.base
        };
        states.push(Span::styled(if f.active.contains(&s) { format!("[{shown}]") } else { format!(" {shown} ") }, style));
        states.push(Span::raw(" "));
    }
    lines.push(Line::from(states));
    let names: Vec<&str> = f.active.iter().map(|&s| play.diagram.states[s].as_str()).collect();
    let now = if names.is_empty() { "no state".to_string() } else { names.join(", ") };
    lines.push(if at == 0 {
        Line::from(vec![label("Move"), Span::styled(format!("start in {now}"), p.base)])
    } else {
        Line::from(vec![label("Move"), Span::styled(f.label.clone(), p.taken), Span::styled(format!("  →  {now}"), p.base)])
    });
    if let Some(read) = f.read {
        let mut spans = vec![label("Input")];
        spans.extend(input_cells(&play.word, read, p));
        lines.push(Line::from(spans));
    }
    if let Some(stack) = &f.stack {
        let text = if stack.is_empty() { "empty".to_string() } else { stack.join(" ") };
        lines.push(Line::from(vec![label("Stack"), Span::styled(text, p.base), Span::styled("   (top first)", p.muted)]));
    }
    if let Some((cells, head)) = &f.tape {
        let mut spans = vec![label("Tape")];
        for (k, c) in cells.iter().enumerate() {
            spans.push(if k == *head { Span::styled(format!("[{c}]"), p.current) } else { Span::styled(format!(" {c} "), p.base) });
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::raw(""));
    if at == last {
        let style = if play.result == "ACCEPTED" { p.accepting } else if play.result == "REJECTED" { p.error } else { p.base };
        lines.push(Line::from(vec![label("Result"), Span::styled(play.result.clone(), style)]));
        lines.push(Line::from(vec![label(""), Span::styled(play.reason.clone(), p.muted)]));
    } else {
        lines.push(Line::from(vec![label("Result"), Span::styled("running…", p.muted)]));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).style(p.base), inner);
}

impl Screen for HomeScreen {
    fn draw(&mut self, frame: &mut Frame, p: &Palette) {
        let Some(view) = self.views.last_mut() else { return };
        let list_keys = [("↑/↓", "choose"), ("Enter", "open"), ("Esc", "back"), ("q", "quit")];
        match view {
            View::Files { files, state } => {
                let items: Vec<ListItem> = files.iter().map(|f| ListItem::new(f.as_str())).collect();
                let hint = if files.is_empty() { "No .kstar files here. Run kstar tui in a folder of programs." } else { "Choose a program." };
                Self::draw_list(frame, p, "Kleene Star", hint, items, state, &[("↑/↓", "choose"), ("Enter", "open"), ("q", "quit")]);
            }
            View::Machines { file, machines, state } => {
                let mut items = vec![ListItem::new(Line::styled("Run the whole file", p.base))];
                items.extend(machines.iter().map(|m| {
                    ListItem::new(Line::from(vec![Span::styled(format!("{:<20}", m.name), p.base), Span::styled(m.kind.clone(), p.muted)]))
                }));
                Self::draw_list(frame, p, file, "Run the file, or choose a machine.", items, state, &list_keys);
            }
            View::Actions { machine, actions, state, .. } => {
                let items: Vec<ListItem> = actions.iter().map(|a| ListItem::new(a.label())).collect();
                let title = format!("{} ({})", machine.name, machine.kind);
                Self::draw_list(frame, p, &title, "What to do with it?", items, state, &list_keys);
            }
            View::Word { machine, action, input, .. } => {
                let title = match action {
                    Action::Trace => format!("trace {machine}"),
                    Action::DrawTrees => format!("the parse trees of a word in {machine}"),
                    _ => format!("draw {machine} on a word"),
                };
                let inner = chrome(frame, p, &title, &[("Enter", "go"), ("Esc", "back")]);
                let lines = vec![
                    Line::styled(" The word (leave it empty for the empty word):", p.muted),
                    Line::raw(""),
                    Line::from(vec![Span::styled(" > ", p.accent), Span::styled(input.text.clone(), p.base)]),
                ];
                frame.render_widget(Paragraph::new(lines), inner);
                frame.set_cursor_position((inner.x + 3 + input.cursor as u16, inner.y + 2));
            }
            View::Text { title, text, scroll } => {
                let inner = chrome(frame, p, title, &[("↑/↓", "scroll"), ("Esc", "back"), ("q", "quit")]);
                let lines: Vec<Line> = text.lines().map(|l| Line::styled(l.to_string(), p.base)).collect();
                frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((*scroll, 0)), inner);
            }
            View::Trace { title, play, at } => draw_trace(frame, p, title, play, *at),
        }
    }

    fn key(&mut self, key: KeyEvent) -> Flow {
        let Some(view) = self.views.last_mut() else { return Flow::Quit };
        if let View::Word { input, .. } = view {
            match key.code {
                KeyCode::Enter => self.choose(),
                KeyCode::Esc => {
                    self.views.pop();
                }
                _ => {
                    input.key(key);
                }
            }
            return Flow::Continue;
        }
        match key.code {
            KeyCode::Char('q') => return Flow::Quit,
            KeyCode::Esc | KeyCode::Backspace => {
                if self.views.len() == 1 {
                    return Flow::Quit;
                }
                self.views.pop();
            }
            KeyCode::Enter => self.choose(),
            code => match view {
                View::Files { files, state } => move_selection(state, files.len(), code),
                View::Machines { machines, state, .. } => move_selection(state, machines.len() + 1, code),
                View::Actions { actions, state, .. } => move_selection(state, actions.len(), code),
                View::Text { scroll, .. } => match code {
                    KeyCode::Up => *scroll = scroll.saturating_sub(1),
                    KeyCode::Down => *scroll = scroll.saturating_add(1),
                    KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                    KeyCode::PageDown => *scroll = scroll.saturating_add(10),
                    _ => {}
                },
                View::Trace { play, at, .. } => {
                    let last = play.frames.len() - 1;
                    *at = match code {
                        KeyCode::Right | KeyCode::Char(' ') => (*at + 1).min(last),
                        KeyCode::Left => at.saturating_sub(1),
                        KeyCode::Home => 0,
                        KeyCode::End => last,
                        _ => *at,
                    };
                }
                View::Word { .. } => {}
            },
        }
        Flow::Continue
    }
}

fn move_selection(state: &mut ListState, len: usize, code: KeyCode) {
    if len == 0 {
        return;
    }
    let i = state.selected().unwrap_or(0);
    let next = match code {
        KeyCode::Down => (i + 1).min(len - 1),
        KeyCode::Up => i.saturating_sub(1),
        KeyCode::Home => 0,
        KeyCode::End => len - 1,
        _ => i,
    };
    state.select(Some(next));
}

/// The backend of the real `kstar tui`: runs and exports through the
/// `kstar` program itself (so they behave exactly as the commands do), and
/// lists, plays and draws machines in a session of its own.
pub struct Local {
    /// The `kstar` program.
    pub program: PathBuf,
    /// The folder the files are looked for in.
    pub root: PathBuf,
    pub theme: Choice,
}

/// The most files the home screen lists.
const MAX_FILES: usize = 500;

/// The `.kstar` files under `root`, three folders deep at most, without
/// hidden folders and build output, as paths relative to `root` with `/`.
pub fn kstar_files(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, depth: usize, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for path in entries {
            if out.len() >= MAX_FILES {
                return;
            }
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if path.is_dir() {
                if depth < 3 && !name.starts_with('.') && !matches!(name.as_str(), "target" | "node_modules") {
                    walk(&path, root, depth + 1, out);
                }
            } else if name.ends_with(".kstar") {
                let shown = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                out.push(shown);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, 0, &mut out);
    out
}

impl Local {
    fn path(&self, file: &str) -> PathBuf {
        self.root.join(file)
    }

    /// Output of the `kstar` program with `args`, both streams.
    fn kstar(&self, args: &[&str]) -> String {
        match std::process::Command::new(&self.program).args(args).current_dir(&self.root).env("NO_COLOR", "1").output() {
            Ok(out) => {
                let mut text = String::from_utf8_lossy(&out.stdout).to_string();
                text.push_str(&String::from_utf8_lossy(&out.stderr));
                text.replace("\r\n", "\n")
            }
            Err(e) => format!("could not run kstar: {e}"),
        }
    }
}

/// A session that has run `file` with its output thrown away (drawings go
/// to `out_dir`), and its text.
fn session_for(path: &Path, out_dir: PathBuf, theme: Choice) -> Result<Session, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?.replace("\r\n", "\n");
    let name = path.to_string_lossy().to_string();
    let mut session = Session::new(&name, Box::new(std::io::sink()), out_dir, true);
    session.set_path(path);
    session.theme(theme);
    session.run(&text)?;
    Ok(session)
}

fn quote(word: &str) -> String {
    word.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Somewhere to put the drawings of a file that is only being looked at.
fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join("kstar-tui")
}

impl Backend for Local {
    fn files(&mut self) -> Vec<String> {
        kstar_files(&self.root)
    }

    fn machines(&mut self, file: &str) -> Result<Vec<Machine>, String> {
        let (path, theme) = (self.path(file), self.theme);
        crate::with_big_stack(move || {
            let _ = std::fs::create_dir_all(scratch_dir());
            let mut session = session_for(&path, scratch_dir(), theme)?;
            let mut machines = Vec::new();
            for name in session.names() {
                if name.starts_with('_') {
                    continue;
                }
                if let Ok(kind) = session.type_of(&name) {
                    if matches!(kind.as_str(), "Dfa" | "Nfa" | "Regex" | "Pda" | "Tm" | "Grammar") {
                        machines.push(Machine { name, kind });
                    }
                }
            }
            Ok(machines)
        })
    }

    fn run(&mut self, file: &str) -> String {
        self.kstar(&["run", "--color", "never", "--theme", self.theme.name(), file])
    }

    fn play(&mut self, file: &str, machine: &str, word: &str) -> Result<Play, String> {
        let (path, theme, machine, word) = (self.path(file), self.theme, machine.to_string(), word.to_string());
        crate::with_big_stack(move || {
            let _ = std::fs::create_dir_all(scratch_dir());
            let mut session = session_for(&path, scratch_dir(), theme)?;
            session.run(&format!("let _kstar_tui = {machine}"))?;
            let value = session.value("_kstar_tui").ok_or_else(|| "the machine has no value".to_string())?;
            crate::lang::player::play(&value, &word).ok_or_else(|| format!("{machine} has no states to step through"))
        })
    }

    fn draw(&mut self, file: &str, machine: &str, word: Option<&str>) -> String {
        let path = self.path(file);
        let folder = path.parent().map(Path::to_path_buf).unwrap_or_else(|| self.root.clone());
        let statement = match word {
            Some(w) => format!("draw {machine} on \"{}\"", quote(w)),
            None => format!("draw {machine}"),
        };
        let theme = self.theme;
        crate::with_big_stack(move || {
            let result = session_for(&path, folder.clone(), theme).and_then(|mut session| {
                let said = Captured::default();
                session.render_svg(true);
                session.set_output(Box::new(said.clone()));
                session.run(&statement)?;
                Ok(said.text())
            });
            match result {
                Ok(said) => format!("In {}:\n{said}", folder.display()),
                Err(message) => message,
            }
        })
    }

    fn export(&mut self, file: &str, machine: &str, format: &str) -> String {
        self.kstar(&["export", "--color", "never", "--format", format, file, machine])
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{press, snapshot, type_text};
    use super::*;
    use crate::automata::dfa::tests::even_zeros;
    use crate::lang::value::Value;

    /// A backend with one file holding one DFA, which records what it was
    /// asked to do.
    #[derive(Default)]
    struct Fake(std::rc::Rc<std::cell::RefCell<Vec<String>>>);

    impl Backend for Fake {
        fn files(&mut self) -> Vec<String> {
            vec!["examples/01_dfa.kstar".into(), "labs/lab1.kstar".into()]
        }
        fn machines(&mut self, _: &str) -> Result<Vec<Machine>, String> {
            Ok(vec![Machine { name: "EvenZeros".into(), kind: "Dfa".into() }, Machine { name: "G".into(), kind: "Grammar".into() }])
        }
        fn run(&mut self, file: &str) -> String {
            self.0.borrow_mut().push(format!("run {file}"));
            "tests: 2 passed, 0 failed\n".into()
        }
        fn play(&mut self, _: &str, _: &str, word: &str) -> Result<Play, String> {
            Ok(crate::lang::player::play(&Value::dfa(even_zeros()), word).unwrap())
        }
        fn draw(&mut self, _: &str, machine: &str, word: Option<&str>) -> String {
            self.0.borrow_mut().push(format!("draw {machine} {word:?}"));
            format!("wrote {machine}.dot\n")
        }
        fn export(&mut self, file: &str, machine: &str, format: &str) -> String {
            self.0.borrow_mut().push(format!("export {format} {file} {machine}"));
            format!("Wrote /home/me/{machine}.rs (Rust, 40 lines)\n")
        }
    }

    fn home() -> (HomeScreen, std::rc::Rc<std::cell::RefCell<Vec<String>>>) {
        let fake = Fake::default();
        let calls = fake.0.clone();
        (HomeScreen::new(Box::new(fake)), calls)
    }

    #[test]
    fn files_then_machines_then_actions() {
        let (mut s, calls) = home();
        snapshot("home_files", &mut s);
        s.key(press(KeyCode::Enter));
        snapshot("home_machines", &mut s);
        s.key(press(KeyCode::Down));
        s.key(press(KeyCode::Enter));
        snapshot("home_actions", &mut s);
        // Export as Rust is the fourth action of a DFA.
        for _ in 0..3 {
            s.key(press(KeyCode::Down));
        }
        s.key(press(KeyCode::Enter));
        assert_eq!(calls.borrow().last().unwrap(), "export rust examples/01_dfa.kstar EvenZeros");
        snapshot("home_export", &mut s);
        // Esc goes back, all the way, then quits.
        for _ in 0..3 {
            assert_eq!(s.key(press(KeyCode::Esc)), Flow::Continue);
        }
        assert_eq!(s.key(press(KeyCode::Esc)), Flow::Quit);
    }

    #[test]
    fn running_the_whole_file() {
        let (mut s, calls) = home();
        s.key(press(KeyCode::Enter));
        s.key(press(KeyCode::Enter));
        assert_eq!(*calls.borrow(), vec!["run examples/01_dfa.kstar".to_string()]);
    }

    #[test]
    fn tracing_a_word_step_by_step() {
        let (mut s, _) = home();
        s.key(press(KeyCode::Enter));
        s.key(press(KeyCode::Down));
        s.key(press(KeyCode::Enter));
        s.key(press(KeyCode::Enter));
        type_text(&mut s, "010");
        snapshot("home_word", &mut s);
        s.key(press(KeyCode::Enter));
        snapshot("home_trace_start", &mut s);
        s.key(press(KeyCode::Right));
        snapshot("home_trace_step", &mut s);
        s.key(press(KeyCode::End));
        snapshot("home_trace_end", &mut s);
    }

    #[test]
    fn a_grammar_is_drawn_on_a_word() {
        let (mut s, calls) = home();
        s.key(press(KeyCode::Enter));
        s.key(press(KeyCode::End));
        s.key(press(KeyCode::Enter));
        // A grammar cannot be traced or drawn without a word.
        snapshot("home_grammar_actions", &mut s);
        s.key(press(KeyCode::Enter));
        type_text(&mut s, "ab");
        s.key(press(KeyCode::Enter));
        assert_eq!(calls.borrow().last().unwrap(), "draw G Some(\"ab\")");
    }

    #[test]
    fn finds_kstar_files() {
        let root = std::env::temp_dir().join("kstar-tui-files-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("labs/deep")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        for f in ["a.kstar", "labs/b.kstar", "labs/deep/c.kstar", ".git/d.kstar", "target/e.kstar", "notes.txt"] {
            std::fs::write(root.join(f), "").unwrap();
        }
        assert_eq!(kstar_files(&root), vec!["a.kstar", "labs/b.kstar", "labs/deep/c.kstar"]);
    }

    #[test]
    fn the_local_backend_lists_plays_and_draws() {
        let root = std::env::temp_dir().join("kstar-tui-local-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("m.kstar"),
            "alphabet B = {0, 1}\ndfa M over B {\n    start a\n    accept a\n    a --0--> b\n    a --1--> a\n    b --0--> a\n    b --1--> b\n}\nlet r = /0*/\nlet n = 3\n",
        )
        .unwrap();
        let mut local = Local { program: PathBuf::from("kstar"), root: root.clone(), theme: Choice::Auto };
        let names: Vec<String> = local.machines("m.kstar").unwrap().into_iter().map(|m| format!("{} {}", m.name, m.kind)).collect();
        assert_eq!(names, vec!["M Dfa", "r Regex"]);
        let play = local.play("m.kstar", "M", "00").unwrap();
        assert_eq!(play.frames.len(), 3);
        assert_eq!(play.result, "ACCEPTED");
        let said = local.draw("m.kstar", "M", Some("0"));
        assert!(said.contains("wrote M.html"), "{said}");
        assert!(root.join("M.html").is_file());
        assert!(local.play("m.kstar", "nope", "0").is_err());
    }
}
