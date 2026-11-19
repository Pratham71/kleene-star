//! `std.collections`: helpers for sets, and `words`, which lists every string
//! of one length over an alphabet.
//!
//! Sets are the only collection in the language, and they hold `Int`, `Str`
//! or `Bool` values, never other sets. That is why there is no power set
//! and no set of pairs here: both would be sets of sets.
//!
//! Most functions are *generic*: `contains(s: Set[T], x: T)` works for a set
//! of Ints and a set of Strs alike, as long as the arguments agree on `T`.
//! The checker works out `T` from the arguments (see `check_arguments`), so
//! the table only has to write `Ty::Elem`. `min`, `max` and `sum` are not
//! generic: they need numbers.
//!
//! The names `union`, `intersect` and `difference` also belong to the
//! language's built-ins, which combine *languages* into a machine. These are
//! different functions that happen to share a name; you reach them as
//! `std.collections.union(...)`, and `from std.collections import union` is
//! an error because the built-in already owns that name.

use super::{Failure, Function, Module, Ty};
use crate::lang::codes;
use crate::lang::value::Value;
use std::collections::BTreeSet;
use std::rc::Rc;

/// The most words `words` will build. It is a count of words, not of
/// characters: 100000 words of length 10 are a million characters.
pub const MAX_WORDS: u128 = 100_000;

const SET_T: Ty = Ty::Set(&Ty::Elem);
const SET_INT: Ty = Ty::Set(&Ty::Int);

pub const MODULE: Module = Module {
    name: "collections",
    doc: "Sets: membership, subsets, union, intersection, difference, size, smallest, largest and sum, and every word of a given length over an alphabet.",
    functions: &[
        Function { name: "contains", params: &[("s", SET_T), ("x", Ty::Elem)], ret: Ty::Bool, doc: "Whether `x` is a member of the set `s`.", run: contains },
        Function { name: "difference", params: &[("a", SET_T), ("b", SET_T)], ret: SET_T, doc: "The members of the set `a` that are not in `b`.", run: difference },
        Function { name: "intersect", params: &[("a", SET_T), ("b", SET_T)], ret: SET_T, doc: "The members that the sets `a` and `b` have in common.", run: intersect },
        Function { name: "is_subset", params: &[("a", SET_T), ("b", SET_T)], ret: Ty::Bool, doc: "Whether every member of the set `a` is also in `b`; the empty set is a subset of every set.", run: is_subset },
        Function { name: "max", params: &[("s", SET_INT)], ret: Ty::Int, doc: "The largest member of a set of Ints; the set must not be empty.", run: max },
        Function { name: "min", params: &[("s", SET_INT)], ret: Ty::Int, doc: "The smallest member of a set of Ints; the set must not be empty.", run: min },
        Function { name: "size", params: &[("s", SET_T)], ret: Ty::Int, doc: "The number of members of the set `s`; the same as `len(s)`.", run: size },
        Function { name: "sum", params: &[("s", SET_INT)], ret: Ty::Int, doc: "The members of a set of Ints added up; the sum of the empty set is 0.", run: sum },
        Function { name: "union", params: &[("a", SET_T), ("b", SET_T)], ret: SET_T, doc: "The members that are in the set `a`, in `b`, or in both.", run: union },
        Function { name: "words", params: &[("alphabet", Ty::Alphabet), ("n", Ty::Int)], ret: Ty::Set(&Ty::Str), doc: "Every string of length `n` over `alphabet`, as a set; `n` must not be negative, and there may be at most 100000 of them.", run: words },
    ],
    explain: None,
};

fn set(args: &[Value], i: usize) -> &BTreeSet<Value> {
    match &args[i] {
        Value::Set(items) => items,
        other => unreachable!("the checker allowed {other:?} as a Set"),
    }
}

fn ints(args: &[Value]) -> Vec<i64> {
    set(args, 0)
        .iter()
        .map(|v| match v {
            Value::Int(n) => *n,
            other => unreachable!("the checker allowed {other:?} in a Set[Int]"),
        })
        .collect()
}

fn new_set(items: BTreeSet<Value>) -> Value {
    Value::Set(Rc::new(items))
}

fn contains(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(set(args, 0).contains(&args[1])))
}

fn is_subset(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(set(args, 0).is_subset(set(args, 1))))
}

fn union(args: &[Value]) -> Result<Value, Failure> {
    Ok(new_set(set(args, 0).union(set(args, 1)).cloned().collect()))
}

fn intersect(args: &[Value]) -> Result<Value, Failure> {
    Ok(new_set(set(args, 0).intersection(set(args, 1)).cloned().collect()))
}

fn difference(args: &[Value]) -> Result<Value, Failure> {
    Ok(new_set(set(args, 0).difference(set(args, 1)).cloned().collect()))
}

fn size(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Int(set(args, 0).len() as i64))
}

fn empty_set(what: &str) -> Failure {
    Failure {
        code: codes::EMPTY_SET,
        arg: Some(0),
        message: format!("the {what} of an empty set is not defined"),
        hint: format!("check `size(s) > 0` before asking for the {what}"),
    }
}

