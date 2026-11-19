//! Ready-made DFAs for patterns that come up again and again: words that
//! start with, end with or contain a given word, words with a given number
//! of some symbol, and words whose length is in a range or has a remainder.
//!
//! Each function builds the *smallest* DFA for its language directly, with
//! state names that say what the state remembers, so the machines read well
//! when printed or drawn. A state that cannot be reached (such as a dead
//! state when every symbol is part of the pattern) is left out.
//!
//! The arguments are trusted: every symbol is in `alphabet`, a modulus is at
//! least 1 and a remainder below it, and `min <= max`. The standard library
//! (`std.automata`) checks them before calling.

use super::{Dfa, State};
use std::collections::BTreeSet;

/// Builds a DFA from a transition function and keeps only the states the
/// start can reach. `next(state, symbol)` gives the target.
fn build(
    alphabet: &[char],
    names: Vec<String>,
    accept: impl Fn(State) -> bool,
    next: impl Fn(State, char) -> State,
) -> Dfa {
    let n = names.len();
    let full = Dfa {
        alphabet: alphabet.to_vec(),
        start: 0,
        accept: (0..n).filter(|&s| accept(s)).collect::<BTreeSet<_>>(),
        delta: (0..n).map(|s| alphabet.iter().map(|&c| next(s, c)).collect()).collect(),
        states: names,
    };
    let order = full.reachable();
    let names = order.iter().map(|&s| full.states[s].clone()).collect();
    full.remap(&order, names)
}

/// Names `prefix0`, `prefix1`, ... up to `prefix{last}`.
fn numbered(prefix: &str, last: usize) -> Vec<String> {
    (0..=last).map(|i| format!("{prefix}{i}")).collect()
}

/// Words that begin with `prefix`. State `p{i}` means "the first `i`
/// symbols matched"; `dead` means a symbol did not match.
pub fn starts_with(alphabet: &[char], prefix: &[char]) -> Dfa {
    let k = prefix.len();
    let dead = k + 1;
    let mut names = numbered("p", k);
    names.push("dead".into());
    build(alphabet, names, |s| s == k, |s, c| match s {
        s if s == k || s == dead => s,
        s if prefix[s] == c => s + 1,
        _ => dead,
    })
}

/// The Knuth-Morris-Pratt table: for each `i`, the length of the longest
/// proper prefix of `word[..i + 1]` that is also a suffix of it.
fn failure(word: &[char]) -> Vec<usize> {
    let mut fail = vec![0; word.len()];
    let mut k = 0;
    for i in 1..word.len() {
        while k > 0 && word[i] != word[k] {
            k = fail[k - 1];
        }
        if word[i] == word[k] {
            k += 1;
        }
        fail[i] = k;
    }
    fail
}

/// The matching automaton of `word`: from "the last `i` symbols read are the
/// first `i` of `word`" (the longest such `i`), where reading `c` leads.
fn kmp_step(word: &[char], fail: &[usize], mut i: usize, c: char) -> usize {
    // Having matched all of `word`, carry on from its longest border.
    if i == word.len() {
        i = if i == 0 { 0 } else { fail[i - 1] };
    }
    while i > 0 && word[i] != c {
        i = fail[i - 1];
    }
    if i < word.len() && word[i] == c {
        i + 1
    } else {
        i
    }
}

/// Words that end with `suffix`. State `m{i}` means "the last `i` symbols
/// read are the first `i` of the suffix" (the longest such match).
pub fn ends_with(alphabet: &[char], suffix: &[char]) -> Dfa {
    let k = suffix.len();
    let fail = failure(suffix);
    build(alphabet, numbered("m", k), |s| s == k, |s, c| kmp_step(suffix, &fail, s, c))
}

/// Words that contain `part` somewhere. Like `ends_with`, except that once
/// `part` has been seen, the machine stays in the accepting state `m{k}`.
pub fn contains(alphabet: &[char], part: &[char]) -> Dfa {
    let k = part.len();
    let fail = failure(part);
    build(alphabet, numbered("m", k), |s| s == k, |s, c| if s == k { k } else { kmp_step(part, &fail, s, c) })
}

