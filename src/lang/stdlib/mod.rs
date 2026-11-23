//! The standard library: modules that come with Kleene Star, reached as
//! `std.<module>.<name>` without an import, or imported like a file with
//! `import std.<module>` and `from std.<module> import <name>`.
//!
//! Everything here is a table. A module is a list of `Function`s, and each
//! function is one entry: its name, its parameter and result types (the
//! type rule the checker uses), a one-line description (which editors show
//! on hover, in completion and in signature help, and which the reference
//! must repeat word for word), and the Rust function that runs it. Nothing
//! else in the language knows about any particular library function:
//!
//! * the loader and the checker ask `exports` for a module's interface, the
//!   same `Exports` table a checked file produces, so `std.math` and
//!   `"utils.kstar"` are checked the same way;
//! * the interpreter asks `function` for the entry and calls its `run`.
//!
//! To add a module, see "Adding a standard-library module" in
//! `docs/internals.md`.

pub mod automata;
pub mod benchmark;
pub mod binary;
pub mod collections;
pub mod grammar;
pub mod io;
pub mod math;
pub mod pda;
pub mod random;
pub mod regex;
pub mod string;
pub mod testing;
pub mod tm;
pub mod visual;

use super::ast::Type;
use super::index::{Def, DefKind};
use super::module::{ExportedFn, Exports};
use super::token::{FileId, Span};
use super::value::Value;

/// A type, as a table can write it down. (`Type` itself holds a `Box` for
/// `Set[T]`, which a constant table cannot build.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Int,
    Bool,
    Str,
    Alphabet,
    Dfa,
    Nfa,
    Regex,
    Grammar,
    Pda,
    Tm,
    Set(&'static Ty),
    /// The element type `T` of a generic function: `Set(&Ty::Elem)` is a
    /// `Set[T]`. The checker fills it in from the arguments of each call.
    Elem,
    /// Any regular language: a `Dfa`, `Nfa`, `Regex` or `Set[Str]`. The
    /// function converts it with the helpers in `lang::langs`, so it picks
    /// the alphabet the way the built-ins do.
    Lang,
    /// Anything the `accepts` operator works on: a regular language, a
    /// `Grammar`, a `Pda` or a `Tm`. The function asks with `langs::accepts`.
    Acceptor,
}

impl Ty {
    /// The checker's version of this type.
    pub fn to_type(self) -> Type {
        match self {
            Ty::Int => Type::Int,
            Ty::Bool => Type::Bool,
            Ty::Str => Type::Str,
            Ty::Alphabet => Type::Alphabet,
            Ty::Dfa => Type::Dfa,
            Ty::Nfa => Type::Nfa,
            Ty::Regex => Type::Regex,
            Ty::Grammar => Type::Grammar,
            Ty::Pda => Type::Pda,
            Ty::Tm => Type::Tm,
            Ty::Set(inner) => Type::Set(Box::new(inner.to_type())),
            Ty::Elem => Type::Var,
            Ty::Lang => Type::Lang,
            Ty::Acceptor => Type::Acceptor,
        }
    }
}

/// Why a library function could not give an answer. The interpreter turns
/// it into a `MathError` (or whichever kind `code` belongs to) pointing at
/// argument `arg`, or at the whole call when `arg` is `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub code: &'static str,
    pub arg: Option<usize>,
    pub message: String,
    pub hint: String,
}

// ---- reading arguments ----
//
// A library function gets values the checker has already given the right
// types, so these only take them apart; any other value is a bug in the
// checker, not a mistake in the program.

/// Argument `i`, an `Int`.
pub fn int_arg(args: &[Value], i: usize) -> i64 {
    match &args[i] {
        Value::Int(n) => *n,
        other => unreachable!("the checker allowed {other:?} as an Int"),
    }
}

/// Argument `i`, a `Str`.
pub fn str_arg(args: &[Value], i: usize) -> &str {
    match &args[i] {
        Value::Str(s) => s,
        other => unreachable!("the checker allowed {other:?} as a Str"),
    }
}

/// Argument `i`, an `Alphabet`.
pub fn alphabet_arg(args: &[Value], i: usize) -> &[char] {
    match &args[i] {
        Value::Alphabet(symbols) => symbols,
        other => unreachable!("the checker allowed {other:?} as an Alphabet"),
    }
}

/// Checks that every symbol of `word`, argument `arg` of a call, is in
/// `alphabet`. `what` names the word in the message ("the pattern").
pub fn word_over(word: &str, alphabet: &[char], arg: usize, what: &str) -> Result<(), Failure> {
    match word.chars().find(|c| !alphabet.contains(c)) {
        Some(c) => Err(Failure {
            code: super::codes::FOREIGN_WORD,
            arg: Some(arg),
            message: format!("{what} contains `{c}`, which is not in the alphabet"),
            hint: format!("the alphabet is {}", crate::automata::fmt_alphabet(alphabet)),
        }),
        None => Ok(()),
    }
}

