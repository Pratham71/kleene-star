//! The `toc` command-line tool.

use std::path::PathBuf;
use std::process::ExitCode;
use toc::Session;

const USAGE: &str = "\
TOC-Lang: a small language for Theory of Computation

Usage:
  toc run <file.toc> [--out <dir>]   run a program
  toc check <file.toc>               look for errors without running
  toc repl                           try things out interactively
  toc <file.toc>                     same as `toc run`
  toc help                           show this message
  toc version                        show the version

`draw` writes .dot files into --out (default: the current folder).";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["help" | "--help" | "-h"] => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        ["version" | "--version" | "-V"] => {
            println!("toc {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        ["run", file] => run_file(file, ".", false),
        ["run", file, "--out", dir] => run_file(file, dir, false),
        ["check", file] => run_file(file, ".", true),
        ["repl"] => toc::with_big_stack(repl),
        [file] if file.ends_with(".toc") => run_file(file, ".", false),
        _ => {
            eprintln!("toc: unrecognised arguments\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Runs (or only checks) a file. Exit code 0 means no errors and no failed tests.
fn run_file(file: &str, out_dir: &str, check_only: bool) -> ExitCode {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("toc: cannot read {file}: {e}");
            return ExitCode::from(2);
        }
    };
    let shown_name = file.replace('\\', "/");
    let out_dir = PathBuf::from(out_dir);
    toc::with_big_stack(move || {
        let mut session = Session::new(&shown_name, Box::new(std::io::stdout()), out_dir, false);
        session.render_svg(true);
        let result = if check_only {
            session.check(&text).map(|_| println!("{shown_name}: no errors found"))
        } else {
            session.run(&text)
        };
        let (passed, failed) = session.tests();
        if passed + failed > 0 {
            println!("tests: {passed} passed, {failed} failed");
        }
        match result {
            Ok(()) if failed == 0 => ExitCode::SUCCESS,
            Ok(()) => ExitCode::FAILURE,
            Err(message) => {
                eprintln!("{message}");
                ExitCode::FAILURE
            }
        }
    })
}

const REPL_HELP: &str = "Type a statement or an expression and press Enter. Examples:
  alphabet B = {0, 1}
  let r = /(0|1)*01/
  r accepts \"1101\"
  minimize(r)
A line that opens a `{` continues until the matching `}`.
Commands:  :help  show this message      :quit  leave (or press Ctrl-D)";

/// The interactive prompt: read an entry, run it, print the result, repeat.
fn repl() -> ExitCode {
    let mut editor = match rustyline::DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("toc: cannot start the interactive prompt: {e}");
            return ExitCode::from(2);
        }
    };
    println!("TOC-Lang {}. Type :help for help, :quit to leave.", env!("CARGO_PKG_VERSION"));
    let mut session = Session::new("<repl>", Box::new(std::io::stdout()), PathBuf::from("."), true);
    session.render_svg(true);
    let mut entry = String::new();
    loop {
        let prompt = if entry.is_empty() { "toc> " } else { "...> " };
        match editor.readline(prompt) {
            Ok(line) => {
                if entry.is_empty() {
                    match line.trim() {
                        "" => continue,
                        ":quit" | ":q" | ":exit" => break,
                        ":help" | ":h" => {
                            println!("{REPL_HELP}");
                            continue;
                        }
                        _ => {}
                    }
                }
                entry.push_str(&line);
                if toc::is_incomplete(&entry) {
                    entry.push('\n');
                    continue;
                }
                let _ = editor.add_history_entry(entry.as_str());
                if let Err(message) = session.run(&entry) {
                    eprintln!("{message}");
                }
                entry.clear();
            }
            // Ctrl-C abandons the entry being typed; Ctrl-D leaves.
            Err(rustyline::error::ReadlineError::Interrupted) => entry.clear(),
            Err(_) => break,
        }
    }
    ExitCode::SUCCESS
}
