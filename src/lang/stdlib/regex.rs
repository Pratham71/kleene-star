//! `std.regex`: regular expressions as values that can be built at run time
//! and put together from pieces, staying regexes all the way.
//!
//! The built-ins already *convert* and *combine* languages, but they turn
//! regexes into machines: `concat(/a/, /b/)` is an `Nfa`, `star(/a/)` too.
//! These functions keep a `Regex` a `Regex`, so the result prints as a
//! regex, can be shown to a student, and can be built from a string read at
//! run time (`parse`). To go the other way, from a machine back to a regex,
//! use the built-in `to_regex`.

use super::{Failure, Function, Module, Ty};
use crate::automata::regex::{self, Regex};
use crate::lang::codes;
use crate::lang::value::Value;
use std::rc::Rc;

pub const MODULE: Module = Module {
    name: "regex",
    doc: "Regular expressions built at run time and put together from pieces: from text, from a word or an alphabet, by choice, sequence and repetition, and the common prefix, suffix and substring forms.",
    functions: &[
        Function { name: "parse", params: &[("text", Ty::Str)], ret: Ty::Regex, doc: "The regex written in `text`, in the same syntax as between slashes; text that is not a regex is an error.", run: parse },
        Function { name: "literal", params: &[("word", Ty::Str)], ret: Ty::Regex, doc: "The regex that matches exactly `word` and nothing else.", run: literal },
        Function { name: "any_of", params: &[("alphabet", Ty::Alphabet)], ret: Ty::Regex, doc: "The regex for any one symbol of `alphabet`, written out as a choice between them.", run: any_of },
        Function { name: "alt", params: &[("a", Ty::Regex), ("b", Ty::Regex)], ret: Ty::Regex, doc: "The choice of `a` or `b`: the words of either one.", run: alt },
        Function { name: "seq", params: &[("a", Ty::Regex), ("b", Ty::Regex)], ret: Ty::Regex, doc: "`ab`: a word of `a` followed by a word of `b`.", run: seq },
        Function { name: "zero_or_more", params: &[("r", Ty::Regex)], ret: Ty::Regex, doc: "`r*`: any number of words of `r` one after another, including none.", run: zero_or_more },
        Function { name: "one_or_more", params: &[("r", Ty::Regex)], ret: Ty::Regex, doc: "`r+`: one or more words of `r` one after another.", run: one_or_more },
        Function { name: "optional", params: &[("r", Ty::Regex)], ret: Ty::Regex, doc: "`r?`: a word of `r`, or the empty string.", run: optional },
        Function { name: "power", params: &[("r", Ty::Regex), ("n", Ty::Int)], ret: Ty::Regex, doc: "Exactly `n` words of `r` one after another; `power(r, 0)` matches only the empty string.", run: power },
        Function { name: "repeat_between", params: &[("r", Ty::Regex), ("min", Ty::Int), ("max", Ty::Int)], ret: Ty::Regex, doc: "From `min` to `max` words of `r` one after another, both included.", run: repeat_between },
        Function { name: "at_least", params: &[("r", Ty::Regex), ("n", Ty::Int)], ret: Ty::Regex, doc: "`n` or more words of `r` one after another.", run: at_least },
        Function { name: "starts_with", params: &[("alphabet", Ty::Alphabet), ("prefix", Ty::Str)], ret: Ty::Regex, doc: "The regex for the strings over `alphabet` that begin with `prefix`.", run: starts_with },
        Function { name: "ends_with", params: &[("alphabet", Ty::Alphabet), ("suffix", Ty::Str)], ret: Ty::Regex, doc: "The regex for the strings over `alphabet` that end with `suffix`.", run: ends_with },
        Function { name: "contains", params: &[("alphabet", Ty::Alphabet), ("part", Ty::Str)], ret: Ty::Regex, doc: "The regex for the strings over `alphabet` that contain `part` somewhere.", run: contains },
        Function { name: "size", params: &[("r", Ty::Regex)], ret: Ty::Int, doc: "The number of parts of the regex: its symbols, operators and joins.", run: size },
        Function { name: "text", params: &[("r", Ty::Regex)], ret: Ty::Str, doc: "The regex written out as a string, without the slashes.", run: text },
    ],
};

/// The largest regex these functions build, in parts (`Regex::size`): the
/// same limit as for a regex the parser reads.
pub const MAX_SIZE: usize = regex::MAX_SIZE;

// ---- reading the arguments ----

fn re(args: &[Value], i: usize) -> &Regex {
    match &args[i] {
        Value::Regex(r) => r,
        other => unreachable!("the checker allowed {other:?} as a Regex"),
    }
}

fn text_arg(args: &[Value], i: usize) -> &str {
    match &args[i] {
        Value::Str(s) => s,
        other => unreachable!("the checker allowed {other:?} as a Str"),
    }
}

fn alphabet(args: &[Value]) -> &[char] {
    match &args[0] {
        Value::Alphabet(symbols) => symbols,
        other => unreachable!("the checker allowed {other:?} as an Alphabet"),
    }
}

