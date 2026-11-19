//! `std.string`: working with the words of a language.
//!
//! A `Str` is a sequence of characters, and every index here counts
//! *characters*, not bytes, so `"é"` has length 1 and `char_at("aé", 1)` is
//! `"é"`. (Rust's own string functions count bytes; we convert once, to a
//! list of `char`s, and index that.)
//!
//! Three kinds of mistake are errors with a hint, never a panic or a wrong
//! answer: an index or count outside what makes sense (`STRING_RANGE`), text
//! that is not a number (`NOT_A_NUMBER`), and a result too big to build
//! (`RESULT_TOO_LARGE`). The last one matters because `repeat("ab", 10^12)`
//! would otherwise run until the machine ran out of memory.

use super::{Failure, Function, Module, Ty};
use crate::lang::codes;
use crate::lang::value::Value;
use std::collections::BTreeSet;
use std::rc::Rc;

/// The most characters a library function will build in one result. For a
/// set of strings, the characters of all its members are added up.
pub const MAX_RESULT_CHARS: u128 = 1_000_000;

const STR_SET: Ty = Ty::Set(&Ty::Str);

pub const MODULE: Module = Module {
    name: "string",
    doc: "Words as data: searching, slicing, repeating, and the prefixes, suffixes and substrings of a word.",
    functions: &[
        Function { name: "char_at", params: &[("s", Ty::Str), ("i", Ty::Int)], ret: Ty::Str, doc: "The character of `s` at index `i`, counting from 0, as a string of length 1.", run: char_at },
        Function { name: "contains", params: &[("s", Ty::Str), ("part", Ty::Str)], ret: Ty::Bool, doc: "Whether `part` occurs anywhere inside `s`; the empty string occurs in every string.", run: contains },
        Function { name: "count", params: &[("s", Ty::Str), ("part", Ty::Str)], ret: Ty::Int, doc: "How many times `part` occurs in `s`, not overlapping: `count(\"aaaa\", \"aa\")` is 2; `part` must not be empty.", run: count },
        Function { name: "ends_with", params: &[("s", Ty::Str), ("suffix", Ty::Str)], ret: Ty::Bool, doc: "Whether `s` ends with `suffix`.", run: ends_with },
        Function { name: "from_int", params: &[("n", Ty::Int)], ret: Ty::Str, doc: "The digits of `n` as a string, with a leading `-` if it is negative.", run: from_int },
        Function { name: "index_of", params: &[("s", Ty::Str), ("part", Ty::Str)], ret: Ty::Int, doc: "The index of the first occurrence of `part` in `s`, or -1 if there is none.", run: index_of },
        Function { name: "is_palindrome", params: &[("s", Ty::Str)], ret: Ty::Bool, doc: "Whether `s` reads the same backwards; the empty string does.", run: is_palindrome },
        Function { name: "length", params: &[("s", Ty::Str)], ret: Ty::Int, doc: "The number of characters in `s`; the same as `len(s)`.", run: length },
        Function { name: "prefixes", params: &[("s", Ty::Str)], ret: STR_SET, doc: "Every prefix of `s`, including the empty string and `s` itself.", run: prefixes },
        Function { name: "repeat", params: &[("s", Ty::Str), ("n", Ty::Int)], ret: Ty::Str, doc: "`s` written `n` times in a row; `n` must not be negative, and the result at most 1000000 characters.", run: repeat },
        Function { name: "reverse", params: &[("s", Ty::Str)], ret: Ty::Str, doc: "The characters of `s` in the opposite order.", run: reverse },
        Function { name: "starts_with", params: &[("s", Ty::Str), ("prefix", Ty::Str)], ret: Ty::Bool, doc: "Whether `s` starts with `prefix`.", run: starts_with },
        Function { name: "substring", params: &[("s", Ty::Str), ("start", Ty::Int), ("end", Ty::Int)], ret: Ty::Str, doc: "The characters of `s` from index `start` up to, but not including, index `end`; needs `0 <= start <= end <= length(s)`.", run: substring },
        Function { name: "substrings", params: &[("s", Ty::Str)], ret: STR_SET, doc: "Every substring of `s` (a run of neighbouring characters), including the empty string and `s` itself.", run: substrings },
        Function { name: "suffixes", params: &[("s", Ty::Str)], ret: STR_SET, doc: "Every suffix of `s`, including the empty string and `s` itself.", run: suffixes },
        Function { name: "symbols", params: &[("s", Ty::Str)], ret: STR_SET, doc: "The distinct characters of `s`, each as a string of length 1; the empty string is not one of them.", run: symbols },
        Function { name: "to_int", params: &[("s", Ty::Str)], ret: Ty::Int, doc: "The number written in `s`: digits with an optional leading `-` or `+`, and nothing else.", run: to_int },
    ],
};

