//! `std.math`: whole-number arithmetic beyond `+ - * %`.
//!
//! Every function works on `Int`s, which are 64-bit. A result that does not
//! fit is an overflow error, never a wrong answer, and an argument outside
//! what a function accepts (a square root of a negative number) is an error
//! with a hint, never a panic.

use super::{Failure, Function, Module, Ty};
use crate::lang::codes;
use crate::lang::value::Value;

pub const MODULE: Module = Module {
    name: "math",
    doc: "Whole-number arithmetic: division, powers, roots, divisors and primes.",
    functions: &[
        Function { name: "abs", params: &[("n", Ty::Int)], ret: Ty::Int, doc: "The size of `n`, without its sign.", run: abs },
        Function { name: "div", params: &[("a", Ty::Int), ("b", Ty::Int)], ret: Ty::Int, doc: "`a` divided by `b` as a whole number: the `q` with `a == b * q + a % b`.", run: div },
        Function { name: "factorial", params: &[("n", Ty::Int)], ret: Ty::Int, doc: "`n! = 1 * 2 * ... * n`, for `n` from 0 to 20.", run: factorial },
        Function { name: "gcd", params: &[("a", Ty::Int), ("b", Ty::Int)], ret: Ty::Int, doc: "The greatest common divisor of `a` and `b`; never negative, and `gcd(0, 0)` is 0.", run: gcd },
        Function { name: "is_prime", params: &[("n", Ty::Int)], ret: Ty::Bool, doc: "Whether `n` is a prime number: greater than 1, with no divisors but 1 and itself.", run: is_prime },
        Function { name: "lcm", params: &[("a", Ty::Int), ("b", Ty::Int)], ret: Ty::Int, doc: "The least common multiple of `a` and `b`; never negative, and 0 if either is 0.", run: lcm },
        Function { name: "log2", params: &[("n", Ty::Int)], ret: Ty::Int, doc: "The base-2 logarithm of `n`, rounded down; `n` must be positive.", run: log2 },
        Function { name: "max", params: &[("a", Ty::Int), ("b", Ty::Int)], ret: Ty::Int, doc: "The larger of `a` and `b`.", run: max },
        Function { name: "min", params: &[("a", Ty::Int), ("b", Ty::Int)], ret: Ty::Int, doc: "The smaller of `a` and `b`.", run: min },
        Function { name: "pow", params: &[("base", Ty::Int), ("exp", Ty::Int)], ret: Ty::Int, doc: "`base` to the power `exp`; `exp` must not be negative.", run: pow },
        Function { name: "sqrt", params: &[("n", Ty::Int)], ret: Ty::Int, doc: "The square root of `n`, rounded down; `n` must not be negative.", run: sqrt },
    ],
};

/// The arguments as numbers. The checker has made sure they are `Int`s.
fn ints(args: &[Value]) -> Vec<i64> {
    args.iter()
        .map(|v| match v {
            Value::Int(n) => *n,
            other => unreachable!("the checker allowed {other:?} as an Int"),
        })
        .collect()
}

fn overflow(what: &str) -> Failure {
    Failure {
        code: codes::OVERFLOW,
        arg: None,
        message: format!("{what} is too large for an Int"),
        hint: "Ints are 64-bit: about plus or minus 9 * 10^18".into(),
    }
}

/// An argument the function does not accept.
fn domain(arg: usize, message: String, hint: &str) -> Failure {
    Failure { code: codes::MATH_DOMAIN, arg: Some(arg), message, hint: hint.into() }
}

fn abs(args: &[Value]) -> Result<Value, Failure> {
    let n = ints(args)[0];
    n.checked_abs().map(Value::Int).ok_or_else(|| overflow(&format!("abs({n})")))
}

fn min(args: &[Value]) -> Result<Value, Failure> {
    let n = ints(args);
    Ok(Value::Int(n[0].min(n[1])))
}

fn max(args: &[Value]) -> Result<Value, Failure> {
    let n = ints(args);
    Ok(Value::Int(n[0].max(n[1])))
}

fn div(args: &[Value]) -> Result<Value, Failure> {
    let (a, b) = (ints(args)[0], ints(args)[1]);
    if b == 0 {
        return Err(domain(1, "division by zero".into(), "the second argument of `div` is 0 here"));
    }
    // Rounding down (Euclid's way, like `%`) keeps `a == b * div(a, b) + a % b`
    // true for every sign.
    a.checked_div_euclid(b).map(Value::Int).ok_or_else(|| overflow(&format!("div({a}, {b})")))
}

