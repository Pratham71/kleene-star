//! `std.random`: random numbers, words, machines and grammars from a seed.
//!
//! Every function takes the seed as its first argument and is a plain
//! function of its arguments: the same call gives the same answer every
//! time, on every computer, so a program that uses randomness still has
//! one fixed output and its tests never fail by chance. For different
//! answers, pass different seeds, such as the counter of a `for` loop.
//!
//! The generator is `automata::random::Rng` (SplitMix64), started from the
//! seed for each call. The operating system is never asked for randomness.

use super::{alphabet_arg, int_arg as int, Failure, Function, Module, Ty};
use crate::automata::random::{self, Rng};
use crate::lang::codes;
use crate::lang::langs;
use crate::lang::value::Value;
use std::collections::BTreeSet;
use std::rc::Rc;

pub const MODULE: Module = Module {
    name: "random",
    doc: "Random Ints, words, DFAs, NFAs and grammars from a seed: the same seed always gives the same result.",
    functions: &[
        Function { name: "int", params: &[("seed", Ty::Int), ("low", Ty::Int), ("high", Ty::Int)], ret: Ty::Int, doc: "An Int from `low` to `high`, both included, each as likely.", run: int_between },
        Function { name: "pick", params: &[("seed", Ty::Int), ("s", Ty::Set(&Ty::Elem))], ret: Ty::Elem, doc: "A member of the set `s`, each as likely; the set must not be empty.", run: pick },
        Function { name: "word", params: &[("seed", Ty::Int), ("alphabet", Ty::Alphabet), ("n", Ty::Int)], ret: Ty::Str, doc: "A word of length `n` over `alphabet`, each symbol drawn on its own.", run: word },
        Function { name: "words", params: &[("seed", Ty::Int), ("alphabet", Ty::Alphabet), ("count", Ty::Int), ("max", Ty::Int)], ret: Ty::Set(&Ty::Str), doc: "`count` random words of length 0 to `max`, as a set, so fewer if some are drawn twice.", run: words },
        Function { name: "word_in", params: &[("seed", Ty::Int), ("lang", Ty::Lang), ("n", Ty::Int)], ret: Ty::Str, doc: "A word of length `n` in the language, each such word as likely; there must be one.", run: word_in },
        Function { name: "random_dfa", params: &[("seed", Ty::Int), ("alphabet", Ty::Alphabet), ("states", Ty::Int)], ret: Ty::Dfa, doc: "A random DFA with `states` states named `q0`, `q1`, ...", run: dfa },
        Function { name: "random_nfa", params: &[("seed", Ty::Int), ("alphabet", Ty::Alphabet), ("states", Ty::Int)], ret: Ty::Nfa, doc: "A random NFA with `states` states named `q0`, `q1`, ..., which may have eps moves.", run: nfa },
        Function { name: "random_grammar", params: &[("seed", Ty::Int), ("alphabet", Ty::Alphabet), ("variables", Ty::Int)], ret: Ty::Grammar, doc: "A random grammar with `variables` variables, from 1 to 26, named `S`, `A`, `B`, ...", run: grammar },
    ],
    explain: None,
};

/// The most states a random machine may have, as in `std.automata`.
const MAX_STATES: i64 = super::automata::MAX_STATES;
/// The longest random word, as in `std.automata.count_words`.
const MAX_LENGTH: i64 = super::automata::MAX_STATES;
/// The most words `words` draws, as in `std.collections.words`.
const MAX_WORDS: i64 = super::collections::MAX_WORDS as i64;
/// The most characters `words` may draw in all.
const MAX_CHARS: i64 = super::string::MAX_RESULT_CHARS as i64;

/// The generator for argument 0, the seed. Negative seeds are as good as
/// any: their 64 bits are the starting point.
fn rng(args: &[Value]) -> Rng {
    Rng::new(int(args, 0) as u64)
}

/// Argument `i`: a whole number from `min` to `max`, or a failure that
/// names `what` it is.
fn bounded(args: &[Value], i: usize, what: &str, min: i64, max: i64) -> Result<usize, Failure> {
    let n = int(args, i);
    if n < min {
        return Err(Failure {
            code: codes::STRING_RANGE,
            arg: Some(i),
            message: format!("{what} must be at least {min}, but this is {n}"),
            hint: format!("use {min} or more"),
        });
    }
    if n > max {
        return Err(Failure {
            code: codes::RESULT_TOO_LARGE,
            arg: Some(i),
            message: format!("{what} can be at most {max}, but this is {n}"),
            hint: format!("use {max} or less"),
        });
    }
    Ok(n as usize)
}