/// The text of argument `i`. The checker has made sure it is a `Str`.
fn text(args: &[Value], i: usize) -> &str {
    match &args[i] {
        Value::Str(s) => s,
        other => unreachable!("the checker allowed {other:?} as a Str"),
    }
}

/// The number in argument `i`. The checker has made sure it is an `Int`.
fn int(args: &[Value], i: usize) -> i64 {
    match &args[i] {
        Value::Int(n) => *n,
        other => unreachable!("the checker allowed {other:?} as an Int"),
    }
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn length_of(s: &str) -> i64 {
    s.chars().count() as i64
}

fn string(chars: &[char]) -> Value {
    Value::str(&chars.iter().collect::<String>())
}

fn set(words: impl IntoIterator<Item = Value>) -> Value {
    Value::Set(Rc::new(words.into_iter().collect::<BTreeSet<_>>()))
}

fn out_of_range(arg: usize, message: String, hint: String) -> Failure {
    Failure { code: codes::STRING_RANGE, arg: Some(arg), message, hint }
}

/// Refuses a result with more than `MAX_RESULT_CHARS` characters in all.
/// `chars` is computed by the caller from the *sizes*, before anything is
/// built, so that a huge request is an error and not a hang.
fn check_size(what: &str, chars: u128) -> Result<(), Failure> {
    if chars > MAX_RESULT_CHARS {
        return Err(Failure {
            code: codes::RESULT_TOO_LARGE,
            arg: None,
            message: format!("{what} would be too large to build (over {MAX_RESULT_CHARS} characters)"),
            hint: "use a shorter string, or reason about the language instead of building every word".into(),
        });
    }
    Ok(())
}

fn length(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Int(length_of(text(args, 0))))
}

fn reverse(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::str(&text(args, 0).chars().rev().collect::<String>()))
}

fn repeat(args: &[Value]) -> Result<Value, Failure> {
    let (s, n) = (text(args, 0), int(args, 1));
    if n < 0 {
        return Err(out_of_range(1, format!("cannot repeat a string {n} times"), "the count must be 0 or more; `repeat(s, 0)` is the empty string".into()));
    }
    let total = s.chars().count() as u128 * n as u128;
    check_size(&format!("`repeat` of a string of length {} by {n}", s.chars().count()), total)?;
    Ok(Value::str(&s.repeat(n as usize)))
}

fn contains(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(text(args, 0).contains(text(args, 1))))
}

fn starts_with(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(text(args, 0).starts_with(text(args, 1))))
}

fn ends_with(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Bool(text(args, 0).ends_with(text(args, 1))))
}

fn index_of(args: &[Value]) -> Result<Value, Failure> {
    let (s, part) = (text(args, 0), text(args, 1));
    // `find` gives a byte position; the language counts characters.
    Ok(Value::Int(s.find(part).map_or(-1, |byte| s[..byte].chars().count() as i64)))
}

fn count(args: &[Value]) -> Result<Value, Failure> {
    let (s, part) = (text(args, 0), text(args, 1));
    if part.is_empty() {
        return Err(out_of_range(
            1,
            "cannot count the empty string".into(),
            "the empty string sits between every two characters; count a string with at least one character".into(),
        ));
    }
    // `matches` skips past each match, so overlapping ones are not counted.
    Ok(Value::Int(s.matches(part).count() as i64))
}

fn char_at(args: &[Value]) -> Result<Value, Failure> {
    let (s, i) = (chars(text(args, 0)), int(args, 1));
    match usize::try_from(i).ok().and_then(|i| s.get(i)) {
        Some(c) => Ok(Value::str(&c.to_string())),
        None => Err(out_of_range(
            1,
            format!("index {i} is outside a string of length {}", s.len()),
            if s.is_empty() { "the empty string has no characters".into() } else { format!("indices run from 0 to {}", s.len() - 1) },
        )),
    }
}