/// Counts `symbol` up to `n`, then one more state, `more`, for "over `n`".
/// `accept(i)` says which counts accept (`more` is count `n + 1`).
fn counting(alphabet: &[char], symbol: char, n: usize, sink_at_n: bool, accept: impl Fn(usize) -> bool) -> Dfa {
    let mut names = numbered("c", n);
    names.push("more".into());
    build(alphabet, names, accept, |s, c| match s {
        s if c != symbol => s,
        s if s == n && sink_at_n => n,
        s => (s + 1).min(n + 1),
    })
}

/// Words with exactly `n` copies of `symbol`. State `c{i}` counts them.
pub fn count_exactly(alphabet: &[char], symbol: char, n: usize) -> Dfa {
    counting(alphabet, symbol, n, false, |s| s == n)
}

/// Words with at least `n` copies of `symbol`: once `c{n}` is reached it is
/// never left.
pub fn count_at_least(alphabet: &[char], symbol: char, n: usize) -> Dfa {
    counting(alphabet, symbol, n, true, |s| s == n)
}

/// Words with at most `n` copies of `symbol`.
pub fn count_at_most(alphabet: &[char], symbol: char, n: usize) -> Dfa {
    counting(alphabet, symbol, n, false, |s| s <= n)
}

/// Words in which the number of `symbol`s leaves remainder `r` when divided
/// by `m`. State `r{i}` is the remainder so far. With `m = 2` this is
/// parity: `r = 0` for an even number, `r = 1` for an odd one.
pub fn count_mod(alphabet: &[char], symbol: char, m: usize, r: usize) -> Dfa {
    build(alphabet, numbered("r", m - 1), |s| s == r, |s, c| if c == symbol { (s + 1) % m } else { s })
}

/// Words whose length leaves remainder `r` when divided by `m`.
pub fn length_mod(alphabet: &[char], m: usize, r: usize) -> Dfa {
    build(alphabet, numbered("r", m - 1), |s| s == r, |s, _| (s + 1) % m)
}

/// Words of length `min` to `max`, both included. State `l{i}` has read `i`
/// symbols; `long` has read more than `max`.
pub fn length_between(alphabet: &[char], min: usize, max: usize) -> Dfa {
    let mut names = numbered("l", max);
    names.push("long".into());
    build(alphabet, names, |s| (min..=max).contains(&s), |s, _| (s + 1).min(max + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: [char; 2] = ['0', '1'];

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn states_say_what_they_remember() {
        let d = starts_with(&B, &chars("01"));
        assert_eq!(d.states, ["p0", "p1", "dead", "p2"]);
        let d = ends_with(&B, &chars("01"));
        assert_eq!(d.states, ["m0", "m1", "m2"]);
        assert_eq!(count_exactly(&B, '1', 2).states, ["c0", "c1", "c2", "more"]);
        assert_eq!(count_mod(&B, '1', 2, 0).states, ["r0", "r1"]);
    }

    #[test]
    fn the_kmp_machine_falls_back_correctly() {
        // "aab": after reading "aa", another "a" keeps "aa" matched.
        let d = ends_with(&['a', 'b'], &chars("aab"));
        assert_eq!(d.accepts("aaab"), Ok(true));
        assert_eq!(d.accepts("aabab"), Ok(false));
        let d = contains(&['a', 'b'], &chars("abab"));
        assert_eq!(d.accepts("aababb"), Ok(true));
        assert_eq!(d.accepts("abaab"), Ok(false));
    }

    #[test]
    fn empty_patterns_accept_everything() {
        for d in [starts_with(&B, &[]), ends_with(&B, &[]), contains(&B, &[]), count_at_least(&B, '0', 0)] {
            assert_eq!(d.states.len(), 1);
            assert_eq!(d.accepts("0110"), Ok(true));
        }
    }
}
