//! Turing machines that come up again and again: arithmetic that leaves
//! its answer on the tape, and deciders for languages beyond what a PDA
//! can do. Each documents its tape convention: what the input must look
//! like and what is on the tape when it halts.
//!
//! The deciders say no by entering their `no` state, never by getting
//! stuck: every state that can see a symbol it has no use for moves to
//! `no`, so the state a run ends in always means something.
//!
//! The machine type and how it runs are in `tm`.

use super::tm::{Dir, Move, Tm, BLANK};
use std::collections::BTreeSet;

/// One move as written in the source of a machine: `(from, read, write,
/// direction, to)`.
type Written<'a> = (&'a str, char, char, Dir, &'a str);

/// Builds a machine from its states (the first is the start), its
/// accepting and rejecting states, and its moves. With `tape` (every
/// symbol it can meet), each state that is neither accepting nor rejecting
/// also gets a move to `no` for every tape symbol it has no move for.
fn build(alphabet: &[char], states: &[&str], accept: &[&str], reject: &[&str], moves: &[Written], tape: Option<&[char]>) -> Tm {
    let mut alphabet = alphabet.to_vec();
    alphabet.sort_unstable();
    alphabet.dedup();
    let at = |name: &str| states.iter().position(|s| *s == name).unwrap_or_else(|| panic!("no state {name}"));
    let mut tm = Tm {
        alphabet,
        states: states.iter().map(|s| s.to_string()).collect(),
        start: 0,
        accept: accept.iter().map(|s| at(s)).collect(),
        reject: reject.iter().map(|s| at(s)).collect(),
        moves: moves.iter().map(|&(from, read, write, dir, to)| Move { from: at(from), read, write, dir, to: at(to) }).collect(),
    };
    if let Some(tape) = tape {
        let no = at("no");
        for state in 0..states.len() {
            if tm.accept.contains(&state) || tm.reject.contains(&state) {
                continue;
            }
            for &c in tape {
                if tm.move_for(state, c).is_none() {
                    tm.moves.push(Move { from: state, read: c, write: c, dir: Dir::Stay, to: no });
                }
            }
        }
    }
    tm
}

use Dir::{Left as L, Right as R, Stay as S};

/// Adds one to a binary number, most significant bit first: go to the
/// right end, then carry leftwards. The empty input counts as 0, so it
/// becomes `1`. Halts in `done` with the sum on the tape.
pub fn increment() -> Tm {
    build(
        &['0', '1'],
        &["right", "carry", "done"],
        &["done"],
        &[],
        &[
            ("right", '0', '0', R, "right"),
            ("right", '1', '1', R, "right"),
            ("right", BLANK, BLANK, L, "carry"),
            ("carry", '1', '0', L, "carry"),
            ("carry", '0', '1', S, "done"),
            ("carry", BLANK, '1', S, "done"),
        ],
        None,
    )
}

/// Unary addition: the input `1^m 0 1^n` (m and n ones with a 0 between)
/// becomes `1^(m+n)`. The 0 is turned into a 1, and then the last 1 is
/// erased. Halts in `done`; an input without exactly one 0 halts in `no`.
pub fn unary_add() -> Tm {
    build(
        &['0', '1'],
        &["first", "second", "erase", "done", "no"],
        &["done"],
        &["no"],
        &[
            ("first", '1', '1', R, "first"),
            ("first", '0', '1', R, "second"),
            ("second", '1', '1', R, "second"),
            ("second", BLANK, BLANK, L, "erase"),
            ("erase", '1', BLANK, S, "done"),
        ],
        Some(&['0', '1', BLANK]),
    )
}

