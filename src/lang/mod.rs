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
pub mod parser;
pub mod source;
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

/// Lexes, parses and checks the text of `src` (the text of file `file`)
/// from byte `from` (on line `first_line`) onwards, reporting every error
/// rather than only the first.
///
/// Each stage carries on past its errors: the lexer skips what it cannot
/// read, the parser skips to the next statement, and the checker checks the
/// statements that did parse. A name defined by a statement that did not
/// parse would then look undefined, so after a syntax error, "not defined"
/// errors are left out: they are most likely consequences of it.
pub fn check_source(
    file: token::FileId,
    src: &str,
    from: usize,
    first_line: u32,
    repl: bool,
    checker: &mut Checker,
) -> Checked {
    let (tokens, mut errors) = lexer::lex_all_in(file, src, from, first_line);
    let (program, parse_errors) = parser::Parser::new(tokens, src, repl).program_recovering();
    errors.extend(parse_errors);
    let syntax_ok = errors.is_empty();
    let mut check_errors = checker.check_all(&program);
    if !syntax_ok {
        check_errors.retain(|e| e.code != codes::UNDEFINED && e.code != codes::UNKNOWN_FUNCTION);
    }
    errors.extend(check_errors);
    errors.sort_by_key(|e| e.span.start);
    let warnings = if errors.is_empty() { checker.warnings(&program) } else { Vec::new() };
    Checked { program, errors, warnings }
}
