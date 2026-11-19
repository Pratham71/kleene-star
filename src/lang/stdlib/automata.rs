//! `std.automata`: ready-made DFAs for common patterns, and questions about
//! regular languages that the built-ins do not answer.
//!
//! The machines come from `automata::patterns` and the answers from
//! `automata::questions`; this file only checks the arguments and turns
//! them into values. Two groups of functions:
//!
//! * **Pattern machines** (`starts_with`, `count_mod`, ...) take the
//!   alphabet first and return the smallest `Dfa` for the pattern. They are
//!   ordinary machines: pass them to `minimize`, `union`, `trace` or `draw`
//!   like any other.
//! * **Questions** (`shortest_word`, `is_finite`, ...) take a `Lang`: any
//!   regular language, the same four types the built-ins take (`Dfa`,
//!   `Nfa`, `Regex`, `Set[Str]`), converted the same way (`lang::langs`).
//!   They extend the built-ins rather than repeat them: `equivalent` says
//!   *whether* two languages differ, `distinguishing_word` says *where*;
//!   `is_empty` says whether there is a word, `shortest_word` gives one.
//!
//! The names `starts_with` and `ends_with` also belong to `std.string`,
//! where they test one string. Here they build a machine for every string
//! with that start or end.

use super::{Failure, Function, Module, Ty};
use crate::automata::{fmt_alphabet, ops, patterns, questions, Dfa};
use crate::lang::codes;
use crate::lang::langs;
use crate::lang::value::Value;
use std::collections::BTreeSet;
use std::rc::Rc;

/// The largest count, modulus, length or pattern a pattern machine accepts:
/// each one becomes about that many states.
pub const MAX_STATES: i64 = 10_000;

pub const MODULE: Module = Module {
    name: "automata",
    doc: "Ready-made DFAs for common patterns (prefixes, suffixes, substrings, counting, parity, lengths), and questions about regular languages: a shortest word, a word that tells two languages apart, finiteness and counting words.",
    functions: &[
        Function { name: "starts_with", params: &[("alphabet", Ty::Alphabet), ("prefix", Ty::Str)], ret: Ty::Dfa, doc: "The smallest DFA for the strings over `alphabet` that begin with `prefix`.", run: starts_with },
        Function { name: "ends_with", params: &[("alphabet", Ty::Alphabet), ("suffix", Ty::Str)], ret: Ty::Dfa, doc: "The smallest DFA for the strings over `alphabet` that end with `suffix`.", run: ends_with },
        Function { name: "contains_substring", params: &[("alphabet", Ty::Alphabet), ("part", Ty::Str)], ret: Ty::Dfa, doc: "The smallest DFA for the strings over `alphabet` that contain `part` somewhere.", run: contains_substring },
        Function { name: "exactly", params: &[("alphabet", Ty::Alphabet), ("symbol", Ty::Str), ("n", Ty::Int)], ret: Ty::Dfa, doc: "The smallest DFA for the strings with exactly `n` copies of `symbol`.", run: exactly },
        Function { name: "at_least", params: &[("alphabet", Ty::Alphabet), ("symbol", Ty::Str), ("n", Ty::Int)], ret: Ty::Dfa, doc: "The smallest DFA for the strings with at least `n` copies of `symbol`.", run: at_least },
        Function { name: "at_most", params: &[("alphabet", Ty::Alphabet), ("symbol", Ty::Str), ("n", Ty::Int)], ret: Ty::Dfa, doc: "The smallest DFA for the strings with at most `n` copies of `symbol`.", run: at_most },
        Function { name: "count_mod", params: &[("alphabet", Ty::Alphabet), ("symbol", Ty::Str), ("m", Ty::Int), ("r", Ty::Int)], ret: Ty::Dfa, doc: "The smallest DFA for the strings whose number of `symbol`s leaves remainder `r` when divided by `m`; `m = 2` gives parity.", run: count_mod },
        Function { name: "length_mod", params: &[("alphabet", Ty::Alphabet), ("m", Ty::Int), ("r", Ty::Int)], ret: Ty::Dfa, doc: "The smallest DFA for the strings whose length leaves remainder `r` when divided by `m`.", run: length_mod },
        Function { name: "length_between", params: &[("alphabet", Ty::Alphabet), ("min", Ty::Int), ("max", Ty::Int)], ret: Ty::Dfa, doc: "The smallest DFA for the strings of length `min` to `max`, both included.", run: length_between },
        Function { name: "shortest_word", params: &[("lang", Ty::Lang)], ret: Ty::Str, doc: "A shortest word of the language (the first in alphabet order if there are several); the language must not be empty.", run: shortest_word },
        Function { name: "distinguishing_word", params: &[("a", Ty::Lang), ("b", Ty::Lang)], ret: Ty::Str, doc: "A shortest word in one language but not the other, showing why they are not `equivalent`; they must differ.", run: distinguishing_word },
        Function { name: "is_finite", params: &[("lang", Ty::Lang)], ret: Ty::Bool, doc: "Whether the language has only finitely many words.", run: is_finite },
        Function { name: "count_words", params: &[("lang", Ty::Lang), ("n", Ty::Int)], ret: Ty::Int, doc: "How many words of length exactly `n` the language has.", run: count_words },
        Function { name: "words_up_to", params: &[("lang", Ty::Lang), ("n", Ty::Int)], ret: Ty::Set(&Ty::Str), doc: "The words of the language of length at most `n`, as a set; there may be at most 100000 of them.", run: words_up_to },
    ],
};