/// Copies a unary number: the input `1^n` becomes `1^n 0 1^n`. It writes
/// the 0 at the end, then for each 1 of the input marks it `x`, carries a
/// 1 to the far end, and comes back; at the end the marks become 1s again.
/// Halts in `done`.
pub fn unary_copy() -> Tm {
    build(
        &['1'],
        &["mark_end", "back", "pick", "carry", "restore", "done"],
        &["done"],
        &[],
        &[
            ("mark_end", '1', '1', R, "mark_end"),
            ("mark_end", BLANK, '0', L, "back"),
            ("back", '1', '1', L, "back"),
            ("back", '0', '0', L, "back"),
            ("back", 'x', 'x', R, "pick"),
            ("back", BLANK, BLANK, R, "pick"),
            ("pick", '1', 'x', R, "carry"),
            ("pick", '0', '0', L, "restore"),
            ("carry", '1', '1', R, "carry"),
            ("carry", '0', '0', R, "carry"),
            ("carry", BLANK, '1', L, "back"),
            ("restore", 'x', '1', L, "restore"),
            ("restore", BLANK, BLANK, R, "done"),
        ],
        None,
    )
}

/// Decides the palindromes over an alphabet: erase the first symbol,
/// remembering it in the state (`carry_c`), check that the last symbol is
/// the same (`check_c`) and erase it too, go back to the start, and repeat.
/// Halts in `yes` or `no`.
pub fn palindromes(alphabet: &[char]) -> Tm {
    let carry: Vec<String> = alphabet.iter().map(|c| format!("carry_{c}")).collect();
    let check: Vec<String> = alphabet.iter().map(|c| format!("check_{c}")).collect();
    let mut states: Vec<&str> = vec!["first"];
    states.extend(carry.iter().map(String::as_str));
    states.extend(check.iter().map(String::as_str));
    states.extend(["back", "yes", "no"]);
    let mut moves: Vec<Written> = vec![("first", BLANK, BLANK, S, "yes"), ("back", BLANK, BLANK, R, "first")];
    for (i, &c) in alphabet.iter().enumerate() {
        moves.push(("first", c, BLANK, R, &carry[i]));
        moves.push((&carry[i], BLANK, BLANK, L, &check[i]));
        moves.push((&check[i], c, BLANK, L, "back"));
        moves.push((&check[i], BLANK, BLANK, S, "yes"));
        for &d in alphabet {
            moves.push((&carry[i], d, d, R, &carry[i]));
        }
        moves.push(("back", c, c, L, "back"));
    }
    let mut tape: Vec<char> = alphabet.to_vec();
    tape.push(BLANK);
    build(alphabet, &states, &["yes"], &["no"], &moves, Some(&tape))
}

/// Decides a^n b^n c^n, the standard language that is not context-free:
/// mark an `a` as `x`, the first unmarked `b` as `y` and the first unmarked
/// `c` as `z`, go back, and repeat; when no `a` is left, check that only
/// marks remain. Halts in `yes` or `no`.
pub fn anbncn(a: char, b: char, c: char) -> Tm {
    let (x, y, z) = ('x', 'y', 'z');
    build(
        &[a, b, c],
        &["mark_a", "find_b", "find_c", "back", "check_y", "check_z", "yes", "no"],
        &["yes"],
        &["no"],
        &[
            ("mark_a", a, x, R, "find_b"),
            ("mark_a", y, y, R, "check_y"),
            ("mark_a", BLANK, BLANK, S, "yes"),
            ("find_b", a, a, R, "find_b"),
            ("find_b", y, y, R, "find_b"),
            ("find_b", b, y, R, "find_c"),
            ("find_c", b, b, R, "find_c"),
            ("find_c", z, z, R, "find_c"),
            ("find_c", c, z, L, "back"),
            ("back", a, a, L, "back"),
            ("back", b, b, L, "back"),
            ("back", y, y, L, "back"),
            ("back", z, z, L, "back"),
            ("back", x, x, R, "mark_a"),
            ("check_y", y, y, R, "check_y"),
            ("check_y", z, z, R, "check_z"),
            ("check_z", z, z, R, "check_z"),
            ("check_z", BLANK, BLANK, S, "yes"),
        ],
        Some(&[a, b, c, x, y, z, BLANK]),
    )
}

