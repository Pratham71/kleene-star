//! `std.testing`: checks that say *why* they failed.
//!
//! Every function here is a plain predicate that returns a `Bool`, meant to
//! be the condition of a `test` statement:
//!
//! ```text
//! test std.testing.accepts_all(M, {"0", "00"})
//! ```
//!
//! When such a test fails, the interpreter asks this module's `explain` why
//! (the `explain` field of the module's table entry) and prints the answer
//! under the test's line: the two values with the first difference marked,
//! the words that were wrongly accepted or rejected, a word that tells two
//! languages apart. Nothing else changes about `test`, so the rules for
//! modules still hold: `test` statements in an imported file are skipped,
//! and so are these checks in them.
//!
//! The functions build on the language rather than repeat it:
//! `same_language` is the built-in `equivalent` plus the word that shows the
//! difference, `accepts_all` is `accepts` over a whole set, and `agree_on` /
//! `agree_up_to` compare grammars, PDAs and Turing machines on sample words,
//! which is all that can be done for them in general (their equivalence is
//! undecidable).

use super::{Failure, Function, Module, Ty};
use crate::automata::{fmt_alphabet, ops};
use crate::lang::codes;
use crate::lang::langs::{self, Refusal};
use crate::lang::value::Value;
use std::collections::BTreeSet;

/// `agree_up_to` tries at most this many words.
pub const MAX_WORDS: u128 = 10_000;

/// How many words an explanation lists before it says "and N more".
const SHOWN: usize = 10;

const SET_T: Ty = Ty::Set(&Ty::Elem);
const WORDS: Ty = Ty::Set(&Ty::Str);

pub const MODULE: Module = Module {
    name: "testing",
    doc: "Checks for `test` statements that explain a failure: equality with the difference shown, sets of words a machine must accept or reject, equal languages with a word that tells them apart, and two acceptors that must agree on sample words.",
    functions: &[
        Function { name: "equal", params: &[("expected", Ty::Elem), ("actual", Ty::Elem)], ret: Ty::Bool, doc: "Whether `actual` is `expected`; a failing test shows both, with the first difference of two strings marked.", run: equal },
        Function { name: "equal_sets", params: &[("expected", SET_T), ("actual", SET_T)], ret: Ty::Bool, doc: "Whether two sets are equal; a failing test lists the members that are missing and those that should not be there.", run: equal_sets },
        Function { name: "accepts_all", params: &[("machine", Ty::Acceptor), ("words", WORDS)], ret: Ty::Bool, doc: "Whether `machine` accepts every word of `words`; a failing test lists the words it rejects.", run: accepts_all },
        Function { name: "rejects_all", params: &[("machine", Ty::Acceptor), ("words", WORDS)], ret: Ty::Bool, doc: "Whether `machine` rejects every word of `words`; a failing test lists the words it accepts.", run: rejects_all },
        Function { name: "same_language", params: &[("expected", Ty::Lang), ("actual", Ty::Lang)], ret: Ty::Bool, doc: "Whether two regular languages are `equivalent`; a failing test shows a shortest word that is in one but not the other.", run: same_language },
        Function { name: "agree_on", params: &[("a", Ty::Acceptor), ("b", Ty::Acceptor), ("words", WORDS)], ret: Ty::Bool, doc: "Whether `a` and `b` accept the same words of `words`; a failing test lists the words only one of them accepts.", run: agree_on },
        Function { name: "agree_up_to", params: &[("a", Ty::Acceptor), ("b", Ty::Acceptor), ("n", Ty::Int)], ret: Ty::Bool, doc: "Whether `a` and `b` accept the same words of length up to `n` over their symbols, at most 10000 words; a failing test lists the words only one accepts.", run: agree_up_to },
    ],
    explain: Some(explain),
};

// ---- running acceptors ----

fn set(args: &[Value], i: usize) -> &BTreeSet<Value> {
    match &args[i] {
        Value::Set(items) => items,
        other => unreachable!("the checker allowed {other:?} as a Set"),
    }
}

/// The words of argument `i`, a `Set[Str]`.
fn words(args: &[Value], i: usize) -> Vec<&str> {
    set(args, i)
        .iter()
        .map(|w| match w {
            Value::Str(w) => &**w,
            other => unreachable!("the checker allowed {other:?} in a Set[Str]"),
        })
        .collect()
}

