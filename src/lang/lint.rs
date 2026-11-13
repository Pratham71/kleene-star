//! Warnings: things that are allowed but probably mistakes.
//!
//! A warning never stops a program. They are only looked for when the
//! program has no errors, and each one is meant to be worth reading, so the
//! rules below try hard not to nag:
//!
//! * A state no path from the start state reaches (any kind of machine).
//! * In a DFA or NFA, a state from which no accepting state can be reached.
//!   A DFA needs somewhere to send the words it has already rejected, so one
//!   *dead state* that loops to itself on every symbol is expected and not
//!   reported.
//! * Nothing at all about a machine that can never accept: that is usually
//!   deliberate, an example of the empty language or of a machine that runs
//!   forever.
//! * A `let` or a function that is never used. Names starting with `_` are
//!   exempt, as is everything typed into the REPL.
//! * An import, or a name taken with `from ... import`, that is never used.
//! * A `test` whose condition uses no names, calls or regexes, so it always
//!   gives the same answer.

use super::ast::*;
use super::codes;
use super::error::Diag;
use super::index::{DefKind, Index};
use super::machine::MachineSpec;
use super::token::FileId;
use std::collections::{HashMap, HashSet};

/// Warnings about one machine written out in full (no loops), which has
/// already passed the checks in `machine::build`.
pub fn machine(spec: &MachineSpec) -> Vec<Diag> {
    // States in order of first mention, with where they were first written.
    let mut order: Vec<(&str, super::token::Span)> = Vec::new();
    let mentions = spec
        .starts
        .iter()
        .map(|(n, s)| (n.as_str(), *s))
        .chain(spec.accepts.iter().map(|(n, s)| (n.as_str(), *s)))
        .chain(spec.rejects.iter().map(|(n, s)| (n.as_str(), *s)))
        .chain(spec.transitions.iter().flat_map(|t| [(t.from.as_str(), t.from_span), (t.to.as_str(), t.to_span)]));
    for (name, span) in mentions {
        if !order.iter().any(|(n, _)| *n == name) {
            order.push((name, span));
        }
    }
    let Some((start, _)) = spec.starts.first() else { return Vec::new() };

    let mut next: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut prev: HashMap<&str, Vec<&str>> = HashMap::new();
    for t in &spec.transitions {
        next.entry(&t.from).or_default().push(&t.to);
        prev.entry(&t.to).or_default().push(&t.from);
    }
    let reachable = closure([start.as_str()], &next);
    // A machine that can never accept is an example of the empty language,
    // or of a machine that runs forever: deliberate, so leave it alone.
    if !spec.accepts.iter().any(|(name, _)| reachable.contains(name.as_str())) {
        return Vec::new();
    }

    let mut warnings = Vec::new();
    for (name, span) in &order {
        if !reachable.contains(name) {
            warnings.push(
                Diag::warning(
                    codes::UNREACHABLE_STATE,
                    *span,
                    format!("state `{name}` can never be reached from the start state `{start}`"),
                )
                .hint("add a transition into it, or remove it; it does not change the language"),
            );
        }
    }

    // Dead states only make sense for finite automata: a PDA's or Turing
    // machine's moves depend on more than the state.
    if !matches!(spec.kind, MachineKind::Dfa | MachineKind::Nfa) {
        return warnings;
    }
    let accepting = spec.accepts.iter().map(|(n, _)| n.as_str());
    let alive = closure(accepting, &prev);
    // A state whose every transition leads back to itself.
    let is_trap = |state: &str| next.get(state).is_some_and(|targets| targets.iter().all(|t| *t == state));
    for (name, span) in &order {
        if !reachable.contains(name) || alive.contains(name) {
            continue;
        }
        if spec.kind == MachineKind::Dfa && is_trap(name) {
            continue;
        }
        let hint = if spec.kind == MachineKind::Dfa {
            "every word that gets here is rejected, which is what a dead state is for; one dead state that loops to itself is enough"
        } else {
            "every path through this state is rejected; an NFA does not need a dead state, so this one can go"
        };
        warnings.push(
            Diag::warning(codes::DEAD_STATE, *span, format!("no accepting state can be reached from state `{name}`"))
                .hint(hint),
        );
    }
    warnings
}

/// Everything reachable from `from` by following `edges`.
fn closure<'a>(from: impl IntoIterator<Item = &'a str>, edges: &HashMap<&'a str, Vec<&'a str>>) -> HashSet<&'a str> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut todo: Vec<&str> = from.into_iter().collect();
    while let Some(state) = todo.pop() {
        if seen.insert(state) {
            todo.extend(edges.get(state).into_iter().flatten().copied());
        }
    }
    seen
}