fn substring(args: &[Value]) -> Result<Value, Failure> {
    let (s, start, end) = (chars(text(args, 0)), int(args, 1), int(args, 2));
    let len = s.len() as i64;
    if start < 0 || start > len {
        return Err(out_of_range(1, format!("start {start} is outside a string of length {len}"), format!("`start` runs from 0 to {len}")));
    }
    if end < start || end > len {
        return Err(out_of_range(
            2,
            format!("end {end} is not between start ({start}) and the length ({len})"),
            format!("`end` runs from `start` to {len}, and the character at `end` is not included"),
        ));
    }
    Ok(string(&s[start as usize..end as usize]))
}

fn is_palindrome(args: &[Value]) -> Result<Value, Failure> {
    let s = chars(text(args, 0));
    Ok(Value::Bool(s.iter().eq(s.iter().rev())))
}

fn from_int(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::str(&int(args, 0).to_string()))
}

fn to_int(args: &[Value]) -> Result<Value, Failure> {
    let s = text(args, 0);
    let digits = s.strip_prefix(['-', '+']).unwrap_or(s);
    // `str::parse` also accepts a leading `+`; we only let through plain
    // ASCII digits so that "1_000" and " 7" are errors, not surprises.
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(Failure {
            code: codes::NOT_A_NUMBER,
            arg: Some(0),
            message: format!("{s:?} is not a whole number"),
            hint: "`to_int` takes digits with an optional leading `-` or `+`, such as \"42\" or \"-7\"".into(),
        });
    }
    s.parse::<i64>().map(Value::Int).map_err(|_| Failure {
        code: codes::OVERFLOW,
        arg: Some(0),
        message: format!("{s:?} is too large for an Int"),
        hint: "Ints are 64-bit: about plus or minus 9 * 10^18".into(),
    })
}

fn symbols(args: &[Value]) -> Result<Value, Failure> {
    Ok(set(text(args, 0).chars().map(|c| Value::str(&c.to_string()))))
}

fn prefixes(args: &[Value]) -> Result<Value, Failure> {
    let s = chars(text(args, 0));
    let n = s.len() as u128;
    check_size("the prefixes", n * (n + 1) / 2)?;
    Ok(set((0..=s.len()).map(|end| string(&s[..end]))))
}

fn suffixes(args: &[Value]) -> Result<Value, Failure> {
    let s = chars(text(args, 0));
    let n = s.len() as u128;
    check_size("the suffixes", n * (n + 1) / 2)?;
    Ok(set((0..=s.len()).map(|start| string(&s[start..]))))
}