fn pow(args: &[Value]) -> Result<Value, Failure> {
    let (base, exp) = (ints(args)[0], ints(args)[1]);
    if exp < 0 {
        return Err(domain(
            1,
            format!("the exponent {exp} is negative"),
            "a negative power is a fraction, and Ints are whole numbers",
        ));
    }
    let exp32 = u32::try_from(exp).unwrap_or(u32::MAX);
    base.checked_pow(exp32).map(Value::Int).ok_or_else(|| overflow(&format!("pow({base}, {exp})")))
}

/// Euclid's algorithm, on the sizes of the numbers (as `u64`, so that the
/// size of the most negative Int fits).
fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn gcd(args: &[Value]) -> Result<Value, Failure> {
    let (a, b) = (ints(args)[0], ints(args)[1]);
    let g = gcd_u64(a.unsigned_abs(), b.unsigned_abs());
    i64::try_from(g).map(Value::Int).map_err(|_| overflow(&format!("gcd({a}, {b})")))
}

fn lcm(args: &[Value]) -> Result<Value, Failure> {
    let (a, b) = (ints(args)[0], ints(args)[1]);
    if a == 0 || b == 0 {
        return Ok(Value::Int(0));
    }
    let (x, y) = (a.unsigned_abs(), b.unsigned_abs());
    // Divide before multiplying, so the only overflow is a true one.
    (x / gcd_u64(x, y))
        .checked_mul(y)
        .and_then(|l| i64::try_from(l).ok())
        .map(Value::Int)
        .ok_or_else(|| overflow(&format!("lcm({a}, {b})")))
}

fn sqrt(args: &[Value]) -> Result<Value, Failure> {
    let n = ints(args)[0];
    if n < 0 {
        return Err(domain(0, format!("the square root of {n} is not a whole number"), "`sqrt` needs a number from 0 up"));
    }
    Ok(Value::Int(n.isqrt()))
}

fn log2(args: &[Value]) -> Result<Value, Failure> {
    let n = ints(args)[0];
    if n <= 0 {
        return Err(domain(0, format!("log2({n}) is not defined"), "`log2` needs a number from 1 up; log2(1) is 0"));
    }
    Ok(Value::Int(i64::from(n.ilog2())))
}

fn factorial(args: &[Value]) -> Result<Value, Failure> {
    let n = ints(args)[0];
    if n < 0 {
        return Err(domain(0, format!("{n}! is not defined"), "a factorial needs a number from 0 up"));
    }
    (1..=n).try_fold(1i64, |acc, k| acc.checked_mul(k)).map(Value::Int).ok_or_else(|| Failure {
        hint: "20! is the largest factorial that fits in a 64-bit Int".into(),
        ..overflow(&format!("{n}!"))
    })
}

/// `a * b % m` without overflow, through 128-bit arithmetic.
fn mul_mod(a: u64, b: u64, m: u64) -> u64 {
    (u128::from(a) * u128::from(b) % u128::from(m)) as u64
}

fn pow_mod(mut base: u64, mut exp: u64, m: u64) -> u64 {
    let mut result = 1;
    base %= m;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, base, m);
        }
        base = mul_mod(base, base, m);
        exp >>= 1;
    }
    result
}