/// Argument 1, an alphabet with at least one symbol.
fn symbols(args: &[Value]) -> Result<&[char], Failure> {
    let alphabet = alphabet_arg(args, 1);
    if alphabet.is_empty() {
        return Err(Failure {
            code: codes::EMPTY_SET,
            arg: Some(1),
            message: "the alphabet has no symbols to draw".into(),
            hint: "give an alphabet with at least one symbol".into(),
        });
    }
    Ok(alphabet)
}

fn int_between(args: &[Value]) -> Result<Value, Failure> {
    let (low, high) = (int(args, 1), int(args, 2));
    if low > high {
        return Err(Failure {
            code: codes::MATH_DOMAIN,
            arg: Some(2),
            message: format!("there is no Int from {low} to {high}"),
            hint: "`low` must not be larger than `high`".into(),
        });
    }
    // The width of the range, as the 64-bit count it really is; `wrapping`
    // because from i64::MIN to i64::MAX is 2^64 - 1 apart.
    let width = high.wrapping_sub(low) as u64;
    let mut rng = rng(args);
    let offset = if width == u64::MAX { rng.next_u64() } else { rng.below(width + 1) };
    Ok(Value::Int(low.wrapping_add(offset as i64)))
}

fn pick(args: &[Value]) -> Result<Value, Failure> {
    let Value::Set(set) = &args[1] else { unreachable!("the checker allowed {:?} as a Set", args[1]) };
    if set.is_empty() {
        return Err(Failure {
            code: codes::EMPTY_SET,
            arg: Some(1),
            message: "cannot pick a member of the empty set".into(),
            hint: "give a set with at least one member".into(),
        });
    }
    let i = rng(args).index(set.len());
    Ok(set.iter().nth(i).expect("the index is below the size").clone())
}

fn word(args: &[Value]) -> Result<Value, Failure> {
    let n = bounded(args, 2, "a length", 0, MAX_LENGTH)?;
    let alphabet = symbols(args)?;
    Ok(Value::str(&random::word(&mut rng(args), alphabet, n)))
}

fn words(args: &[Value]) -> Result<Value, Failure> {
    let count = bounded(args, 2, "a number of words", 0, MAX_WORDS)?;
    let max = bounded(args, 3, "a length", 0, MAX_LENGTH)?;
    if (count as i64).saturating_mul(max as i64) > MAX_CHARS {
        return Err(Failure {
            code: codes::RESULT_TOO_LARGE,
            arg: None,
            message: format!("{count} words of up to {max} symbols could be too long"),
            hint: format!("`count` times `max` may be at most {MAX_CHARS}"),
        });
    }
    let alphabet = symbols(args)?;
    let mut rng = rng(args);
    let set: BTreeSet<Value> = (0..count)
        .map(|_| {
            let n = rng.index(max + 1);
            Value::str(&random::word(&mut rng, alphabet, n))
        })
        .collect();
    Ok(Value::Set(Rc::new(set)))
}

fn word_in(args: &[Value]) -> Result<Value, Failure> {
    let n = bounded(args, 2, "a length", 0, MAX_LENGTH)?;
    let dfa = langs::to_dfas(&args[1..2]).expect("one language always has an alphabet").remove(0);
    random::word_of(&mut rng(args), &dfa, n).map(|w| Value::str(&w)).ok_or_else(|| Failure {
        code: codes::NO_SUCH_WORD,
        arg: Some(2),
        message: format!("the language has no word of length {n}"),
        hint: "`std.automata.count_words(lang, n)` says how many words of a length there are".into(),
    })
}

fn dfa(args: &[Value]) -> Result<Value, Failure> {
    let states = bounded(args, 2, "a number of states", 1, MAX_STATES)?;
    let alphabet = symbols(args)?;
    Ok(Value::dfa(random::dfa(&mut rng(args), alphabet, states)))
}

fn nfa(args: &[Value]) -> Result<Value, Failure> {
    let states = bounded(args, 2, "a number of states", 1, MAX_STATES)?;
    let alphabet = symbols(args)?;
    Ok(Value::nfa(random::nfa(&mut rng(args), alphabet, states)))
}

fn grammar(args: &[Value]) -> Result<Value, Failure> {
    let variables = bounded(args, 2, "a number of variables", 1, random::VARIABLE_NAMES.len() as i64)?;
    let alphabet = symbols(args)?;
    Ok(Value::Grammar(Rc::new(random::grammar(&mut rng(args), alphabet, variables))))
}
