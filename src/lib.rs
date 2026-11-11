//! Kleene Star: a small language for Theory of Computation.
//!
//! * `automata` is a plain Rust library of machines and algorithms.
//! * `lang` is the language built on top of it.
//! * `Session` (below) ties the stages of `lang` together.
//! * `ide` answers editor questions; the `kstar-lsp` server is built on it.

pub mod automata;
pub mod ide;
pub mod lang;

use lang::ast::Stmt;
use lang::check::Checker;
use lang::error::{Diag, Paint};
use lang::interp::Interp;
use lang::lexer::lex;
use lang::parser::Parser;
use std::io::Write;
use std::path::PathBuf;

/// At most this many errors are shown for one run; the rest are counted.
const MAX_ERRORS_SHOWN: usize = 20;

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
    color: bool,
    /// Warnings from the last `check`, rendered, waiting to be shown.
    warnings: Vec<String>,
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
            color: false,
            warnings: Vec::new(),
        }
    }

    /// Shows errors and warnings in colour (for a terminal).
    pub fn color(&mut self, enabled: bool) {
        self.color = enabled;
    }

    /// Makes every function call and return print a line, with its
    /// arguments and result.
    pub fn trace_calls(&mut self, enabled: bool) {
        self.interp.trace_calls = enabled;
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

    /// Checks and runs `text`. On failure returns the formatted errors.
    /// Warnings are kept for `take_warnings`.
    pub fn run(&mut self, text: &str) -> Result<(), String> {
        let program = self.check(text)?;
        self.interp
            .run(&program, &self.file, &self.src)
            .map_err(|diag| diag.render_styled(&self.file, &self.src, self.color))
    }

    /// Lexes, parses and checks `text` without running it. On failure
    /// returns every error found, formatted.
    pub fn check(&mut self, text: &str) -> Result<Vec<Stmt>, String> {
        let from = self.src.len();
        let first_line = self.src.matches('\n').count() + 1;
        self.src.push_str(text);
        self.src.push('\n');

        // Check on a copy, so a rejected entry leaves no trace behind.
        let mut checker = self.checker.clone();
        let checked = lang::check_source(&self.src, from, first_line, self.repl, &mut checker);
        self.warnings = checked.warnings.iter().map(|w| w.render_styled(&self.file, &self.src, self.color)).collect();
        if checked.errors.is_empty() {
            self.checker = checker;
            Ok(checked.program)
        } else {
            Err(self.render_errors(&checked.errors))
        }
    }

    /// The errors, one after another with a blank line between, and a
    /// count at the end when there is more than one.
    fn render_errors(&self, errors: &[Diag]) -> String {
        let shown: Vec<String> = errors
            .iter()
            .take(MAX_ERRORS_SHOWN)
            .map(|e| e.render_styled(&self.file, &self.src, self.color))
            .collect();
        let mut out = shown.join("\n\n");
        if errors.len() > 1 {
            let more = errors.len().saturating_sub(MAX_ERRORS_SHOWN);
            let count = format!("{} errors found", errors.len());
            out.push_str("\n\n");
            out.push_str(&Paint(self.color).red(&count));
            if more > 0 {
                out.push_str(&format!(" (the last {more} are not shown)"));
            }
        }
        out
    }

    /// The warnings found by the last `check` or `run`, formatted, leaving
    /// none behind.
    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    /// The type of the expression `text`, for the REPL's `:type`; nothing
    /// is defined or run.
    pub fn type_of(&mut self, text: &str) -> Result<String, String> {
        let mut src = self.src.clone();
        let from = src.len();
        let first_line = src.matches('\n').count() + 1;
        src.push_str(text);
        src.push('\n');
        let mut checker = self.checker.clone();
        let result = lex(&src, from, first_line).and_then(|tokens| {
            let mut parser = Parser::new(tokens, &src, true);
            let expr = parser.expr()?;
            parser.finish()?;
            checker.type_of(&expr)
        });
        result.map(|ty| ty.to_string()).map_err(|diag| diag.render_styled("<repl>", &src, self.color))
    }

    /// Every name defined at the top level so far, for completion.
    pub fn names(&self) -> Vec<String> {
        self.checker.global_names()
    }

    /// How many `test` statements passed and failed so far.
    pub fn tests(&self) -> (usize, usize) {
        (self.interp.passed, self.interp.failed)
    }
}

/// Rewrites a program in the canonical style, as `kstar fmt` does. Windows
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

/// The stack size for threads that run Kleene Star code: 256 MB, of which
/// only what is used is ever committed.
pub const STACK_SIZE: usize = 256 * 1024 * 1024;

/// Runs `work` on a thread with a large stack. Every stage is recursive: the
/// parser, checker and formatter over the nesting of the program (which
/// the parser limits to `lang::parser::MAX_NESTING` levels), and the
/// interpreter over calls (up to 500 deep). A default thread has too little
/// stack for the deepest programs allowed, especially in a debug build.
pub fn with_big_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(work)
        .expect("could not start the interpreter thread")
        .join()
        .expect("the interpreter thread panicked")
}

/// The program `kstar new` writes: a short, working example to change.
/// `name` titles it and `file` is the path to run it by.
pub fn starter_program(name: &str, file: &str) -> String {
    format!(
        r#"# {name}: a Kleene Star program. Run it with:  kstar run {file}

alphabet B = {{0, 1}}

# The words that end in 1.
dfa EndsIn1 over B {{
    start  q0
    accept q1
    q0 --0--> q0
    q0 --1--> q1
    q1 --0--> q0
    q1 --1--> q1
}}

print EndsIn1
test EndsIn1 accepts "0101"
test not (EndsIn1 accepts "10")
test equivalent(EndsIn1, /(0|1)*1/)
"#
    )
}