fn min(args: &[Value]) -> Result<Value, Failure> {
    ints(args).into_iter().min().map(Value::Int).ok_or_else(|| empty_set("smallest member"))
}

fn max(args: &[Value]) -> Result<Value, Failure> {
    ints(args).into_iter().max().map(Value::Int).ok_or_else(|| empty_set("largest member"))
}

fn sum(args: &[Value]) -> Result<Value, Failure> {
    // `try_fold` stops at the first addition that does not fit.
    ints(args).into_iter().try_fold(0i64, i64::checked_add).map(Value::Int).ok_or_else(|| Failure {
        code: codes::OVERFLOW,
        arg: Some(0),
        message: "the sum is too large for an Int".into(),
        hint: "Ints are 64-bit: about plus or minus 9 * 10^18".into(),
    })
}

/// Every string of length `n` over the alphabet. The count, `size^n`, is
/// worked out before anything is built, so a request that is too big is an
/// error instead of a hang.
fn words(args: &[Value]) -> Result<Value, Failure> {
    let (Value::Alphabet(symbols), Value::Int(n)) = (&args[0], &args[1]) else {
        unreachable!("the checker allowed {args:?} for (Alphabet, Int)")
    };
    if *n < 0 {
        return Err(Failure {
            code: codes::STRING_RANGE,
            arg: Some(1),
            message: format!("there are no strings of length {n}"),
            hint: "the length must be 0 or more; the only string of length 0 is the empty string".into(),
        });
    }
    let size = symbols.len() as u128;
    // With no symbols there is no word of positive length, and with one
    // symbol there is exactly one word, `n` symbols long. Both would make
    // the loops below run `n` times, which could be 10^18, so they are
    // answered directly.
    if size <= 1 {
        let too_long = *n as u128 > super::string::MAX_RESULT_CHARS;
        return match (size, *n) {
            (0, 0) => Ok(new_set(BTreeSet::from([Value::str("")]))),
            (0, _) => Ok(new_set(BTreeSet::new())),
            (_, _) if too_long => Err(Failure {
                code: codes::RESULT_TOO_LARGE,
                arg: Some(1),
                message: format!("a word of length {n} is longer than the limit of {} characters", super::string::MAX_RESULT_CHARS),
                hint: "ask for shorter words".into(),
            }),
            _ => Ok(new_set(BTreeSet::from([Value::str(&symbols[0].to_string().repeat(*n as usize))]))),
        };
    }
    // Multiply one step at a time and stop as soon as the limit is passed,
    // so `words(B, 1000000)` fails after 17 steps, not after a million.
    let mut count: u128 = 1;
    for _ in 0..*n {
        count *= size;
        if count > MAX_WORDS {
            return Err(Failure {
                code: codes::RESULT_TOO_LARGE,
                arg: None,
                message: format!(
                    "the {size} symbol{} would make more than {MAX_WORDS} words of length {n}, which is the limit",
                    if size == 1 { "" } else { "s" }
                ),
                hint: format!("ask for shorter words (at most {MAX_WORDS} are built), or reason about the language without listing it"),
            });
        }
    }
    // Build the words one position at a time: all words of length k + 1 are
    // the words of length k with each symbol added at the end.
    let mut level = vec![String::new()];
    for _ in 0..*n {
        level = level.iter().flat_map(|w| symbols.iter().map(move |c| format!("{w}{c}"))).collect();
    }
    Ok(new_set(level.iter().map(|w| Value::str(w)).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &[Value]) -> Result<Value, Failure> {
        let f = MODULE.functions.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no std.collections.{name}"));
        (f.run)(args)
    }

    fn ok(name: &str, args: &[Value]) -> Value {
        call(name, args).unwrap_or_else(|e| panic!("{name}{args:?} failed: {}", e.message))
    }

    fn code(name: &str, args: &[Value]) -> &'static str {
        call(name, args).unwrap_err().code
    }

    fn ints_set(items: &[i64]) -> Value {
        new_set(items.iter().map(|&n| Value::Int(n)).collect())
    }

    fn strs(items: &[&str]) -> Value {
        new_set(items.iter().map(|w| Value::str(w)).collect())
    }

    fn alphabet(symbols: &str) -> Value {
        Value::Alphabet(Rc::new(symbols.chars().collect()))
    }

    #[test]
    fn membership_and_subsets() {
        let s = ints_set(&[1, 2, 3]);
        assert_eq!(ok("contains", &[s.clone(), Value::Int(2)]), Value::Bool(true));
        assert_eq!(ok("contains", &[s.clone(), Value::Int(9)]), Value::Bool(false));
        assert_eq!(ok("contains", &[strs(&[]), Value::str("")]), Value::Bool(false));
        assert_eq!(ok("contains", &[strs(&[""]), Value::str("")]), Value::Bool(true));
        assert_eq!(ok("is_subset", &[ints_set(&[1, 3]), s.clone()]), Value::Bool(true));
        assert_eq!(ok("is_subset", &[s.clone(), ints_set(&[1, 3])]), Value::Bool(false));
        assert_eq!(ok("is_subset", &[ints_set(&[]), ints_set(&[])]), Value::Bool(true));
        assert_eq!(ok("is_subset", &[ints_set(&[]), s.clone()]), Value::Bool(true));
        assert_eq!(ok("is_subset", &[s.clone(), s]), Value::Bool(true));
    }

    #[test]
    fn combining() {
        let (a, b) = (ints_set(&[1, 2, 3]), ints_set(&[3, 4]));
        assert_eq!(ok("union", &[a.clone(), b.clone()]), ints_set(&[1, 2, 3, 4]));
        assert_eq!(ok("intersect", &[a.clone(), b.clone()]), ints_set(&[3]));
        assert_eq!(ok("difference", &[a.clone(), b.clone()]), ints_set(&[1, 2]));
        assert_eq!(ok("difference", &[b.clone(), a.clone()]), ints_set(&[4]));
        assert_eq!(ok("union", &[a.clone(), ints_set(&[])]), a);
        assert_eq!(ok("intersect", &[a.clone(), ints_set(&[])]), ints_set(&[]));
        assert_eq!(ok("difference", &[a.clone(), a]), ints_set(&[]));
        assert_eq!(ok("union", &[strs(&["a"]), strs(&["", "b"])]), strs(&["", "a", "b"]));
    }

    #[test]
    fn numbers_in_a_set() {
        let s = ints_set(&[4, -2, 9]);
        assert_eq!(ok("size", std::slice::from_ref(&s)), Value::Int(3));
        assert_eq!(ok("size", &[strs(&[])]), Value::Int(0));
        assert_eq!(ok("min", std::slice::from_ref(&s)), Value::Int(-2));
        assert_eq!(ok("max", std::slice::from_ref(&s)), Value::Int(9));
        assert_eq!(ok("sum", std::slice::from_ref(&s)), Value::Int(11));
        assert_eq!(ok("sum", &[ints_set(&[])]), Value::Int(0));
        assert_eq!(ok("min", &[ints_set(&[7])]), Value::Int(7));
        assert_eq!(code("min", &[ints_set(&[])]), codes::EMPTY_SET);
        assert_eq!(code("max", &[ints_set(&[])]), codes::EMPTY_SET);
        assert_eq!(code("sum", &[ints_set(&[i64::MAX, 1])]), codes::OVERFLOW);
        assert_eq!(ok("sum", &[ints_set(&[i64::MAX, -1])]), Value::Int(i64::MAX - 1));
    }

    fn word_list(set: Value) -> Vec<String> {
        match set {
            Value::Set(items) => items.iter().map(|v| v.to_string().trim_matches('"').to_string()).collect(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn every_word_of_a_length() {
        let two = ok("words", &[alphabet("01"), Value::Int(2)]);
        assert_eq!(word_list(two), ["00", "01", "10", "11"]);
        assert_eq!(ok("words", &[alphabet("01"), Value::Int(0)]), strs(&[""]));
        assert_eq!(ok("words", &[alphabet(""), Value::Int(0)]), strs(&[""]));
        assert_eq!(ok("words", &[alphabet(""), Value::Int(3)]), strs(&[]));
        assert_eq!(ok("words", &[alphabet("a"), Value::Int(3)]), strs(&["aaa"]));
        let Value::Set(three) = ok("words", &[alphabet("abc"), Value::Int(4)]) else { panic!() };
        assert_eq!(three.len(), 81);
        assert_eq!(code("words", &[alphabet("01"), Value::Int(-1)]), codes::STRING_RANGE);
    }

    #[test]
    fn the_word_limit() {
        // 2^16 = 65536 fits; 2^17 does not.
        let Value::Set(ok16) = ok("words", &[alphabet("01"), Value::Int(16)]) else { panic!() };
        assert_eq!(ok16.len(), 65_536);
        let failure = call("words", &[alphabet("01"), Value::Int(17)]).unwrap_err();
        assert_eq!(failure.code, codes::RESULT_TOO_LARGE);
        assert!(failure.message.contains("100000"), "the message names the limit: {}", failure.message);
        // Fails fast, without building anything or overflowing.
        assert_eq!(code("words", &[alphabet("0123456789"), Value::Int(i64::MAX)]), codes::RESULT_TOO_LARGE);
        // A one-symbol alphabet has one word, so only its length is limited.
        assert_eq!(ok("words", &[alphabet("a"), Value::Int(5)]), strs(&["aaaaa"]));
        assert_eq!(code("words", &[alphabet("a"), Value::Int(i64::MAX)]), codes::RESULT_TOO_LARGE);
    }

    #[test]
    fn the_empty_alphabet_has_no_long_words_even_for_huge_lengths() {
        assert_eq!(ok("words", &[alphabet(""), Value::Int(i64::MAX)]), strs(&[]));
    }
}
