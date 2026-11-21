//! `std.grammar`: helpers for context-free grammars, building on the
//! built-ins `to_cnf`, `is_empty`, `derive`, `to_pda` and `accepts`.
//!
//! The work is done in `automata::grammar` (the transformations) and
//! `automata::cfg` (FIRST, FOLLOW, LL(1), CNF checks, listing words and
//! the ready-made grammars); this file checks arguments and makes values.
//! Three groups:
//!
//! * **Cleaning up**, the steps of the CNF construction one at a time, so
//!   they can be shown and checked separately: `remove_useless`,
//!   `remove_eps`, `remove_units`, and the sets they are based on,
//!   `nullable`, `generating` and `reachable`.
//! * **Parsing questions**: `first`, `follow`, `is_ll1`, `is_cnf`, and
//!   `words_up_to` to look at a language.
//! * **Ready-made grammars**: `balanced`, `anbn`, `palindromes`,
//!   `equal_count`.
//!
//! Sets of variables are sets of their names. In `first` and `follow` a
//! terminal is a one-symbol string and `""` means "nothing": the empty
//! word in FIRST, the end of the input in FOLLOW.

use super::{alphabet_arg, int_arg as int, str_arg as text, Failure, Function, Module, Ty};
use crate::automata::cfg;
use crate::automata::grammar::Grammar;
use crate::lang::codes;
use crate::lang::value::Value;
use std::collections::BTreeSet;
use std::rc::Rc;

const GRAMMAR: Ty = Ty::Grammar;
const NAMES: Ty = Ty::Set(&Ty::Str);

pub const MODULE: Module = Module {
    name: "grammar",
    doc: "Helpers for context-free grammars: the steps of the CNF construction one at a time, FIRST and FOLLOW sets, LL(1) and CNF checks, listing short words, and ready-made grammars.",
    functions: &[
        Function { name: "nullable", params: &[("g", GRAMMAR)], ret: NAMES, doc: "The variables that can derive the empty word.", run: nullable },
        Function { name: "generating", params: &[("g", GRAMMAR)], ret: NAMES, doc: "The variables that can derive some word of terminals.", run: generating },
        Function { name: "reachable", params: &[("g", GRAMMAR)], ret: NAMES, doc: "The variables that can appear in a derivation from the start variable.", run: reachable },
        Function { name: "remove_useless", params: &[("g", GRAMMAR)], ret: GRAMMAR, doc: "The same grammar without the variables that generate nothing or cannot be reached.", run: remove_useless },
        Function { name: "remove_eps", params: &[("g", GRAMMAR)], ret: GRAMMAR, doc: "An equivalent grammar whose only eps rule, if any, is for a start variable that is on no right-hand side.", run: remove_eps },
        Function { name: "remove_units", params: &[("g", GRAMMAR)], ret: GRAMMAR, doc: "An equivalent grammar without unit rules such as `A -> B`.", run: remove_units },
        Function { name: "is_cnf", params: &[("g", GRAMMAR)], ret: Ty::Bool, doc: "Whether the grammar is in Chomsky normal form, as `to_cnf` makes it.", run: is_cnf },
        Function { name: "first", params: &[("g", GRAMMAR), ("variable", Ty::Str)], ret: NAMES, doc: "The terminals that can begin a word `variable` derives, and `\"\"` if it can derive the empty word.", run: first },
        Function { name: "follow", params: &[("g", GRAMMAR), ("variable", Ty::Str)], ret: NAMES, doc: "The terminals that can come right after `variable`, and `\"\"` if it can come last.", run: follow },
        Function { name: "is_ll1", params: &[("g", GRAMMAR)], ret: Ty::Bool, doc: "Whether a parser can always choose the rule to use by looking at the next symbol only.", run: is_ll1 },
        Function { name: "words_up_to", params: &[("g", GRAMMAR), ("n", Ty::Int)], ret: NAMES, doc: "The words of the grammar's language of length at most `n`, as a set.", run: words_up_to },
        Function { name: "balanced", params: &[("open", Ty::Str), ("close", Ty::Str)], ret: GRAMMAR, doc: "Balanced brackets: `S -> open S close S` or eps.", run: balanced },
        Function { name: "anbn", params: &[("a", Ty::Str), ("b", Ty::Str)], ret: GRAMMAR, doc: "`n` copies of `a` then `n` copies of `b`: `S -> a S b` or eps.", run: anbn },
        Function { name: "palindromes", params: &[("alphabet", Ty::Alphabet)], ret: GRAMMAR, doc: "The words over `alphabet` that read the same backwards.", run: palindromes },
        Function { name: "equal_count", params: &[("a", Ty::Str), ("b", Ty::Str)], ret: GRAMMAR, doc: "The words over `a` and `b` with as many of one as of the other.", run: equal_count },
    ],
    explain: None,
};

