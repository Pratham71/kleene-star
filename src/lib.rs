//! TOC-Lang: a small language for Theory of Computation.
//!
//! * `automata` is a plain Rust library of machines and algorithms.
//! * `lang` is the language built on top of it.
//! * `Session` (below) ties the stages of `lang` together.

pub mod automata;
pub mod lang;

use lang::ast::Stmt;
use lang::check::Checker;
use lang::error::Diag;
use lang::interp::Interp;
use lang::lexer::lex;
use lang::parser::Parser;
use std::io::Write;
use std::path::PathBuf;

/// One program being run: a file, or a whole REPL conversation.
///
/// All the text ever fed in is kept in `src`, so an error can always quote
/// the line it is about, even one typed several REPL entries ago.
pub struct Session {
    file: String,
    src: String,
    checker: Checker,
    interp: Interp,
    repl: bool,
}

impl Session {
    /// `file` is the name shown in error messages; `out` receives everything
    /// the program prints; `out_dir` is where `draw` puts its files.
    pub fn new(file: &str, out: Box<dyn Write>, out_dir: PathBuf, repl: bool) -> Session {
        Session {
            file: file.to_string(),
            src: String::new(),
            checker: Checker::new(repl),
            interp: Interp::new(out, out_dir),
            repl,
        }
    }

    /// Lets `draw` also produce SVG files when Graphviz is installed.
    pub fn render_svg(&mut self, enabled: bool) {
        self.interp.render_svg = enabled;
    }

    /// Makes `trace` print `-` instead of an em dash, for consoles that
    /// would show the em dash as garbage.
    pub fn ascii_only(&mut self, enabled: bool) {
        self.interp.ascii = enabled;
    }

    /// Checks and runs `text`. On failure returns the formatted error.
    pub fn run(&mut self, text: &str) -> Result<(), String> {
        let program = self.check(text)?;
        self.interp
            .run(&program, &self.file, &self.src)
            .map_err(|diag| diag.render(&self.file, &self.src))
    }

    /// Lexes, parses and checks `text` without running it.
    pub fn check(&mut self, text: &str) -> Result<Vec<Stmt>, String> {
        let from = self.src.len();
        let first_line = self.src.matches('\n').count() + 1;
        self.src.push_str(text);
        self.src.push('\n');

        // Check on a copy, so a rejected entry leaves no trace behind.
        let mut checker = self.checker.clone();
        let result: Result<Vec<Stmt>, Diag> = lex(&self.src, from, first_line)
            .and_then(|tokens| Parser::new(tokens, &self.src, self.repl).program())
            .and_then(|program| checker.check(&program).map(|()| program));
        match result {
            Ok(program) => {
                self.checker = checker;
                Ok(program)
            }
            Err(diag) => Err(diag.render(&self.file, &self.src)),
        }
    }

    /// How many `test` statements passed and failed so far.
    pub fn tests(&self) -> (usize, usize) {
        (self.interp.passed, self.interp.failed)
    }
}

/// Rewrites a program in the canonical style, as `toc fmt` does. Windows
/// line endings in `text` are kept. On failure returns the formatted error
/// (a program must parse before it can be formatted).
pub fn format_program(file: &str, text: &str) -> Result<String, String> {
    let windows_endings = text.contains("\r\n");
    let src = text.replace("\r\n", "\n");
    let formatted = lang::format::format(&src).map_err(|diag| diag.render(file, &src))?;
    Ok(if windows_endings { formatted.replace('\n', "\r\n") } else { formatted })
}

/// True if `text` has an opening `{` that is not closed yet, which is how
/// the REPL knows to keep reading lines before running anything.
pub fn is_incomplete(text: &str) -> bool {
    use lang::token::Tok;
    let Ok(tokens) = lex(text, 0, 1) else { return false };
    let opened = tokens.iter().filter(|t| t.tok == Tok::LBrace).count();
    let closed = tokens.iter().filter(|t| t.tok == Tok::RBrace).count();
    opened > closed
}

/// Runs `work` on a thread with a large stack. The interpreter is recursive,
/// so deeply nested TOC calls need more stack than a default thread has.
pub fn with_big_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(work)
        .expect("could not start the interpreter thread")
        .join()
        .expect("the interpreter thread panicked")
}