/// Argument `i`: a number of copies, 0 or more.
fn count(args: &[Value], i: usize) -> Result<u128, Failure> {
    match &args[i] {
        Value::Int(n) if *n >= 0 => Ok(*n as u128),
        Value::Int(n) => Err(Failure {
            code: codes::STRING_RANGE,
            arg: Some(i),
            message: format!("a number of copies cannot be negative, but this is {n}"),
            hint: "use 0 or more".into(),
        }),
        other => unreachable!("the checker allowed {other:?} as an Int"),
    }
}

/// Fails unless a result of at most `bound` parts fits. Every caller works
/// out the bound *before* building, so `power(r, 1000000000)` fails at once
/// instead of building a billion copies first.
fn fits(bound: u128) -> Result<(), Failure> {
    if bound > MAX_SIZE as u128 {
        return Err(Failure {
            code: codes::RESULT_TOO_LARGE,
            arg: None,
            message: format!("the regex would have more than {MAX_SIZE} parts"),
            hint: "build a smaller regex, or work with a machine (`to_dfa`) instead".into(),
        });
    }
    Ok(())
}

fn size_of(args: &[Value], i: usize) -> u128 {
    re(args, i).size() as u128
}

fn value(r: Regex) -> Result<Value, Failure> {
    Ok(Value::Regex(Rc::new(r)))
}

// ---- building ----

fn parse(args: &[Value]) -> Result<Value, Failure> {
    let parsed = regex::parse(text_arg(args, 0)).map_err(|(_, message)| Failure {
        code: codes::BAD_REGEX,
        arg: Some(0),
        message: format!("in regex: {message}"),
        hint: "a regex is made of symbols, `|` for or, `*` for repeat, and `( )` for grouping, as in \"(0|1)*01\"".into(),
    })?;
    value(parsed)
}

fn literal(args: &[Value]) -> Result<Value, Failure> {
    let word = text_arg(args, 0);
    fits(2 * word.chars().count() as u128)?;
    value(regex::literal(word))
}

fn any_of(args: &[Value]) -> Result<Value, Failure> {
    value(regex::any_of(alphabet(args)))
}

fn alt(args: &[Value]) -> Result<Value, Failure> {
    fits(size_of(args, 0) + size_of(args, 1) + 1)?;
    value(Regex::alt(re(args, 0).clone(), re(args, 1).clone()))
}

fn seq(args: &[Value]) -> Result<Value, Failure> {
    fits(size_of(args, 0) + size_of(args, 1) + 1)?;
    value(Regex::concat(re(args, 0).clone(), re(args, 1).clone()))
}

fn zero_or_more(args: &[Value]) -> Result<Value, Failure> {
    fits(size_of(args, 0) + 1)?;
    value(Regex::star(re(args, 0).clone()))
}

fn one_or_more(args: &[Value]) -> Result<Value, Failure> {
    // `r+` is written `r r*`, so it holds two copies of `r`.
    fits(2 * size_of(args, 0) + 2)?;
    value(Regex::plus(re(args, 0).clone()))
}

fn optional(args: &[Value]) -> Result<Value, Failure> {
    fits(size_of(args, 0) + 2)?;
    value(Regex::optional(re(args, 0).clone()))
}

fn power(args: &[Value]) -> Result<Value, Failure> {
    let n = count(args, 1)?;
    fits((size_of(args, 0) + 1) * n)?;
    value(regex::power(re(args, 0), n as usize))
}

fn repeat_between(args: &[Value]) -> Result<Value, Failure> {
    let (min, max) = (count(args, 1)?, count(args, 2)?);
    if min > max {
        return Err(Failure {
            code: codes::MATH_DOMAIN,
            arg: None,
            message: format!("there is no number of copies from {min} to {max}"),
            hint: "`min` must not be larger than `max`".into(),
        });
    }
    fits((size_of(args, 0) + 3) * max)?;
    value(regex::repeat_between(re(args, 0), min as usize, max as usize))
}

fn at_least(args: &[Value]) -> Result<Value, Failure> {
    let n = count(args, 1)?;
    fits((size_of(args, 0) + 1) * (n + 1) + 1)?;
    value(regex::at_least(re(args, 0), n as usize))
}

/// For the prefix, suffix and substring forms: argument 1 as a regex (every
/// symbol must be in the alphabet), and the regex for any string over the
/// alphabet, `(0|1)*`.
fn word_and_anything(args: &[Value]) -> Result<(Regex, Regex), Failure> {
    let symbols = alphabet(args);
    let word = text_arg(args, 1);
    super::word_over(word, symbols, 1, "the word")?;
    fits(2 * word.chars().count() as u128 + 4 * symbols.len() as u128 + 4)?;
    Ok((regex::literal(word), Regex::star(regex::any_of(symbols))))
}