// ---- reading the arguments ----

fn alphabet(args: &[Value]) -> &[char] {
    match &args[0] {
        Value::Alphabet(symbols) => symbols,
        other => unreachable!("the checker allowed {other:?} as an Alphabet"),
    }
}

fn text(args: &[Value], i: usize) -> &str {
    match &args[i] {
        Value::Str(s) => s,
        other => unreachable!("the checker allowed {other:?} as a Str"),
    }
}

fn int(args: &[Value], i: usize) -> i64 {
    match &args[i] {
        Value::Int(n) => *n,
        other => unreachable!("the checker allowed {other:?} as an Int"),
    }
}

/// Argument `i`: a word whose every symbol is in the alphabet, and short
/// enough to become one state per symbol.
fn pattern(args: &[Value], i: usize) -> Result<Vec<char>, Failure> {
    super::word_over(text(args, i), alphabet(args), i, "the pattern")?;
    let word: Vec<char> = text(args, i).chars().collect();
    if word.len() as i64 > MAX_STATES {
        return Err(too_many_states(i, "a pattern", word.len() as i64));
    }
    Ok(word)
}

/// Argument `i`: one symbol of the alphabet, written as a string.
fn symbol(args: &[Value], i: usize) -> Result<char, Failure> {
    let symbols = alphabet(args);
    let given = text(args, i);
    let mut chars = given.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return Err(Failure {
            code: codes::NOT_A_SYMBOL,
            arg: Some(i),
            message: format!("{given:?} is not one symbol"),
            hint: format!("write a single symbol of the alphabet, such as {:?}", symbols.first().map_or("a".into(), |c| c.to_string())),
        });
    };
    if !symbols.contains(&c) {
        return Err(Failure {
            code: codes::NOT_IN_ALPHABET,
            arg: Some(i),
            message: format!("symbol `{c}` is not in the alphabet"),
            hint: format!("the alphabet is {}", fmt_alphabet(symbols)),
        });
    }
    Ok(c)
}

/// Argument `i`: a count or length from 0 to `MAX_STATES`.
fn count(args: &[Value], i: usize, what: &str) -> Result<usize, Failure> {
    let n = int(args, i);
    if n < 0 {
        return Err(Failure {
            code: codes::STRING_RANGE,
            arg: Some(i),
            message: format!("{what} cannot be negative, but this is {n}"),
            hint: "use 0 or more".into(),
        });
    }
    if n > MAX_STATES {
        return Err(too_many_states(i, what, n));
    }
    Ok(n as usize)
}