/// Languages that do not share an alphabet are a library failure at the
/// argument that does not fit, so `langs::to_dfas(args)?` works in a library
/// function.
impl From<super::langs::AlphabetProblem> for Failure {
    fn from(problem: super::langs::AlphabetProblem) -> Failure {
        Failure { code: problem.code, arg: Some(problem.arg), message: problem.message, hint: problem.hint }
    }
}

/// One library function.
pub struct Function {
    pub name: &'static str,
    /// Each parameter's name and type, in order.
    pub params: &'static [(&'static str, Ty)],
    pub ret: Ty,
    /// What it does, in one line.
    pub doc: &'static str,
    /// Runs it on arguments the checker has already given the right types.
    pub run: fn(&[Value]) -> Result<Value, Failure>,
}

impl Function {
    /// How the function is shown: `fn gcd(a: Int, b: Int) -> Int`.
    pub fn signature(&self) -> String {
        let params: Vec<String> = self.params.iter().map(|(name, ty)| format!("{name}: {}", ty.to_type())).collect();
        format!("fn {}({}) -> {}", self.name, params.join(", "), self.ret.to_type())
    }
}

/// One standard-library module.
pub struct Module {
    /// The name after `std.`.
    pub name: &'static str,
    /// One line about what the module is for.
    pub doc: &'static str,
    pub functions: &'static [Function],
    /// For modules of checks (`std.testing`): says why a call that returned
    /// `false` failed, given the function's name and the same arguments. A
    /// failing `test` statement prints it under the test's line.
    pub explain: Option<fn(&str, &[Value]) -> String>,
}

/// Every standard-library module, in alphabetical order.
pub const MODULES: &[Module] = &[automata::MODULE, benchmark::MODULE, binary::MODULE, collections::MODULE, grammar::MODULE, io::MODULE, math::MODULE, pda::MODULE, random::MODULE, regex::MODULE, string::MODULE, testing::MODULE, tm::MODULE, visual::MODULE];

/// The module called `name`, if there is one.
pub fn module(name: &str) -> Option<&'static Module> {
    MODULES.iter().find(|m| m.name == name)
}

/// The function `name` of module `module`.
pub fn function(module: &str, name: &str) -> Option<&'static Function> {
    self::module(module)?.functions.iter().find(|f| f.name == name)
}

/// What module `name` offers, for the loader and the checker: the same
/// table a checked file produces.
pub fn exports(name: &str) -> Option<Exports> {
    let module = module(name)?;
    let mut exports = Exports::default();
    for f in module.functions {
        let def = Def {
            name: f.name.to_string(),
            kind: DefKind::Function,
            ty: None,
            // The library has no source text, so editors cannot go there.
            span: Span { file: FileId::STD, ..Span::default() },
            visible: (usize::MAX, usize::MAX),
            detail: f.signature(),
            origin: None,
            doc: Some(f.doc.to_string()),
        };
        let params = f.params.iter().map(|(_, ty)| ty.to_type()).collect();
        exports.functions.insert(f.name.to_string(), ExportedFn { params, ret: f.ret.to_type(), def });
    }
    Some(exports)
}

/// A hint listing the modules: "its modules are `std.math`, `std.string`".
pub fn modules_hint() -> String {
    if MODULES.is_empty() {
        return "the standard library has no modules yet".into();
    }
    let names: Vec<String> = MODULES.iter().map(|m| format!("`std.{}`", m.name)).collect();
    format!("its modules are {}", names.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference documents every function with its exact signature and
    /// description, so the two cannot drift apart.
    #[test]
    fn the_reference_matches_the_table() {
        let reference = include_str!("../../../docs/reference.md");
        for module in MODULES {
            let heading = format!("### `std.{}`", module.name);
            let start = reference.find(&heading).unwrap_or_else(|| panic!("docs/reference.md has no section {heading}"));
            let section = &reference[start..];
            let section = &section[..section[heading.len()..].find("\n#").map_or(section.len(), |i| i + heading.len())];
            assert!(section.contains(module.doc), "the reference does not describe std.{} as: {}", module.name, module.doc);
            for f in module.functions {
                let row = format!("| `{}` | {} |", f.signature().trim_start_matches("fn "), f.doc);
                assert!(section.contains(&row), "docs/reference.md, {heading}, needs this row:\n{row}");
            }
            let rows = section.lines().filter(|l| l.starts_with("| `")).count();
            assert_eq!(rows, module.functions.len(), "{heading} lists a function the table does not have");
        }
    }

    #[test]
    fn modules_and_functions_are_in_order_and_unique() {
        let names: Vec<&str> = MODULES.iter().map(|m| m.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names, sorted, "MODULES must be sorted and unique");
        for module in MODULES {
            let mut seen = std::collections::HashSet::new();
            for f in module.functions {
                assert!(seen.insert(f.name), "std.{}.{} is listed twice", module.name, f.name);
                assert!(!f.doc.is_empty() && !f.doc.contains('\n'), "std.{}.{} needs a one-line doc", module.name, f.name);
            }
        }
    }
}