/// Asks `acceptor` (argument `which`) about `word`; `arg` is the argument
/// to blame if the word does not fit its alphabet.
fn accepts(args: &[Value], which: usize, word: &str, arg: Option<usize>) -> Result<bool, Failure> {
    langs::accepts(&args[which], word).map_err(|refusal| match refusal {
        Refusal::Foreign { symbol, alphabet } => Failure {
            code: codes::FOREIGN_WORD,
            arg,
            message: format!("the word {word:?} contains `{symbol}`, which is not in the alphabet of argument {}", which + 1),
            hint: format!("that argument is over {}; test it on words over that alphabet", fmt_alphabet(&alphabet)),
        },
        Refusal::PdaGaveUp => Failure {
            code: codes::GAVE_UP,
            arg: Some(which),
            message: format!("the PDA gave up on {word:?} without finding an answer"),
            hint: "eps moves that push without reading can let a PDA run forever; the answer is unknown, not \"no\"".into(),
        },
        Refusal::TmStillRunning => Failure {
            code: codes::STEP_LIMIT,
            arg: Some(which),
            message: format!("the Turing machine is still running on {word:?} after {} steps", crate::automata::tm::STEP_LIMIT),
            hint: "it may never halt; by the halting problem no program can decide that in general".into(),
        },
    })
}

/// The words of `words` on which `machine` answers `wanted`'s opposite.
fn wrong<'a>(args: &[Value], words: &[&'a str], wanted: bool) -> Result<Vec<&'a str>, Failure> {
    let mut wrong = Vec::new();
    for &word in words {
        if accepts(args, 0, word, Some(1))? != wanted {
            wrong.push(word);
        }
    }
    Ok(wrong)
}

/// The words on which `a` and `b` (arguments 0 and 1) differ: those only
/// `a` accepts, and those only `b` accepts.
fn disagreements(args: &[Value], words: &[String], arg: Option<usize>) -> Result<(Vec<String>, Vec<String>), Failure> {
    let (mut only_a, mut only_b) = (Vec::new(), Vec::new());
    for word in words {
        match (accepts(args, 0, word, arg)?, accepts(args, 1, word, arg)?) {
            (true, false) => only_a.push(word.clone()),
            (false, true) => only_b.push(word.clone()),
            _ => {}
        }
    }
    Ok((only_a, only_b))
}

/// Every word of length up to argument 2 over the symbols of arguments 0
/// and 1, shortest first.
fn sample_words(args: &[Value]) -> Result<Vec<String>, Failure> {
    let n = super::int_arg(args, 2);
    if n < 0 {
        return Err(Failure {
            code: codes::STRING_RANGE,
            arg: Some(2),
            message: format!("a length cannot be negative, but this is {n}"),
            hint: "use 0 or more".into(),
        });
    }
    let symbols: Vec<char> = args[..2].iter().flat_map(symbols).collect::<BTreeSet<char>>().into_iter().collect();
    // Count before building: 1 + k + k^2 + ... + k^n words.
    let (mut total, mut level) = (1u128, 1u128);
    for _ in 0..n {
        level *= symbols.len() as u128;
        total += level;
        if total > MAX_WORDS {
            return Err(Failure {
                code: codes::RESULT_TOO_LARGE,
                arg: Some(2),
                message: format!("there are more than {MAX_WORDS} words of length up to {n} over {}", fmt_alphabet(&symbols)),
                hint: format!("use a smaller length; at most {MAX_WORDS} words are tried"),
            });
        }
    }
    let mut all = vec![String::new()];
    let mut layer = vec![String::new()];
    for _ in 0..n {
        layer = layer.iter().flat_map(|w| symbols.iter().map(move |c| format!("{w}{c}"))).collect();
        all.extend(layer.iter().cloned());
    }
    Ok(all)
}

/// The symbols an acceptor works over: its alphabet, or for a regex or a
/// set of words, the symbols it mentions.
fn symbols(acceptor: &Value) -> Vec<char> {
    match acceptor {
        Value::Grammar(g) => g.alphabet.clone(),
        Value::Pda(p) => p.alphabet.clone(),
        Value::Tm(m) => m.alphabet.clone(),
        other => langs::symbols_of(other),
    }
}

// ---- the checks ----

fn equal(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(args[0] == args[1]))
}

fn equal_sets(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(set(args, 0) == set(args, 1)))
}

fn accepts_all(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(wrong(args, &words(args, 1), true)?.is_empty()))
}

fn rejects_all(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(wrong(args, &words(args, 1), false)?.is_empty()))
}