fn too_many_states(arg: usize, what: &str, n: i64) -> Failure {
    Failure {
        code: codes::RESULT_TOO_LARGE,
        arg: Some(arg),
        message: format!("{what} of {n} would need more than {MAX_STATES} states"),
        hint: format!("pattern machines are limited to {MAX_STATES} states"),
    }
}

/// Arguments `i` and `i + 1`: a modulus `m` of at least 1 and a remainder
/// `r` from 0 to `m - 1`.
fn modulus(args: &[Value], i: usize) -> Result<(usize, usize), Failure> {
    let (m, r) = (int(args, i), int(args, i + 1));
    if m < 1 {
        return Err(Failure {
            code: codes::MATH_DOMAIN,
            arg: Some(i),
            message: format!("cannot divide by {m}"),
            hint: "the modulus `m` must be 1 or more".into(),
        });
    }
    if m > MAX_STATES {
        return Err(too_many_states(i, "a modulus", m));
    }
    if !(0..m).contains(&r) {
        return Err(Failure {
            code: codes::MATH_DOMAIN,
            arg: Some(i + 1),
            message: format!("a remainder after dividing by {m} cannot be {r}"),
            hint: format!("the remainder `r` must be from 0 to {}", m - 1),
        });
    }
    Ok((m as usize, r as usize))
}

/// The language arguments as DFAs over one alphabet.
fn dfas(langs: &[Value]) -> Result<Vec<Dfa>, Failure> {
    langs::to_dfas(langs).map_err(|problem| Failure {
        code: problem.code,
        arg: Some(problem.arg),
        message: problem.message,
        hint: problem.hint,
    })
}

fn dfa(args: &[Value]) -> Dfa {
    dfas(&args[..1]).expect("one language always has an alphabet").remove(0)
}

// ---- pattern machines ----

fn starts_with(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(patterns::starts_with(alphabet(args), &pattern(args, 1)?)))
}

fn ends_with(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(patterns::ends_with(alphabet(args), &pattern(args, 1)?)))
}

fn contains_substring(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(patterns::contains(alphabet(args), &pattern(args, 1)?)))
}

fn exactly(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(patterns::count_exactly(alphabet(args), symbol(args, 1)?, count(args, 2, "a count")?)))
}

fn at_least(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(patterns::count_at_least(alphabet(args), symbol(args, 1)?, count(args, 2, "a count")?)))
}

fn at_most(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(patterns::count_at_most(alphabet(args), symbol(args, 1)?, count(args, 2, "a count")?)))
}

fn count_mod(args: &[Value]) -> Result<Value, Failure> {
    let c = symbol(args, 1)?;
    let (m, r) = modulus(args, 2)?;
    Ok(Value::dfa(patterns::count_mod(alphabet(args), c, m, r)))
}

fn length_mod(args: &[Value]) -> Result<Value, Failure> {
    let (m, r) = modulus(args, 1)?;
    Ok(Value::dfa(patterns::length_mod(alphabet(args), m, r)))
}

fn length_between(args: &[Value]) -> Result<Value, Failure> {
    let min = count(args, 1, "a length")?;
    let max = count(args, 2, "a length")?;
    if min > max {
        return Err(Failure {
            code: codes::MATH_DOMAIN,
            arg: None,
            message: format!("there is no length from {min} to {max}"),
            hint: "`min` must not be larger than `max`".into(),
        });
    }
    Ok(Value::dfa(patterns::length_between(alphabet(args), min, max)))
}

// ---- questions ----

fn shortest_word(args: &[Value]) -> Result<Value, Failure> {
    match questions::shortest_word(&dfa(args)) {
        Some(word) => Ok(Value::str(&word)),
        None => Err(Failure {
            code: codes::NO_SUCH_WORD,
            arg: Some(0),
            message: "the language is empty, so it has no shortest word".into(),
            hint: "check `is_empty(lang)` first".into(),
        }),
    }
}

fn distinguishing_word(args: &[Value]) -> Result<Value, Failure> {
    let both = dfas(args)?;
    match ops::distinguishing_word(&both[0], &both[1]) {
        Some(word) => Ok(Value::str(&word)),
        None => Err(Failure {
            code: codes::NO_SUCH_WORD,
            arg: None,
            message: "the two languages are equivalent, so no word tells them apart".into(),
            hint: "check `equivalent(a, b)` first".into(),
        }),
    }
}

