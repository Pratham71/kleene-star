//! The Kleene Star language: from source text to a running program.
//!
//! ```text
//! text --lexer--> tokens --parser--> syntax tree --checker--> (ok) --interpreter--> output
//! ```
//!
//! `check_source` runs the first three stages and collects every error and
//! warning; the interpreter only runs a program with no errors.

pub mod ast;
pub mod builtins;
pub mod check;
pub mod codes;
pub mod error;
pub mod format;
pub mod index;
pub mod interp;
pub mod lexer;
pub mod lint;
pub mod machine;
pub mod module;
pub mod parser;
pub mod source;
pub mod stdlib;
pub mod token;
pub mod trace;
pub mod value;

use ast::Stmt;
use check::Checker;
use error::Diag;

/// A program after lexing, parsing and checking.
pub struct Checked {
    /// The statements that parsed. Only safe to run if `errors` is empty.
    pub program: Vec<Stmt>,
    /// Every error found, in the order they appear in the source.
    pub errors: Vec<Diag>,
    /// Warnings, in source order. Only looked for when there are no errors.
    pub warnings: Vec<Diag>,
}

/// Lexes and parses the text of file `file` from byte `from` (on line
/// `first_line`) onwards, carrying on after errors: the lexer skips what it
/// cannot read and the parser skips to the next statement. Returns the
/// statements that parsed and every error.
pub fn parse_source(file: token::FileId, src: &str, from: usize, first_line: u32, repl: bool) -> (Vec<Stmt>, Vec<Diag>) {
    let (tokens, mut errors) = lexer::lex_all_in(file, src, from, first_line);
    let (program, parse_errors) = parser::Parser::new(tokens, src, repl).program_recovering();
    errors.extend(parse_errors);
    (program, errors)
}

/// Checks a parsed program and returns its errors in source order.
///
/// A name defined by a statement that did not parse would look undefined,
/// so when there were syntax errors (`syntax_ok` is false), "not defined"
/// errors are left out: they are most likely consequences of them.
pub fn check_parsed(program: &[Stmt], syntax_ok: bool, checker: &mut Checker) -> Vec<Diag> {
    let mut errors = checker.check_all(program);
    if !syntax_ok {
        errors.retain(|e| e.code != codes::UNDEFINED && e.code != codes::UNKNOWN_FUNCTION);
    }
    errors
}

/// Lexes, parses and checks the text of file `file` from byte `from` (on
/// line `first_line`) onwards, reporting every error rather than only the
/// first. The files it imports are loaded and checked by `loader` first;
/// their errors come before the file's own, each shown in its own file.
pub fn check_source(
    file: token::FileId,
    sources: &mut source::SourceMap,
    from: usize,
    first_line: u32,
    repl: bool,
    checker: &mut Checker,
    loader: &mut module::Loader,
) -> Checked {
    let (mut program, mut errors) = parse_source(file, sources.text(file), from, first_line, repl);
    let syntax_ok = errors.is_empty();
    errors.extend(loader.resolve(&mut program, file, sources));
    checker.set_modules(loader.exports.clone());
    errors.extend(check_parsed(&program, syntax_ok, checker));
    errors.sort_by_key(|e| e.span.start);
    let mut all = loader.errors_below(&program);
    all.extend(errors);
    let warnings = if all.is_empty() { checker.warnings(&program) } else { Vec::new() };
    Checked { program, errors: all, warnings }
}