fn same_language(args: &[Value]) -> Result<Value, Failure> {
    let dfas = langs::to_dfas(args)?;
    Ok(Value::Bool(ops::equivalent(&dfas[0], &dfas[1])))
}

fn agree_on(args: &[Value]) -> Result<Value, Failure> {
    let words: Vec<String> = words(args, 2).into_iter().map(String::from).collect();
    let (only_a, only_b) = disagreements(args, &words, Some(2))?;
    Ok(Value::Bool(only_a.is_empty() && only_b.is_empty()))
}

fn agree_up_to(args: &[Value]) -> Result<Value, Failure> {
    let (only_a, only_b) = disagreements(args, &sample_words(args)?, None)?;
    Ok(Value::Bool(only_a.is_empty() && only_b.is_empty()))
}

// ---- explaining a failure ----

/// Words as `"a", "b"`, at most `SHOWN` of them, then how many more.
fn list<S: AsRef<str>>(words: &[S]) -> String {
    let shown: Vec<String> = words.iter().take(SHOWN).map(|w| format!("{:?}", w.as_ref())).collect();
    match words.len().saturating_sub(SHOWN) {
        0 => shown.join(", "),
        more => format!("{}, and {more} more", shown.join(", ")),
    }
}

/// A set's members as the language prints a set.
fn members<'a>(items: impl Iterator<Item = &'a Value>) -> String {
    Value::Set(std::rc::Rc::new(items.cloned().collect())).to_string()
}

/// Why `name` returned `false` for `args`: the lines a failing `test`
/// prints under its own.
fn explain(name: &str, args: &[Value]) -> String {
    explanation(name, args).unwrap_or_default()
}