fn is_finite(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(questions::is_finite(&dfa(args))))
}

fn count_words(args: &[Value]) -> Result<Value, Failure> {
    let n = count(args, 1, "a length")?;
    questions::count_words(&dfa(args), n).map(Value::Int).ok_or_else(|| Failure {
        code: codes::OVERFLOW,
        arg: None,
        message: format!("the number of words of length {n} is too large for an Int"),
        hint: "Ints are 64-bit: about plus or minus 9 * 10^18".into(),
    })
}

fn words_up_to(args: &[Value]) -> Result<Value, Failure> {
    let n = count(args, 1, "a length")?;
    let too_many = || Failure {
        code: codes::RESULT_TOO_LARGE,
        arg: None,
        message: format!("the words of length up to {n} are too many to list"),
        hint: format!(
            "ask for shorter words (at most {} are listed), or use `count_words` to count them",
            super::collections::MAX_WORDS
        ),
    };
    let words = questions::words_up_to(&dfa(args), n, super::string::MAX_RESULT_CHARS as usize).ok_or_else(too_many)?;
    if words.len() as u128 > super::collections::MAX_WORDS {
        return Err(too_many());
    }
    Ok(Value::Set(Rc::new(words.iter().map(|w| Value::str(w)).collect::<BTreeSet<_>>())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &[Value]) -> Result<Value, Failure> {
        let f = MODULE.functions.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no std.automata.{name}"));
        (f.run)(args)
    }

    fn ok(name: &str, args: &[Value]) -> Value {
        call(name, args).unwrap_or_else(|e| panic!("{name}{args:?} failed: {}", e.message))
    }

    fn fails(name: &str, args: &[Value]) -> Failure {
        call(name, args).expect_err("expected a failure")
    }

    fn b() -> Value {
        Value::Alphabet(Rc::new(vec!['0', '1']))
    }

    fn s(text: &str) -> Value {
        Value::str(text)
    }

    fn accepts(machine: &Value, word: &str) -> bool {
        match machine {
            Value::Dfa(d) => d.accepts(word).unwrap(),
            other => panic!("not a Dfa: {other:?}"),
        }
    }

    fn regex(src: &str) -> Value {
        Value::Regex(Rc::new(crate::automata::regex::parse(src).unwrap()))
    }

    #[test]
    fn pattern_machines() {
        let m = ok("starts_with", &[b(), s("01")]);
        assert!(accepts(&m, "011") && !accepts(&m, "10"));
        let m = ok("ends_with", &[b(), s("01")]);
        assert!(accepts(&m, "1101") && !accepts(&m, "010"));
        let m = ok("contains_substring", &[b(), s("11")]);
        assert!(accepts(&m, "0110") && !accepts(&m, "0101"));
        let m = ok("exactly", &[b(), s("1"), Value::Int(2)]);
        assert!(accepts(&m, "0101") && !accepts(&m, "111"));
        let m = ok("at_least", &[b(), s("1"), Value::Int(2)]);
        assert!(accepts(&m, "111") && !accepts(&m, "100"));
        let m = ok("at_most", &[b(), s("1"), Value::Int(1)]);
        assert!(accepts(&m, "000") && !accepts(&m, "101"));
        let m = ok("count_mod", &[b(), s("0"), Value::Int(2), Value::Int(0)]);
        assert!(accepts(&m, "1001") && !accepts(&m, "0"));
        let m = ok("length_mod", &[b(), Value::Int(3), Value::Int(1)]);
        assert!(accepts(&m, "0") && accepts(&m, "0101") && !accepts(&m, "01"));
        let m = ok("length_between", &[b(), Value::Int(1), Value::Int(2)]);
        assert!(accepts(&m, "1") && !accepts(&m, "") && !accepts(&m, "101"));
    }

    #[test]
    fn bad_patterns_and_symbols() {
        assert_eq!(fails("starts_with", &[b(), s("012")]).code, codes::FOREIGN_WORD);
        assert_eq!(fails("exactly", &[b(), s("01"), Value::Int(1)]).code, codes::NOT_A_SYMBOL);
        assert_eq!(fails("exactly", &[b(), s(""), Value::Int(1)]).code, codes::NOT_A_SYMBOL);
        let f = fails("at_most", &[b(), s("2"), Value::Int(1)]);
        assert_eq!((f.code, f.arg), (codes::NOT_IN_ALPHABET, Some(1)));
        assert_eq!(fails("ends_with", &[b(), s(&"0".repeat(10_001))]).code, codes::RESULT_TOO_LARGE);
    }

    #[test]
    fn bad_numbers() {
        let f = fails("exactly", &[b(), s("1"), Value::Int(-1)]);
        assert_eq!((f.code, f.arg), (codes::STRING_RANGE, Some(2)));
        assert_eq!(fails("at_least", &[b(), s("1"), Value::Int(MAX_STATES + 1)]).code, codes::RESULT_TOO_LARGE);
        assert_eq!(fails("count_mod", &[b(), s("1"), Value::Int(0), Value::Int(0)]).code, codes::MATH_DOMAIN);
        let f = fails("length_mod", &[b(), Value::Int(3), Value::Int(3)]);
        assert_eq!((f.code, f.arg), (codes::MATH_DOMAIN, Some(2)));
        assert_eq!(fails("length_mod", &[b(), Value::Int(2), Value::Int(-1)]).code, codes::MATH_DOMAIN);
        assert_eq!(fails("length_between", &[b(), Value::Int(3), Value::Int(2)]).code, codes::MATH_DOMAIN);
        assert_eq!(fails("length_mod", &[b(), Value::Int(i64::MAX), Value::Int(0)]).code, codes::RESULT_TOO_LARGE);
    }

    #[test]
    fn questions_take_any_regular_language() {
        assert_eq!(ok("shortest_word", &[regex("(10)+")]), s("10"));
        let words = Value::Set(Rc::new([s("ab"), s("b")].into_iter().collect()));
        assert_eq!(ok("shortest_word", std::slice::from_ref(&words)), s("b"));
        assert_eq!(ok("is_finite", std::slice::from_ref(&words)), Value::Bool(true));
        assert_eq!(ok("is_finite", &[regex("a*")]), Value::Bool(false));
        assert_eq!(ok("count_words", &[regex("(a|b)*"), Value::Int(3)]), Value::Int(8));
        assert_eq!(ok("words_up_to", &[regex("a*b"), Value::Int(2)]), Value::Set(Rc::new([s("ab"), s("b")].into_iter().collect())));
        assert_eq!(ok("distinguishing_word", &[regex("a*"), regex("a+")]), s(""));
    }

    #[test]
    fn questions_without_an_answer() {
        assert_eq!(fails("shortest_word", &[regex("[]")]).code, codes::NO_SUCH_WORD);
        assert_eq!(fails("distinguishing_word", &[regex("a*"), regex("(aa*)?")]).code, codes::NO_SUCH_WORD);
        assert_eq!(fails("count_words", &[regex("(a|b)*"), Value::Int(64)]).code, codes::OVERFLOW);
        assert_eq!(fails("words_up_to", &[regex("(a|b)*"), Value::Int(30)]).code, codes::RESULT_TOO_LARGE);
        assert_eq!(fails("words_up_to", &[regex("a"), Value::Int(-1)]).code, codes::STRING_RANGE);
    }

    #[test]
    fn languages_must_share_an_alphabet() {
        let d = ok("starts_with", &[b(), s("0")]);
        let f = fails("distinguishing_word", &[d.clone(), regex("a")]);
        assert_eq!((f.code, f.arg), (codes::NOT_IN_ALPHABET, Some(1)));
        let other = Value::dfa(patterns::starts_with(&['a'], &[]));
        assert_eq!(fails("distinguishing_word", &[d, other]).code, codes::ALPHABET_MISMATCH);
    }
}
