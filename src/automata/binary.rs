//! Machines for binary numbers and bit strings, over the alphabet {0, 1},
//! and conversions between numbers and bit strings.
//!
//! **How a bit string is read as a number.** Most significant bit first, as
//! people write numbers: `110` is 6. Leading zeros do not change the value
//! (`0110` is 6 too), and the empty string is read as 0. So the remainder
//! machines accept `""`, `"0"` and `"00"` among the multiples of 3. To insist
//! on numerals written without leading zeros, intersect with
//! `no_leading_zeros()`.

use super::{patterns, Dfa, State};
use std::collections::BTreeSet;

/// The binary alphabet.
pub const BITS: [char; 2] = ['0', '1'];

/// The bit strings whose value leaves remainder `r` when divided by `n`
/// (`n >= 1`, `r < n`).
///
/// State `r{i}` means "the bits read so far are a number with remainder
/// `i`". Reading one more bit `b` doubles the number and adds `b`, so the
/// remainder goes from `i` to `(2i + b) mod n`: only the remainder needs
/// remembering, never the number itself, which is why `n` states are
/// enough. (This is the textbook machine with one state per remainder. It
/// is minimal when `n` is odd; for even `n` some remainders behave alike and
/// `minimize` merges them.)
pub fn remainder(n: usize, r: usize) -> Dfa {
    Dfa {
        alphabet: BITS.to_vec(),
        states: (0..n).map(|i| format!("r{i}")).collect(),
        start: 0,
        accept: BTreeSet::from([r]),
        delta: (0..n).map(|i| vec![(2 * i) % n, (2 * i + 1) % n]).collect(),
    }
}

/// Numerals without leading zeros: `0`, or a `1` followed by any bits. The
/// empty string is not one of them.
pub fn no_leading_zeros() -> Dfa {
    // start --0--> zero (accepts, but nothing may follow it)
    // start --1--> number (accepts, and anything may follow)
    let (start, zero, number, dead): (State, State, State, State) = (0, 1, 2, 3);
    Dfa {
        alphabet: BITS.to_vec(),
        states: ["start", "zero", "number", "dead"].map(String::from).to_vec(),
        start,
        accept: BTreeSet::from([zero, number]),
        delta: vec![vec![zero, number], vec![dead, dead], vec![number, number], vec![dead, dead]],
    }
}

/// Bit strings in which `bit` never appears more than `k` times in a row.
/// State `run{i}` means the last `i` bits were `bit`; `dead` means a run
/// grew past `k`. With `bit = '1'` and `k = 1`, no two 1s are next to each
/// other (the strings counted by the Fibonacci numbers).
pub fn run_at_most(bit: char, k: usize) -> Dfa {
    let dead = k + 1;
    let mut states: Vec<String> = (0..=k).map(|i| format!("run{i}")).collect();
    states.push("dead".into());
    let delta = (0..=dead)
        .map(|s| {
            BITS.iter()
                .map(|&c| match s {
                    s if s == dead => dead,
                    _ if c != bit => 0,
                    s => s + 1,
                })
                .collect()
        })
        .collect();
    Dfa { alphabet: BITS.to_vec(), states, start: 0, accept: (0..=k).collect(), delta }
}

/// Bit strings of exactly `n` bits.
pub fn fixed_width(n: usize) -> Dfa {
    patterns::length_between(&BITS, n, n)
}

/// Bit strings with an even (`odd = false`) or odd (`odd = true`) number of
/// 1s: what a parity bit makes true.
pub fn parity(odd: bool) -> Dfa {
    patterns::count_mod(&BITS, '1', 2, odd as usize)
}

/// `n` written in binary without leading zeros; 0 is `"0"`.
pub fn to_binary(n: u64) -> String {
    format!("{n:b}")
}

/// The value of a bit string (see the module comment for how it is read),
/// or `None` if it has a character other than `0` and `1`, or a value too
/// large for 64 bits.
pub fn from_binary(bits: &str) -> Option<u64> {
    bits.chars().try_fold(0u64, |value, c| {
        let bit = match c {
            '0' => 0,
            '1' => 1,
            _ => return None,
        };
        value.checked_mul(2)?.checked_add(bit)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divisible_by_three() {
        let d = remainder(3, 0);
        for (w, expected) in [("", true), ("0", true), ("11", true), ("110", true), ("0110", true), ("10", false), ("111", false)] {
            assert_eq!(d.accepts(w), Ok(expected), "{w:?}");
        }
    }

    #[test]
    fn no_two_ones_together() {
        let d = run_at_most('1', 1);
        assert_eq!(d.accepts("10101"), Ok(true));
        assert_eq!(d.accepts("0110"), Ok(false));
        assert_eq!(d.states, ["run0", "run1", "dead"]);
    }

    #[test]
    fn conversions() {
        assert_eq!(to_binary(6), "110");
        assert_eq!(from_binary("0110"), Some(6));
        assert_eq!(from_binary("12"), None);
    }
}
