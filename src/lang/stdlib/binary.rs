//! `std.binary`: machines for binary numbers and bit strings, and the
//! conversions between Ints and bit strings that make checking them easy.
//!
//! Every machine is over the alphabet {0, 1}, so none of these functions
//! takes an alphabet. A bit string is read as a number most significant bit
//! first, leading zeros do not count, and the empty string is 0; see
//! `automata::binary` for why, and `no_leading_zeros` for numerals written
//! the usual way.
//!
//! Relation to other modules: `fixed_width` and the parity machines are the
//! `std.automata` machines `length_between` and `count_mod` with the
//! alphabet filled in; `divisible_by` is the remainder machine that the
//! tutorial builds by hand with a `for` loop.

use super::automata::too_many_states;
use super::{int_arg as int, str_arg as text, Failure, Function, Module, Ty};
use crate::automata::binary;
use crate::lang::codes;
use crate::lang::value::Value;

pub const MODULE: Module = Module {
    name: "binary",
    doc: "Binary numbers and bit strings over {0, 1}: divisibility and remainders, numerals without leading zeros, fixed widths, parity, runs of equal bits, and converting between Ints and bit strings.",
    functions: &[
        Function { name: "divisible_by", params: &[("n", Ty::Int)], ret: Ty::Dfa, doc: "The bit strings whose value is a multiple of `n`, as the machine with one state per remainder.", run: divisible_by },
        Function { name: "remainder", params: &[("n", Ty::Int), ("r", Ty::Int)], ret: Ty::Dfa, doc: "The bit strings whose value leaves remainder `r` when divided by `n`.", run: remainder },
        Function { name: "no_leading_zeros", params: &[], ret: Ty::Dfa, doc: "Numerals written without leading zeros: `0`, or a `1` followed by any bits.", run: no_leading_zeros },
        Function { name: "fixed_width", params: &[("n", Ty::Int)], ret: Ty::Dfa, doc: "The bit strings of exactly `n` bits.", run: fixed_width },
        Function { name: "even_parity", params: &[], ret: Ty::Dfa, doc: "The bit strings with an even number of 1s.", run: even_parity },
        Function { name: "odd_parity", params: &[], ret: Ty::Dfa, doc: "The bit strings with an odd number of 1s.", run: odd_parity },
        Function { name: "run_at_most", params: &[("bit", Ty::Str), ("k", Ty::Int)], ret: Ty::Dfa, doc: "The bit strings in which `bit` never appears more than `k` times in a row.", run: run_at_most },
        Function { name: "to_binary", params: &[("n", Ty::Int)], ret: Ty::Str, doc: "`n` written in binary without leading zeros; `n` must not be negative, and 0 is `\"0\"`.", run: to_binary },
        Function { name: "from_binary", params: &[("bits", Ty::Str)], ret: Ty::Int, doc: "The value of a bit string, most significant bit first; leading zeros do not count and `\"\"` is 0.", run: from_binary },
    ],
    explain: None,
};

/// The largest machine built here, as in `std.automata`.
const MAX_STATES: i64 = super::automata::MAX_STATES;

/// Argument `i`: a divisor from 1 to `MAX_STATES`.
fn divisor(args: &[Value], i: usize) -> Result<usize, Failure> {
    let n = int(args, i);
    if n < 1 {
        return Err(Failure {
            code: codes::MATH_DOMAIN,
            arg: Some(i),
            message: format!("cannot divide by {n}"),
            hint: "the divisor must be 1 or more".into(),
        });
    }
    if n > MAX_STATES {
        return Err(too_many_states(i, "a divisor", n));
    }
    Ok(n as usize)
}

/// Argument `i`: a width or run length from 0 to `MAX_STATES`.
fn length(args: &[Value], i: usize) -> Result<usize, Failure> {
    let n = int(args, i);
    if n < 0 {
        return Err(Failure {
            code: codes::STRING_RANGE,
            arg: Some(i),
            message: format!("a length cannot be negative, but this is {n}"),
            hint: "use 0 or more".into(),
        });
    }
    if n > MAX_STATES {
        return Err(too_many_states(i, "a length", n));
    }
    Ok(n as usize)
}

fn divisible_by(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(binary::remainder(divisor(args, 0)?, 0)))
}

fn remainder(args: &[Value]) -> Result<Value, Failure> {
    let n = divisor(args, 0)?;
    let r = int(args, 1);
    if !(0..n as i64).contains(&r) {
        return Err(Failure {
            code: codes::MATH_DOMAIN,
            arg: Some(1),
            message: format!("a remainder after dividing by {n} cannot be {r}"),
            hint: format!("the remainder must be from 0 to {}", n - 1),
        });
    }
    Ok(Value::dfa(binary::remainder(n, r as usize)))
}

fn no_leading_zeros(_: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(binary::no_leading_zeros()))
}

fn fixed_width(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(binary::fixed_width(length(args, 0)?)))
}

fn even_parity(_: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(binary::parity(false)))
}

fn odd_parity(_: &[Value]) -> Result<Value, Failure> {
    Ok(Value::dfa(binary::parity(true)))
}

