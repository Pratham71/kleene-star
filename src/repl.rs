//! `kstar repl`: the interactive prompt.

use kstar::Session;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

const HELP: &str = "Type a statement or an expression and press Enter. Examples:
  alphabet B = {0, 1}
  let r = /(0|1)*01/
  r accepts \"1101\"
  minimize(r)
A line that opens a `{` continues until the matching `}`. Tab completes names.

Commands:
  :type <expr>      show the type of an expression, without running it
  :trace M <word>   step through machine M on a word (short for `trace M on \"word\"`)
  :load <file>      run a file here, so its definitions can be used
  :reset            forget everything defined so far
  :help             show this message
  :quit             leave (or press Ctrl-D)";

const COMMANDS: &[&str] = &[":type", ":trace", ":load", ":reset", ":help", ":quit"];

/// Completes keywords, built-in functions, the names defined so far, and
/// the prompt's own commands.
struct Completion {
    names: Rc<RefCell<Vec<String>>>,
}

impl Completer for Completion {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let before = &line[..pos];
        let start = before.rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':')).map_or(0, |i| i + 1);
        let word = &before[start..];
        if word.is_empty() {
            return Ok((pos, Vec::new()));
        }
        let names = self.names.borrow();
        let mut found: Vec<&str> = kstar::lang::token::Tok::KEYWORDS
            .iter()
            .chain(kstar::lang::builtins::NAMES)
            .chain(COMMANDS)
            .copied()
            .chain(names.iter().map(String::as_str))
            .filter(|c| c.starts_with(word))
            .collect();
        found.sort_unstable();
        found.dedup();
        let pairs = found.into_iter().map(|c| Pair { display: c.to_string(), replacement: c.to_string() }).collect();
        Ok((start, pairs))
    }
}

impl Hinter for Completion {
    type Hint = String;
}
impl Highlighter for Completion {}
impl Validator for Completion {}
impl Helper for Completion {}

/// Where the history of earlier sessions is kept: `~/.kstar_history`.
fn history_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".kstar_history"))
}

fn new_session(color: bool, trace_calls: bool) -> Session {
    let mut session = Session::new("<repl>", Box::new(std::io::stdout()), PathBuf::from("."), true);
    session.render_svg(true);
    session.ascii_only(crate::console_needs_ascii());
    session.color(color);
    session.trace_calls(trace_calls);
    session
}

pub fn repl(color: bool, trace_calls: bool) -> ExitCode {
    let mut editor: Editor<Completion, DefaultHistory> = match Editor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("kstar: cannot start the interactive prompt: {e}");
            return ExitCode::from(2);
        }
    };
    let names = Rc::new(RefCell::new(Vec::new()));
    editor.set_helper(Some(Completion { names: names.clone() }));
    let history = history_file();
    if let Some(path) = &history {
        let _ = editor.load_history(path);
    }

    println!("Kleene Star {}. Type :help for help, :quit to leave.", env!("CARGO_PKG_VERSION"));
    let mut session = new_session(color, trace_calls);
    let mut entry = String::new();
    loop {
        let prompt = if entry.is_empty() { "kstar> " } else { "...> " };
        let line = match editor.readline(prompt) {
            Ok(line) => line,
            // Ctrl-C abandons the entry being typed; Ctrl-D leaves.
            Err(rustyline::error::ReadlineError::Interrupted) => {
                entry.clear();
                continue;
            }
            Err(_) => break,
        };
        if entry.is_empty() && line.trim_start().starts_with(':') {
            let _ = editor.add_history_entry(line.as_str());
            let (command, argument) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
            let argument = argument.trim();
            match command {
                ":quit" | ":q" | ":exit" => break,
                ":help" | ":h" => println!("{HELP}"),
                ":reset" => {
                    session = new_session(color, trace_calls);
                    println!("everything defined so far is forgotten");
                }
                ":type" | ":t" if !argument.is_empty() => match session.type_of(argument) {
                    Ok(ty) => println!("{argument}: {ty}"),
                    Err(message) => eprintln!("{message}"),
                },
                ":load" | ":l" if !argument.is_empty() => match std::fs::read_to_string(argument) {
                    Ok(text) => {
                        report(&mut session, &text.replace("\r\n", "\n"));
                        println!("loaded {argument}");
                    }
                    Err(e) => eprintln!("kstar: cannot read {argument}: {e}"),
                },
                ":trace" if !argument.is_empty() => match argument.split_once(' ') {
                    Some((machine, word)) => {
                        let word = word.trim();
                        let quoted = if word.starts_with('"') { word.to_string() } else { format!("\"{word}\"") };
                        report(&mut session, &format!("trace {machine} on {quoted}"));
                    }
                    None => report(&mut session, &format!("trace {argument} on \"\"")),
                },
                _ => eprintln!("unknown command `{}`; type :help for the list", line.trim()),
            }
            *names.borrow_mut() = session.names();
            continue;
        }
        entry.push_str(&line);
        if kstar::is_incomplete(&entry) {
            entry.push('\n');
            continue;
        }
        if entry.trim().is_empty() {
            entry.clear();
            continue;
        }
        let _ = editor.add_history_entry(entry.as_str());
        report(&mut session, &entry);
        *names.borrow_mut() = session.names();
        entry.clear();
    }
    if let Some(path) = &history {
        let _ = editor.save_history(path);
    }
    ExitCode::SUCCESS
}

/// Runs an entry and shows its warnings and errors.
fn report(session: &mut Session, text: &str) {
    let result = session.run(text);
    for warning in session.take_warnings() {
        eprintln!("{warning}");
    }
    if let Err(message) = result {
        eprintln!("{message}");
    }
}
