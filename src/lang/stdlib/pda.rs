//! `std.pda`: helpers for pushdown automata, building on the built-ins
//! `to_pda`, `accepts`, `trace` and `states`.
//!
//! The work is done in `automata::pdas`; this file checks arguments and
//! makes values. Two groups:
//!
//! * **Ready-made PDAs** for the standard context-free languages, with
//!   states named after what they do: `anbn`, `balanced`, `palindromes`,
//!   `equal_count`. They match the grammars of the same names in
//!   `std.grammar`.
//! * **Questions**: `to_grammar` (the converse of the built-in `to_pda`),
//!   `is_deterministic`, `stack_symbols`, and two that go through the
//!   grammar and so always answer: `member` and `words_up_to`. The `accepts`
//!   operator searches the PDA's runs and gives up when they go on forever;
//!   the grammar has no such runs.

use super::grammar::two_symbols;
use super::{alphabet_arg, int_arg as int, str_arg as text, Failure, Function, Module, Ty};
use crate::automata::grammar::Grammar;
use crate::automata::pda::Pda;
use crate::automata::{fmt_alphabet, pdas};
use crate::lang::codes;
use crate::lang::value::Value;
use std::collections::BTreeSet;
use std::rc::Rc;

pub const MODULE: Module = Module {
    name: "pda",
    doc: "Ready-made pushdown automata, the grammar of a PDA, whether a PDA is deterministic, and membership and word lists that never give up.",
    functions: &[
        Function { name: "anbn", params: &[("a", Ty::Str), ("b", Ty::Str)], ret: Ty::Pda, doc: "A deterministic PDA for `n` copies of `a` then `n` copies of `b`.", run: anbn },
        Function { name: "balanced", params: &[("open", Ty::Str), ("close", Ty::Str)], ret: Ty::Pda, doc: "A deterministic PDA for balanced brackets, with `open` and `close` as the brackets.", run: balanced },
        Function { name: "palindromes", params: &[("alphabet", Ty::Alphabet)], ret: Ty::Pda, doc: "A PDA for the words over `alphabet` that read the same backwards; it guesses where the middle is.", run: palindromes },
        Function { name: "equal_count", params: &[("a", Ty::Str), ("b", Ty::Str)], ret: Ty::Pda, doc: "A PDA for the words over `a` and `b` with as many of one as of the other.", run: equal_count },
        Function { name: "to_grammar", params: &[("p", Ty::Pda)], ret: Ty::Grammar, doc: "A grammar with the same language as the PDA, by the triple construction.", run: to_grammar },
        Function { name: "is_deterministic", params: &[("p", Ty::Pda)], ret: Ty::Bool, doc: "Whether at most one move can be taken in every configuration.", run: is_deterministic },
        Function { name: "stack_symbols", params: &[("p", Ty::Pda)], ret: Ty::Set(&Ty::Str), doc: "The symbols the PDA uses on its stack, the bottom symbol included.", run: stack_symbols },
        Function { name: "member", params: &[("p", Ty::Pda), ("w", Ty::Str)], ret: Ty::Bool, doc: "Whether the PDA accepts `w`, decided through its grammar, so it never gives up as `accepts` can.", run: member },
        Function { name: "words_up_to", params: &[("p", Ty::Pda), ("n", Ty::Int)], ret: Ty::Set(&Ty::Str), doc: "The words the PDA accepts of length at most `n`, as a set.", run: words_up_to },
    ],
    explain: None,
};

/// The most rules `to_grammar` may make before trimming.
const MAX_RULES: usize = 100_000;

fn pda(args: &[Value]) -> &Pda {
    match &args[0] {
        Value::Pda(p) => p,
        other => unreachable!("the checker allowed {other:?} as a Pda"),
    }
}

fn value(p: Pda) -> Value {
    Value::Pda(Rc::new(p))
}

/// The grammar of argument 0, or the failure for a PDA too large to turn
/// into one.
fn grammar(args: &[Value]) -> Result<Grammar, Failure> {
    pdas::to_grammar(pda(args), MAX_RULES).ok_or_else(|| Failure {
        code: codes::RESULT_TOO_LARGE,
        arg: Some(0),
        message: format!("the grammar of this PDA would have more than {MAX_RULES} rules"),
        hint: "a move that pushes k symbols gives (number of states + 2)^k rules; push fewer symbols per move".into(),
    })
}

fn anbn(args: &[Value]) -> Result<Value, Failure> {
    let (a, b) = two_symbols(args)?;
    Ok(value(pdas::anbn(a, b)))
}

fn balanced(args: &[Value]) -> Result<Value, Failure> {
    let (open, close) = two_symbols(args)?;
    Ok(value(pdas::balanced(open, close)))
}

fn palindromes(args: &[Value]) -> Result<Value, Failure> {
    Ok(value(pdas::palindromes(alphabet_arg(args, 0))))
}

fn equal_count(args: &[Value]) -> Result<Value, Failure> {
    let (a, b) = two_symbols(args)?;
    Ok(value(pdas::equal_count(a, b)))
}

fn to_grammar(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Grammar(Rc::new(grammar(args)?)))
}

fn is_deterministic(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(pdas::is_deterministic(pda(args))))
}

fn stack_symbols(args: &[Value]) -> Result<Value, Failure> {
    let symbols: BTreeSet<Value> = pdas::stack_symbols(pda(args)).iter().map(|s| Value::str(s)).collect();
    Ok(Value::Set(Rc::new(symbols)))
}

fn member(args: &[Value]) -> Result<Value, Failure> {
    let word = text(args, 1);
    let p = pda(args);
    if let Some(c) = word.chars().find(|c| !p.alphabet.contains(c)) {
        return Err(Failure {
            code: codes::FOREIGN_WORD,
            arg: Some(1),
            message: format!("the word contains `{c}`, which is not in the PDA's alphabet"),
            hint: format!("the alphabet is {}", fmt_alphabet(&p.alphabet)),
        });
    }
    Ok(Value::Bool(grammar(args)?.accepts(word).expect("the word is over the alphabet")))
}

fn words_up_to(args: &[Value]) -> Result<Value, Failure> {
    let n = int(args, 1);
    super::grammar::check_length(n, 1)?;
    super::grammar::list_words(&grammar(args)?, n as usize)
}