fn run_at_most(args: &[Value]) -> Result<Value, Failure> {
    let bit = match text(args, 0) {
        "0" => '0',
        "1" => '1',
        other if other.chars().count() == 1 => {
            return Err(Failure {
                code: codes::NOT_IN_ALPHABET,
                arg: Some(0),
                message: format!("`{other}` is not a bit"),
                hint: "write \"0\" or \"1\"".into(),
            })
        }
        other => {
            return Err(Failure {
                code: codes::NOT_A_SYMBOL,
                arg: Some(0),
                message: format!("{other:?} is not one symbol"),
                hint: "write \"0\" or \"1\"".into(),
            })
        }
    };
    Ok(Value::dfa(binary::run_at_most(bit, length(args, 1)?)))
}

fn to_binary(args: &[Value]) -> Result<Value, Failure> {
    let n = int(args, 0);
    if n < 0 {
        return Err(Failure {
            code: codes::MATH_DOMAIN,
            arg: Some(0),
            message: format!("{n} is negative, and bit strings here stand for numbers from 0 up"),
            hint: "write the sign separately, or work with a number from 0 up".into(),
        });
    }
    Ok(Value::str(&binary::to_binary(n as u64)))
}

fn from_binary(args: &[Value]) -> Result<Value, Failure> {
    let bits = text(args, 0);
    if let Some(c) = bits.chars().find(|c| !matches!(c, '0' | '1')) {
        return Err(Failure {
            code: codes::NOT_A_NUMBER,
            arg: Some(0),
            message: format!("{bits:?} is not a bit string: it contains `{c}`"),
            hint: "a bit string is made of 0s and 1s only".into(),
        });
    }
    match binary::from_binary(bits).and_then(|v| i64::try_from(v).ok()) {
        Some(v) => Ok(Value::Int(v)),
        None => Err(Failure {
            code: codes::OVERFLOW,
            arg: Some(0),
            message: format!("the value of these {} bits is too large for an Int", bits.len()),
            hint: "Ints are 64-bit: the largest is 63 ones in a row".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &[Value]) -> Result<Value, Failure> {
        let f = MODULE.functions.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no std.binary.{name}"));
        (f.run)(args)
    }

    fn accepts(name: &str, args: &[Value], word: &str) -> bool {
        match call(name, args) {
            Ok(Value::Dfa(d)) => d.accepts(word).unwrap(),
            other => panic!("{name}: {other:?}"),
        }
    }

    fn code(name: &str, args: &[Value]) -> &'static str {
        call(name, args).expect_err("expected a failure").code
    }

    fn int(n: i64) -> Value {
        Value::Int(n)
    }

    #[test]
    fn machines() {
        assert!(accepts("divisible_by", &[int(3)], "110") && !accepts("divisible_by", &[int(3)], "111"));
        assert!(accepts("remainder", &[int(3), int(1)], "100"));
        assert!(accepts("no_leading_zeros", &[], "0") && !accepts("no_leading_zeros", &[], "01"));
        assert!(accepts("fixed_width", &[int(3)], "010") && !accepts("fixed_width", &[int(3)], "01"));
        assert!(accepts("even_parity", &[], "0110") && !accepts("even_parity", &[], "010"));
        assert!(accepts("odd_parity", &[], "010") && !accepts("odd_parity", &[], ""));
        assert!(accepts("run_at_most", &[Value::str("0"), int(2)], "1001") && !accepts("run_at_most", &[Value::str("0"), int(2)], "0001"));
    }

    #[test]
    fn conversions() {
        assert_eq!(call("to_binary", &[int(10)]).unwrap(), Value::str("1010"));
        assert_eq!(call("to_binary", &[int(0)]).unwrap(), Value::str("0"));
        assert_eq!(call("to_binary", &[int(i64::MAX)]).unwrap(), Value::str(&"1".repeat(63)));
        assert_eq!(call("from_binary", &[Value::str("001010")]).unwrap(), int(10));
        assert_eq!(call("from_binary", &[Value::str("")]).unwrap(), int(0));
    }

    #[test]
    fn mistakes() {
        assert_eq!(code("divisible_by", &[int(0)]), codes::MATH_DOMAIN);
        assert_eq!(code("divisible_by", &[int(-3)]), codes::MATH_DOMAIN);
        assert_eq!(code("divisible_by", &[int(10_001)]), codes::RESULT_TOO_LARGE);
        assert_eq!(code("remainder", &[int(3), int(3)]), codes::MATH_DOMAIN);
        assert_eq!(code("fixed_width", &[int(-1)]), codes::STRING_RANGE);
        assert_eq!(code("fixed_width", &[int(10_001)]), codes::RESULT_TOO_LARGE);
        assert_eq!(code("run_at_most", &[Value::str("2"), int(1)]), codes::NOT_IN_ALPHABET);
        assert_eq!(code("run_at_most", &[Value::str("01"), int(1)]), codes::NOT_A_SYMBOL);
        assert_eq!(code("run_at_most", &[Value::str("1"), int(-1)]), codes::STRING_RANGE);
        assert_eq!(code("to_binary", &[int(-5)]), codes::MATH_DOMAIN);
        assert_eq!(code("from_binary", &[Value::str("102")]), codes::NOT_A_NUMBER);
        assert_eq!(code("from_binary", &[Value::str(&"1".repeat(64))]), codes::OVERFLOW);
    }
}