/// The letters `anbncn` writes as marks, which its symbols must not be.
pub const MARKS: [char; 3] = ['x', 'y', 'z'];

/// The symbols a machine can meet on its tape: its alphabet, the blank,
/// and everything it writes.
pub fn tape_symbols(tm: &Tm) -> BTreeSet<char> {
    let mut symbols: BTreeSet<char> = tm.alphabet.iter().copied().collect();
    symbols.insert(BLANK);
    for m in &tm.moves {
        symbols.insert(m.read);
        symbols.insert(m.write);
    }
    symbols
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::tm::{Halt, STEP_LIMIT};

    fn output(tm: &Tm, input: &str) -> String {
        let run = tm.run(input, STEP_LIMIT, false);
        assert_eq!(run.halt, Halt::Accepted, "{input:?}");
        run.last().tape.contents()
    }

    fn verdict(tm: &Tm, input: &str) -> bool {
        let run = tm.run(input, STEP_LIMIT, false);
        match run.halt {
            Halt::Accepted => true,
            Halt::Rejected => false,
            other => panic!("{input:?}: {other:?}, in {}", tm.states[run.last().state]),
        }
    }

    fn all_words(alphabet: &[char], n: usize) -> Vec<String> {
        let mut all = vec![String::new()];
        let mut layer = vec![String::new()];
        for _ in 0..n {
            layer = layer.iter().flat_map(|w| alphabet.iter().map(move |c| format!("{w}{c}"))).collect();
            all.extend(layer.iter().cloned());
        }
        all
    }

    #[test]
    fn arithmetic_leaves_the_answer_on_the_tape() {
        for n in 0..70u64 {
            let bits = format!("{n:b}");
            assert_eq!(output(&increment(), &bits), format!("{:b}", n + 1));
        }
        assert_eq!(output(&increment(), ""), "1");
        for m in 0..6 {
            for n in 0..6 {
                let input = format!("{}0{}", "1".repeat(m), "1".repeat(n));
                assert_eq!(output(&unary_add(), &input), "1".repeat(m + n), "{input}");
            }
            assert_eq!(output(&unary_copy(), &"1".repeat(m)), format!("{}0{}", "1".repeat(m), "1".repeat(m)));
        }
        for bad in ["", "11", "1010", "00"] {
            assert!(!verdict(&unary_add(), bad), "{bad}");
        }
    }

    #[test]
    fn deciders_decide_their_languages() {
        let pal = palindromes(&['a', 'b']);
        for w in all_words(&['a', 'b'], 7) {
            assert_eq!(verdict(&pal, &w), w.chars().rev().collect::<String>() == w, "{w}");
        }
        let abc = anbncn('a', 'b', 'c');
        for w in all_words(&['a', 'b', 'c'], 7) {
            let n = w.len() / 3;
            let expected = w == format!("{}{}{}", "a".repeat(n), "b".repeat(n), "c".repeat(n));
            assert_eq!(verdict(&abc, &w), expected, "{w}");
        }
        assert!(verdict(&abc, "aaabbbccc"));
    }

    #[test]
    fn deciders_never_get_stuck() {
        for tm in [palindromes(&['0', '1']), anbncn('a', 'b', 'c'), unary_add()] {
            for state in 0..tm.states.len() {
                if tm.accept.contains(&state) || tm.reject.contains(&state) {
                    continue;
                }
                for c in tape_symbols(&tm) {
                    assert!(tm.move_for(state, c).is_some(), "{} has no move on {c}", tm.states[state]);
                }
            }
        }
    }

    #[test]
    fn runs_count_the_cells_they_visit() {
        let run = increment().run("111", STEP_LIMIT, false);
        // The head goes one past the end, then back past the start.
        assert_eq!(run.cells, 5);
        assert_eq!(increment().run("", STEP_LIMIT, false).cells, 2);
    }
}