fn explanation(name: &str, args: &[Value]) -> Result<String, Failure> {
    Ok(match name {
        "equal" => {
            let (expected, actual) = (&args[0], &args[1]);
            let show = |v: &Value| match v {
                Value::Str(s) => format!("{s:?}"),
                other => other.to_string(),
            };
            let mut text = format!("expected: {}\nactual:   {}", show(expected), show(actual));
            if let (Value::Str(e), Value::Str(a)) = (expected, actual) {
                let at = e.chars().zip(a.chars()).take_while(|(x, y)| x == y).count();
                // The caret goes under that character of the quoted
                // `actual`, so count how wide the quoted prefix is.
                let prefix: String = a.chars().take(at).collect();
                let column = "actual:   ".len() + format!("{prefix:?}").chars().count() - 1;
                text.push_str(&format!("\n{}^ first difference, at index {at}", " ".repeat(column)));
            }
            text
        }
        "equal_sets" => {
            let (expected, actual) = (set(args, 0), set(args, 1));
            let mut lines = Vec::new();
            if !expected.is_subset(actual) {
                lines.push(format!("missing:    {}", members(expected.difference(actual))));
            }
            if !actual.is_subset(expected) {
                lines.push(format!("unexpected: {}", members(actual.difference(expected))));
            }
            lines.join("\n")
        }
        "accepts_all" => format!("should accept, but rejects: {}", list(&wrong(args, &words(args, 1), true)?)),
        "rejects_all" => format!("should reject, but accepts: {}", list(&wrong(args, &words(args, 1), false)?)),
        "same_language" => {
            let dfas = langs::to_dfas(args)?;
            let word = ops::distinguishing_word(&dfas[0], &dfas[1]).unwrap_or_default();
            if dfas[0].accepts(&word) == Ok(true) {
                format!("{word:?} is in the expected language but not in the actual one")
            } else {
                format!("{word:?} is in the actual language but not in the expected one")
            }
        }
        "agree_on" | "agree_up_to" => {
            let sample = if name == "agree_on" {
                words(args, 2).into_iter().map(String::from).collect()
            } else {
                sample_words(args)?
            };
            let (only_a, only_b) = disagreements(args, &sample, None)?;
            let mut lines = Vec::new();
            if !only_a.is_empty() {
                lines.push(format!("accepted only by the first: {}", list(&only_a)));
            }
            if !only_b.is_empty() {
                lines.push(format!("accepted only by the second: {}", list(&only_b)));
            }
            lines.join("\n")
        }
        _ => String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::codes;
    use std::rc::Rc;

    fn function(name: &str) -> &'static Function {
        MODULE.functions.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no std.testing.{name}"))
    }

    /// Runs a check that should fail, and returns its explanation.
    fn failing(name: &str, args: &[Value]) -> String {
        assert_eq!((function(name).run)(args), Ok(Value::Bool(false)), "{name} should fail");
        explain(name, args)
    }

    fn passes(name: &str, args: &[Value]) -> bool {
        (function(name).run)(args) == Ok(Value::Bool(true))
    }

    fn s(text: &str) -> Value {
        Value::str(text)
    }

    fn words(items: &[&str]) -> Value {
        Value::Set(Rc::new(items.iter().map(|w| s(w)).collect()))
    }

    fn re(src: &str) -> Value {
        Value::Regex(Rc::new(crate::automata::regex::parse(src).unwrap()))
    }

    #[test]
    fn equal_shows_both_values_and_where_strings_differ() {
        assert!(passes("equal", &[Value::Int(3), Value::Int(3)]));
        assert_eq!(failing("equal", &[Value::Int(3), Value::Int(4)]), "expected: 3\nactual:   4");
        assert_eq!(
            failing("equal", &[s("abcd"), s("abxd")]),
            "expected: \"abcd\"\nactual:   \"abxd\"\n             ^ first difference, at index 2"
        );
        assert_eq!(
            failing("equal", &[s("ab"), s("abc")]),
            "expected: \"ab\"\nactual:   \"abc\"\n             ^ first difference, at index 2"
        );
    }

    #[test]
    fn equal_sets_shows_what_is_missing_and_what_is_extra() {
        assert!(passes("equal_sets", &[words(&["a", "b"]), words(&["b", "a"])]));
        assert_eq!(failing("equal_sets", &[words(&["a", "b"]), words(&["b", "c"])]), "missing:    {\"a\"}\nunexpected: {\"c\"}");
        assert_eq!(failing("equal_sets", &[words(&["a"]), words(&[])]), "missing:    {\"a\"}");
    }

    #[test]
    fn accepting_and_rejecting_sets_of_words() {
        assert!(passes("accepts_all", &[re("a*"), words(&["", "aa"])]));
        assert_eq!(failing("accepts_all", &[re("a*"), words(&["a", "b", "ab"])]), "should accept, but rejects: \"ab\", \"b\"");
        assert!(passes("rejects_all", &[re("a*"), words(&["b"])]));
        assert_eq!(failing("rejects_all", &[re("a*"), words(&["", "b"])]), "should reject, but accepts: \"\"");
        let many: Vec<String> = (0..15).map(|i| "a".repeat(i)).collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        let text = failing("rejects_all", &[re("a*"), words(&many)]);
        assert!(text.ends_with("and 5 more"), "{text}");
    }

    #[test]
    fn same_language_names_a_word_that_differs() {
        assert!(passes("same_language", &[re("(a|b)*"), re("(a*b*)*")]));
        assert_eq!(
            failing("same_language", &[re("a*"), re("a+")]),
            "\"\" is in the expected language but not in the actual one"
        );
        assert_eq!(
            failing("same_language", &[re("ab"), re("ab|b")]),
            "\"b\" is in the actual language but not in the expected one"
        );
    }

    #[test]
    fn two_acceptors_agree_on_words() {
        assert!(passes("agree_on", &[re("a*"), words(&["", "a"]), words(&["a", "b"])]));
        assert_eq!(
            failing("agree_on", &[re("a*"), re("a+|b"), words(&["", "a", "b"])]),
            "accepted only by the first: \"\"\naccepted only by the second: \"b\""
        );
        assert!(passes("agree_up_to", &[re("(ab)*"), re("((ab)*)*"), Value::Int(6)]));
        assert_eq!(failing("agree_up_to", &[re("a*"), re("a?"), Value::Int(3)]), "accepted only by the first: \"aa\", \"aaa\"");
    }

    #[test]
    fn mistakes() {
        let run = |name: &str, args: &[Value]| (function(name).run)(args).unwrap_err().code;
        assert_eq!(run("agree_up_to", &[re("a"), re("b"), Value::Int(-1)]), codes::STRING_RANGE);
        assert_eq!(run("agree_up_to", &[re("ab"), re("ba"), Value::Int(20)]), codes::RESULT_TOO_LARGE);
        let even = Value::dfa(crate::automata::patterns::count_mod(&['0', '1'], '0', 2, 0));
        let f = (function("accepts_all").run)(&[even, words(&["0", "02"])]).unwrap_err();
        assert_eq!((f.code, f.arg), (codes::FOREIGN_WORD, Some(1)));
    }
}