/// The most words `words_up_to` lists, as in `std.automata.words_up_to`.
const MAX_WORDS: usize = super::collections::MAX_WORDS as usize;
/// The longest length `words_up_to` takes, like `std.automata.count_words`.
const MAX_LENGTH: i64 = super::automata::MAX_STATES;
/// How many characters of words `words_up_to` may build on the way, for
/// every variable of the CNF copy, as in `std.automata.words_up_to`.
const MAX_WORK: usize = super::string::MAX_RESULT_CHARS as usize;
/// The most rules `remove_eps` may make.
const MAX_RULES: usize = 100_000;

fn grammar(args: &[Value], i: usize) -> &Grammar {
    match &args[i] {
        Value::Grammar(g) => g,
        other => unreachable!("the checker allowed {other:?} as a Grammar"),
    }
}

fn value(g: Grammar) -> Value {
    Value::Grammar(Rc::new(g))
}

fn strings(items: impl IntoIterator<Item = String>) -> Value {
    Value::Set(Rc::new(items.into_iter().map(|s| Value::str(&s)).collect::<BTreeSet<_>>()))
}

/// The names of the variables marked true.
fn names(g: &Grammar, marked: &[bool]) -> Value {
    strings(g.variables.iter().zip(marked).filter(|(_, m)| **m).map(|(v, _)| v.clone()))
}

/// Argument `i`: the name of one of the grammar's variables.
fn variable(args: &[Value], i: usize) -> Result<usize, Failure> {
    let g = grammar(args, 0);
    let name = text(args, i);
    g.variables.iter().position(|v| v == name).ok_or_else(|| {
        let known: Vec<String> = g.variables.iter().map(|v| format!("`{v}`")).collect();
        Failure {
            code: codes::NO_SUCH_VARIABLE,
            arg: Some(i),
            message: format!("the grammar has no variable {name:?}"),
            hint: format!("its variables are {}", known.join(", ")),
        }
    })
}

/// Argument `i`: a string of exactly one symbol, a letter or digit as in
/// an alphabet written in a program.
fn symbol(args: &[Value], i: usize) -> Result<char, Failure> {
    let given = text(args, i);
    let mut chars = given.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_alphanumeric() => Ok(c),
        (Some(c), None) => Err(Failure {
            code: codes::NOT_A_SYMBOL,
            arg: Some(i),
            message: format!("`{c}` cannot be a symbol"),
            hint: "a symbol is one letter or digit, as in an alphabet".into(),
        }),
        _ => Err(Failure {
            code: codes::NOT_A_SYMBOL,
            arg: Some(i),
            message: format!("{given:?} is not one symbol"),
            hint: "write a single letter or digit, such as \"a\"".into(),
        }),
    }
}

/// Arguments 0 and 1: two different symbols.
fn two_symbols(args: &[Value]) -> Result<(char, char), Failure> {
    let (a, b) = (symbol(args, 0)?, symbol(args, 1)?);
    if a == b {
        return Err(Failure {
            code: codes::DUPLICATE_SYMBOL,
            arg: Some(1),
            message: format!("both symbols are `{a}`"),
            hint: "the two symbols must be different".into(),
        });
    }
    Ok((a, b))
}