/// Warnings about the program as a whole, the text of file `file`: unused
/// names and imports, and constant tests.
pub fn program(stmts: &[Stmt], index: &Index, repl: bool, file: FileId) -> Vec<Diag> {
    let mut warnings = Vec::new();
    if !repl {
        let mut used = vec![false; index.defs.len()];
        for &(span, def) in &index.refs {
            // A function's own name is recorded as a use of itself.
            if span != index.defs[def].span {
                used[def] = true;
            }
        }
        for (def, used) in index.defs.iter().zip(used) {
            // Definitions in other files are in the index only so that
            // editors can show them.
            if used || def.name.starts_with('_') || def.span.file != file {
                continue;
            }
            if matches!(def.kind, DefKind::Module | DefKind::Imported) {
                let what = if def.kind == DefKind::Module { "module" } else { "imported name" };
                warnings.push(
                    Diag::warning(codes::UNUSED_IMPORT, def.span, format!("{what} `{}` is never used", def.name))
                        .hint("remove the import; an import that is never used costs a file read and a check"),
                );
                continue;
            }
            let (code, what) = match def.kind {
                DefKind::Variable => (codes::UNUSED, "value"),
                DefKind::Function => (codes::UNUSED_FUNCTION, "function"),
                _ => continue,
            };
            warnings.push(
                Diag::warning(code, def.span, format!("{what} `{}` is never used", def.name))
                    .hint(format!("remove it, or start its name with `_` (`_{}`) to keep it quietly", def.name)),
            );
        }
    }
    constant_tests(stmts, &mut warnings);
    warnings
}

fn constant_tests(stmts: &[Stmt], warnings: &mut Vec<Diag>) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Test(cond) if is_constant(cond) => warnings.push(
                Diag::warning(
                    codes::CONSTANT_TEST,
                    cond.span,
                    "this test always gives the same answer: it uses no names, calls or regexes",
                )
                .hint("a test should check something that could go wrong, like `test M accepts \"0110\"`"),
            ),
            StmtKind::Fn(f) => constant_tests(&f.body, warnings),
            StmtKind::If { then, otherwise, .. } => {
                constant_tests(then, warnings);
                constant_tests(otherwise.as_deref().unwrap_or_default(), warnings);
            }
            StmtKind::For { body, .. } => constant_tests(body, warnings),
            _ => {}
        }
    }
}

/// True if `expr` is made only of literal numbers, strings and booleans.
fn is_constant(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) => true,
        ExprKind::Paren(inner) | ExprKind::Unary(_, inner) => is_constant(inner),
        ExprKind::Binary(_, left, right) => is_constant(left) && is_constant(right),
        ExprKind::Set(items) => items.iter().all(is_constant),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::lang::check::Checker;
    use crate::lang::lexer::lex;
    use crate::lang::parser::Parser;

    /// The codes of the warnings for `src`, in order.
    fn warnings(src: &str) -> Vec<&'static str> {
        let program = Parser::new(lex(src, 0, 1).unwrap(), src, false).program().unwrap();
        let mut checker = Checker::new(false);
        assert!(checker.check_all(&program).is_empty(), "{src} has errors");
        checker.warnings(&program).into_iter().map(|w| w.code).collect()
    }

    const B: &str = "alphabet B = {0, 1}\n";

    #[test]
    fn a_good_program_has_none() {
        let src = format!("{B}dfa M over B {{ start a  accept b  a --0--> b  a --1--> d  b --0,1--> b  d --0,1--> d }}\ntest M accepts \"0\"");
        assert_eq!(warnings(&src), Vec::<&str>::new());
    }

    #[test]
    fn unreachable_and_dead_states() {
        let dfa = format!("{B}dfa M over B {{ start a  accept b  a --0,1--> b  b --0,1--> b  z --0,1--> a }}\nprint M");
        assert_eq!(warnings(&dfa), ["W0100"]);
        // In a DFA, a second dead state that is not a trap is reported.
        let dfa = format!("{B}dfa M over B {{ start a  accept b  a --0--> b  a --1--> c  b --0,1--> b  c --0,1--> d  d --0,1--> d }}\nprint M");
        assert_eq!(warnings(&dfa), ["W0101"]);
        // In an NFA every dead state is reported.
        let nfa = format!("{B}nfa N over B {{ start a  accept b  a --0--> b  a --1--> c }}\nprint N");
        assert_eq!(warnings(&nfa), ["W0101"]);
        // A machine that accepts nothing is left alone.
        let empty = format!("{B}nfa N over B {{ start a  accept z  a --0--> b }}\nprint N");
        assert_eq!(warnings(&empty), Vec::<&str>::new());
    }

    #[test]
    fn unused_names_and_constant_tests() {
        assert_eq!(warnings("let x = 1\nlet _y = 2\nprint 3"), ["W0200"]);
        assert_eq!(warnings("fn f() -> Int { return 1 }\nfn g() -> Int { return f() }\nprint g()"), Vec::<&str>::new());
        assert_eq!(warnings("fn f() -> Int { return 1 }"), ["W0201"]);
        assert_eq!(warnings("test 1 < 2\ntest true\ntest /0*/ accepts \"0\""), ["W0300", "W0300"]);
    }
}