fn substrings(args: &[Value]) -> Result<Value, Failure> {
    let s = chars(text(args, 0));
    let n = s.len() as u128;
    // There are n(n+1)/2 runs of neighbouring characters, of total length
    // n(n+1)(n+2)/6 (a run of length k can start in n-k+1 places).
    check_size("the substrings", n * (n + 1) * (n + 2) / 6)?;
    let mut all = vec![Value::str("")];
    for start in 0..s.len() {
        for end in start + 1..=s.len() {
            all.push(string(&s[start..end]));
        }
    }
    Ok(set(all))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &[Value]) -> Result<Value, Failure> {
        let f = MODULE.functions.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no std.string.{name}"));
        (f.run)(args)
    }

    fn s(text: &str) -> Value {
        Value::str(text)
    }

    fn n(i: i64) -> Value {
        Value::Int(i)
    }

    fn ok(name: &str, args: &[Value]) -> Value {
        call(name, args).unwrap_or_else(|e| panic!("{name}{args:?} failed: {}", e.message))
    }

    fn code(name: &str, args: &[Value]) -> &'static str {
        call(name, args).unwrap_err().code
    }

    fn words(set: Value) -> Vec<String> {
        match set {
            Value::Set(items) => items
                .iter()
                .map(|v| match v {
                    Value::Str(w) => w.to_string(),
                    other => panic!("{other:?} in a set of strings"),
                })
                .collect(),
            other => panic!("{other:?} is not a set"),
        }
    }

    #[test]
    fn measuring_and_reversing() {
        assert_eq!(ok("length", &[s("")]), n(0));
        assert_eq!(ok("length", &[s("abc")]), n(3));
        // Characters, not bytes: "é" is two bytes and "😀" is four.
        assert_eq!(ok("length", &[s("é😀")]), n(2));
        assert_eq!(ok("reverse", &[s("abc")]), s("cba"));
        assert_eq!(ok("reverse", &[s("é😀")]), s("😀é"));
        assert_eq!(ok("reverse", &[s("")]), s(""));
        assert_eq!(ok("is_palindrome", &[s("")]), Value::Bool(true));
        assert_eq!(ok("is_palindrome", &[s("a")]), Value::Bool(true));
        assert_eq!(ok("is_palindrome", &[s("abba")]), Value::Bool(true));
        assert_eq!(ok("is_palindrome", &[s("ab")]), Value::Bool(false));
    }

    #[test]
    fn searching() {
        assert_eq!(ok("contains", &[s("banana"), s("nan")]), Value::Bool(true));
        assert_eq!(ok("contains", &[s("banana"), s("")]), Value::Bool(true));
        assert_eq!(ok("contains", &[s(""), s("a")]), Value::Bool(false));
        assert_eq!(ok("starts_with", &[s("banana"), s("ban")]), Value::Bool(true));
        assert_eq!(ok("starts_with", &[s("ban"), s("banana")]), Value::Bool(false));
        assert_eq!(ok("ends_with", &[s("banana"), s("ana")]), Value::Bool(true));
        assert_eq!(ok("ends_with", &[s("banana"), s("")]), Value::Bool(true));
        assert_eq!(ok("index_of", &[s("banana"), s("an")]), n(1));
        assert_eq!(ok("index_of", &[s("banana"), s("x")]), n(-1));
        assert_eq!(ok("index_of", &[s("banana"), s("")]), n(0));
        // A character index, not a byte index.
        assert_eq!(ok("index_of", &[s("éé0"), s("0")]), n(2));
        assert_eq!(ok("count", &[s("aaaa"), s("aa")]), n(2));
        assert_eq!(ok("count", &[s("banana"), s("a")]), n(3));
        assert_eq!(ok("count", &[s(""), s("a")]), n(0));
        assert_eq!(code("count", &[s("abc"), s("")]), codes::STRING_RANGE);
    }

    #[test]
    fn slicing() {
        assert_eq!(ok("char_at", &[s("abc"), n(0)]), s("a"));
        assert_eq!(ok("char_at", &[s("aéc"), n(1)]), s("é"));
        assert_eq!(code("char_at", &[s("abc"), n(3)]), codes::STRING_RANGE);
        assert_eq!(code("char_at", &[s("abc"), n(-1)]), codes::STRING_RANGE);
        assert_eq!(code("char_at", &[s(""), n(0)]), codes::STRING_RANGE);
        assert_eq!(ok("substring", &[s("hello"), n(1), n(3)]), s("el"));
        assert_eq!(ok("substring", &[s("hello"), n(0), n(5)]), s("hello"));
        assert_eq!(ok("substring", &[s("hello"), n(5), n(5)]), s(""));
        assert_eq!(ok("substring", &[s("héllo"), n(1), n(2)]), s("é"));
        assert_eq!(code("substring", &[s("hello"), n(-1), n(2)]), codes::STRING_RANGE);
        assert_eq!(code("substring", &[s("hello"), n(3), n(2)]), codes::STRING_RANGE);
        assert_eq!(code("substring", &[s("hello"), n(0), n(6)]), codes::STRING_RANGE);
        assert_eq!(code("substring", &[s("hello"), n(6), n(6)]), codes::STRING_RANGE);
        // The error points at the argument that is wrong.
        assert_eq!(call("substring", &[s("hello"), n(-1), n(2)]).unwrap_err().arg, Some(1));
        assert_eq!(call("substring", &[s("hello"), n(0), n(6)]).unwrap_err().arg, Some(2));
    }

    #[test]
    fn repeating() {
        assert_eq!(ok("repeat", &[s("ab"), n(3)]), s("ababab"));
        assert_eq!(ok("repeat", &[s("ab"), n(0)]), s(""));
        assert_eq!(ok("repeat", &[s(""), n(i64::MAX)]), s(""));
        assert_eq!(code("repeat", &[s("ab"), n(-1)]), codes::STRING_RANGE);
        assert_eq!(ok("repeat", &[s("a"), n(1_000_000)]), s(&"a".repeat(1_000_000)));
        assert_eq!(code("repeat", &[s("a"), n(1_000_001)]), codes::RESULT_TOO_LARGE);
        assert_eq!(code("repeat", &[s("ab"), n(1_000_000_000_000)]), codes::RESULT_TOO_LARGE);
        assert_eq!(code("repeat", &[s("ab"), n(i64::MAX)]), codes::RESULT_TOO_LARGE);
    }

    #[test]
    fn numbers() {
        assert_eq!(ok("to_int", &[s("42")]), n(42));
        assert_eq!(ok("to_int", &[s("-7")]), n(-7));
        assert_eq!(ok("to_int", &[s("+7")]), n(7));
        assert_eq!(ok("to_int", &[s("007")]), n(7));
        assert_eq!(ok("to_int", &[s("9223372036854775807")]), n(i64::MAX));
        assert_eq!(ok("to_int", &[s("-9223372036854775808")]), n(i64::MIN));
        for bad in ["", "-", "+", "abc", "1_000", " 7", "7 ", "1.5", "--1", "٣"] {
            assert_eq!(code("to_int", &[s(bad)]), codes::NOT_A_NUMBER, "{bad:?}");
        }
        assert_eq!(code("to_int", &[s("9223372036854775808")]), codes::OVERFLOW);
        assert_eq!(ok("from_int", &[n(0)]), s("0"));
        assert_eq!(ok("from_int", &[n(-45)]), s("-45"));
        assert_eq!(ok("from_int", &[n(i64::MIN)]), s("-9223372036854775808"));
        for k in [0, 1, -1, 123456, i64::MAX, i64::MIN] {
            assert_eq!(ok("to_int", &[ok("from_int", &[n(k)])]), n(k));
        }
    }

    #[test]
    fn sets_of_words() {
        assert_eq!(words(ok("symbols", &[s("abca")])), ["a", "b", "c"]);
        assert_eq!(words(ok("symbols", &[s("")])), Vec::<String>::new());
        assert_eq!(words(ok("prefixes", &[s("ab")])), ["", "a", "ab"]);
        assert_eq!(words(ok("prefixes", &[s("")])), [""]);
        assert_eq!(words(ok("suffixes", &[s("ab")])), ["", "ab", "b"]);
        assert_eq!(words(ok("suffixes", &[s("")])), [""]);
        // Duplicates collapse: "aa" has the substrings "", "a" and "aa".
        assert_eq!(words(ok("substrings", &[s("aa")])), ["", "a", "aa"]);
        assert_eq!(words(ok("substrings", &[s("abc")])), ["", "a", "ab", "abc", "b", "bc", "c"]);
        assert_eq!(words(ok("substrings", &[s("")])), [""]);
        assert_eq!(words(ok("prefixes", &[s("é😀")])), ["", "é", "é😀"]);
    }

    #[test]
    fn big_results_are_refused_not_built() {
        let long = s(&"a".repeat(2_000));
        // 2000 characters have about 2 million prefix characters in all.
        assert_eq!(code("prefixes", std::slice::from_ref(&long)), codes::RESULT_TOO_LARGE);
        assert_eq!(code("suffixes", std::slice::from_ref(&long)), codes::RESULT_TOO_LARGE);
        assert_eq!(code("substrings", &[s(&"a".repeat(200))]), codes::RESULT_TOO_LARGE);
        assert!(call("substrings", &[s(&"ab".repeat(50))]).is_ok());
        // `symbols` never grows beyond the number of characters.
        assert!(call("symbols", std::slice::from_ref(&long)).is_ok());
    }

    #[test]
    fn length_agrees_with_the_built_in_len() {
        // The interpreter counts `len` of a Str with `chars().count()` too.
        for text in ["", "abc", "é😀x"] {
            assert_eq!(ok("length", &[s(text)]), n(text.chars().count() as i64));
        }
    }
}