fn nullable(args: &[Value]) -> Result<Value, Failure> {
    let g = grammar(args, 0);
    Ok(names(g, &g.nullable()))
}

fn generating(args: &[Value]) -> Result<Value, Failure> {
    let g = grammar(args, 0);
    Ok(names(g, &g.generating()))
}

fn reachable(args: &[Value]) -> Result<Value, Failure> {
    let g = grammar(args, 0);
    Ok(names(g, &g.reachable()))
}

fn remove_useless(args: &[Value]) -> Result<Value, Failure> {
    Ok(value(grammar(args, 0).trimmed()))
}

fn remove_eps(args: &[Value]) -> Result<Value, Failure> {
    grammar(args, 0).remove_eps(MAX_RULES).map(value).ok_or_else(|| Failure {
        code: codes::RESULT_TOO_LARGE,
        arg: Some(0),
        message: format!("removing the eps rules would make more than {MAX_RULES} rules"),
        hint: "each rule is copied for every way of leaving out its nullable variables; split long rules first".into(),
    })
}

fn remove_units(args: &[Value]) -> Result<Value, Failure> {
    Ok(value(grammar(args, 0).remove_units()))
}

fn is_cnf(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(cfg::is_cnf(grammar(args, 0))))
}

fn first(args: &[Value]) -> Result<Value, Failure> {
    let g = grammar(args, 0);
    let v = variable(args, 1)?;
    let mut set: Vec<String> = cfg::first_sets(g)[v].iter().map(|c| c.to_string()).collect();
    if g.nullable()[v] {
        set.push(String::new());
    }
    Ok(strings(set))
}

fn follow(args: &[Value]) -> Result<Value, Failure> {
    let g = grammar(args, 0);
    let v = variable(args, 1)?;
    let (terminals, end) = &cfg::follow_sets(g)[v];
    let mut set: Vec<String> = terminals.iter().map(|c| c.to_string()).collect();
    if *end {
        set.push(String::new());
    }
    Ok(strings(set))
}

fn is_ll1(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(cfg::is_ll1(grammar(args, 0))))
}

fn words_up_to(args: &[Value]) -> Result<Value, Failure> {
    let n = int(args, 1);
    if n < 0 {
        return Err(Failure {
            code: codes::STRING_RANGE,
            arg: Some(1),
            message: format!("a length cannot be negative, but this is {n}"),
            hint: "use 0 or more".into(),
        });
    }
    if n > MAX_LENGTH {
        return Err(Failure {
            code: codes::RESULT_TOO_LARGE,
            arg: Some(1),
            message: format!("words of length up to {n} are too long to list"),
            hint: format!("the longest length `words_up_to` takes is {MAX_LENGTH}"),
        });
    }
    let too_many = || Failure {
        code: codes::RESULT_TOO_LARGE,
        arg: None,
        message: format!("the words of length up to {n} are too many to list"),
        hint: format!("ask for shorter words (at most {MAX_WORDS} are listed)"),
    };
    let words = cfg::words_up_to(grammar(args, 0), n as usize, MAX_WORK).ok_or_else(too_many)?;
    if words.len() > MAX_WORDS {
        return Err(too_many());
    }
    Ok(strings(words))
}

fn balanced(args: &[Value]) -> Result<Value, Failure> {
    let (open, close) = two_symbols(args)?;
    Ok(value(cfg::balanced(open, close)))
}

fn anbn(args: &[Value]) -> Result<Value, Failure> {
    let (a, b) = two_symbols(args)?;
    Ok(value(cfg::anbn(a, b)))
}

fn palindromes(args: &[Value]) -> Result<Value, Failure> {
    Ok(value(cfg::palindromes(alphabet_arg(args, 0))))
}

fn equal_count(args: &[Value]) -> Result<Value, Failure> {
    let (a, b) = two_symbols(args)?;
    Ok(value(cfg::equal_count(a, b)))
}