/// The Miller-Rabin test. Trying every number up to the square root would
/// take billions of steps for a large Int; with these twelve bases, this
/// test is known to be exact for every number below 3.3 * 10^24, which
/// covers every Int.
fn is_prime(args: &[Value]) -> Result<Value, Failure> {
    let n = ints(args)[0];
    if n < 2 {
        return Ok(Value::Bool(false));
    }
    let n = n as u64;
    const BASES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
    if let Some(&p) = BASES.iter().find(|&&p| n.is_multiple_of(p)) {
        return Ok(Value::Bool(n == p));
    }
    // n - 1 = d * 2^s with d odd.
    let s = (n - 1).trailing_zeros();
    let d = (n - 1) >> s;
    let prime = BASES.iter().all(|&a| {
        let mut x = pow_mod(a, d, n);
        if x == 1 || x == n - 1 {
            return true;
        }
        for _ in 1..s {
            x = mul_mod(x, x, n);
            if x == n - 1 {
                return true;
            }
        }
        false
    });
    Ok(Value::Bool(prime))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &[i64]) -> Result<Value, Failure> {
        let f = MODULE.functions.iter().find(|f| f.name == name).unwrap();
        (f.run)(&args.iter().map(|&n| Value::Int(n)).collect::<Vec<_>>())
    }

    fn int(name: &str, args: &[i64]) -> i64 {
        match call(name, args) {
            Ok(Value::Int(n)) => n,
            other => panic!("{name}{args:?} gave {other:?}"),
        }
    }

    fn code(name: &str, args: &[i64]) -> &'static str {
        call(name, args).unwrap_err().code
    }

    #[test]
    fn arithmetic() {
        assert_eq!(int("abs", &[-7]), 7);
        assert_eq!((int("min", &[3, -2]), int("max", &[3, -2])), (-2, 3));
        assert_eq!(int("pow", &[2, 10]), 1024);
        assert_eq!(int("pow", &[-3, 3]), -27);
        assert_eq!(int("pow", &[0, 0]), 1);
        assert_eq!(code("pow", &[2, 63]), codes::OVERFLOW);
        assert_eq!(code("pow", &[2, -1]), codes::MATH_DOMAIN);
        assert_eq!(code("abs", &[i64::MIN]), codes::OVERFLOW);
    }

    #[test]
    fn division_agrees_with_modulo() {
        for a in -20..=20 {
            for b in [-7, -3, -1, 1, 2, 5] {
                let q = int("div", &[a, b]);
                assert_eq!(b * q + a.rem_euclid(b), a, "div({a}, {b})");
            }
        }
        assert_eq!(int("div", &[7, 2]), 3);
        assert_eq!(int("div", &[-7, 2]), -4);
        assert_eq!(code("div", &[1, 0]), codes::MATH_DOMAIN);
        assert_eq!(code("div", &[i64::MIN, -1]), codes::OVERFLOW);
    }

    #[test]
    fn divisors() {
        assert_eq!(int("gcd", &[12, 18]), 6);
        assert_eq!(int("gcd", &[-12, 18]), 6);
        assert_eq!(int("gcd", &[0, 0]), 0);
        assert_eq!(int("gcd", &[i64::MIN, 6]), 2);
        assert_eq!(code("gcd", &[i64::MIN, 0]), codes::OVERFLOW);
        assert_eq!(int("lcm", &[4, 6]), 12);
        assert_eq!(int("lcm", &[-4, 6]), 12);
        assert_eq!(int("lcm", &[0, 5]), 0);
        assert_eq!(code("lcm", &[i64::MAX, i64::MAX - 1]), codes::OVERFLOW);
    }

    #[test]
    fn roots_logs_and_factorials() {
        assert_eq!(int("sqrt", &[0]), 0);
        assert_eq!(int("sqrt", &[15]), 3);
        assert_eq!(int("sqrt", &[16]), 4);
        assert_eq!(int("sqrt", &[i64::MAX]), 3_037_000_499);
        assert_eq!(code("sqrt", &[-1]), codes::MATH_DOMAIN);
        assert_eq!(int("log2", &[1]), 0);
        assert_eq!(int("log2", &[1023]), 9);
        assert_eq!(int("log2", &[1024]), 10);
        assert_eq!(code("log2", &[0]), codes::MATH_DOMAIN);
        assert_eq!(int("factorial", &[0]), 1);
        assert_eq!(int("factorial", &[20]), 2_432_902_008_176_640_000);
        assert_eq!(code("factorial", &[21]), codes::OVERFLOW);
        assert_eq!(code("factorial", &[-1]), codes::MATH_DOMAIN);
    }

    #[test]
    fn primes_agree_with_trial_division() {
        let slow = |n: i64| n >= 2 && (2..).take_while(|d| d * d <= n).all(|d| n % d != 0);
        for n in -5..5000 {
            assert_eq!(call("is_prime", &[n]).unwrap(), Value::Bool(slow(n)), "{n}");
        }
        let prime = |n: i64| call("is_prime", &[n]).unwrap() == Value::Bool(true);
        assert!(prime(1_000_000_007));
        assert!(prime(9_223_372_036_854_775_783)); // the largest prime Int
        assert!(!prime(i64::MAX));
        assert!(!prime(3_215_031_751)); // fools the bases 2, 3, 5 and 7 alone
    }
}