fn starts_with(args: &[Value]) -> Result<Value, Failure> {
    let (word, anything) = word_and_anything(args)?;
    value(Regex::concat(word, anything))
}

fn ends_with(args: &[Value]) -> Result<Value, Failure> {
    let (word, anything) = word_and_anything(args)?;
    value(Regex::concat(anything, word))
}

fn contains(args: &[Value]) -> Result<Value, Failure> {
    let (word, anything) = word_and_anything(args)?;
    value(Regex::concat(Regex::concat(anything.clone(), word), anything))
}

// ---- looking at a regex ----

fn size(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Int(re(args, 0).size() as i64))
}

fn text(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::str(&re(args, 0).to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &[Value]) -> Result<Value, Failure> {
        let f = MODULE.functions.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no std.regex.{name}"));
        (f.run)(args)
    }

    /// The regex a call returns, as text.
    fn ok(name: &str, args: &[Value]) -> String {
        match call(name, args) {
            Ok(Value::Regex(r)) => r.to_string(),
            Ok(other) => other.to_string(),
            Err(e) => panic!("{name}{args:?} failed: {}", e.message),
        }
    }

    fn fails(name: &str, args: &[Value]) -> Failure {
        call(name, args).expect_err("expected a failure")
    }

    fn r(src: &str) -> Value {
        Value::Regex(Rc::new(regex::parse(src).unwrap()))
    }

    fn s(text: &str) -> Value {
        Value::str(text)
    }

    fn b() -> Value {
        Value::Alphabet(Rc::new(vec!['0', '1']))
    }

    #[test]
    fn building_from_text_words_and_alphabets() {
        assert_eq!(ok("parse", &[s("(0|1)*01")]), "(0|1)*01");
        assert_eq!(ok("literal", &[s("abc")]), "abc");
        assert_eq!(ok("literal", &[s("")]), "()");
        assert_eq!(ok("any_of", &[b()]), "0|1");
        assert_eq!(ok("any_of", &[Value::Alphabet(Rc::new(vec![]))]), "[]");
    }

    #[test]
    fn combining() {
        assert_eq!(ok("alt", &[r("ab"), r("c")]), "ab|c");
        assert_eq!(ok("seq", &[r("a|b"), r("c")]), "(a|b)c");
        assert_eq!(ok("zero_or_more", &[r("ab")]), "(ab)*");
        assert_eq!(ok("one_or_more", &[r("a")]), "aa*");
        assert_eq!(ok("optional", &[r("ab")]), "(ab)?");
        assert_eq!(ok("power", &[r("ab"), Value::Int(3)]), "ababab");
        assert_eq!(ok("power", &[r("ab"), Value::Int(0)]), "()");
        assert_eq!(ok("repeat_between", &[r("a"), Value::Int(1), Value::Int(3)]), "a(aa?)?");
        assert_eq!(ok("at_least", &[r("a"), Value::Int(2)]), "aaa*");
    }

    #[test]
    fn common_forms() {
        assert_eq!(ok("starts_with", &[b(), s("01")]), "01(0|1)*");
        assert_eq!(ok("ends_with", &[b(), s("01")]), "(0|1)*01");
        assert_eq!(ok("contains", &[b(), s("11")]), "(0|1)*11(0|1)*");
    }

    #[test]
    fn looking_at_a_regex() {
        assert_eq!(call("size", &[r("(0|1)*01")]).unwrap(), Value::Int(8));
        assert_eq!(call("text", &[r("(0|1)*01")]).unwrap(), s("(0|1)*01"));
    }

    #[test]
    fn mistakes() {
        let f = fails("parse", &[s("(0|1")]);
        assert_eq!((f.code, f.arg), (codes::BAD_REGEX, Some(0)));
        assert!(f.message.contains("never closed"), "{}", f.message);
        assert_eq!(fails("power", &[r("a"), Value::Int(-1)]).code, codes::STRING_RANGE);
        assert_eq!(fails("power", &[r("ab"), Value::Int(1_000_000_000)]).code, codes::RESULT_TOO_LARGE);
        assert_eq!(fails("power", &[r("ab"), Value::Int(i64::MAX)]).code, codes::RESULT_TOO_LARGE);
        assert_eq!(fails("repeat_between", &[r("a"), Value::Int(3), Value::Int(1)]).code, codes::MATH_DOMAIN);
        assert_eq!(fails("at_least", &[r("a"), Value::Int(-2)]).code, codes::STRING_RANGE);
        let f = fails("ends_with", &[b(), s("012")]);
        assert_eq!((f.code, f.arg), (codes::FOREIGN_WORD, Some(1)));
        assert_eq!(fails("literal", &[s(&"a".repeat(30_000))]).code, codes::RESULT_TOO_LARGE);
        // Joining two big regexes is checked too.
        let big = call("power", &[r("a"), Value::Int(9_000)]).unwrap();
        assert_eq!(fails("seq", &[big.clone(), big]).code, codes::RESULT_TOO_LARGE);
    }
}
