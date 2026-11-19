//! Questions about the language of a DFA that the built-ins do not answer:
//! its shortest word, whether it is finite, how many words of a length it
//! has, and the list of its short words.
//!
//! All of them look at the DFA's graph rather than at words one by one, so
//! they finish quickly even when the language is infinite.

use super::Dfa;
use std::collections::VecDeque;

/// The shortest word the DFA accepts, and among those the first in alphabet
/// order; `None` if it accepts nothing.
///
/// A breadth-first search from the start visits states in order of the
/// shortest word that reaches them, and trying the symbols in alphabet
/// order makes that word the first in alphabet order too.
pub fn shortest_word(dfa: &Dfa) -> Option<String> {
    let mut word_to: Vec<Option<String>> = vec![None; dfa.states.len()];
    word_to[dfa.start] = Some(String::new());
    let mut queue = VecDeque::from([dfa.start]);
    while let Some(s) = queue.pop_front() {
        let word = word_to[s].clone().expect("every queued state has a word");
        if dfa.accept.contains(&s) {
            return Some(word);
        }
        for (i, &t) in dfa.delta[s].iter().enumerate() {
            if word_to[t].is_none() {
                word_to[t] = Some(format!("{word}{}", dfa.alphabet[i]));
                queue.push_back(t);
            }
        }
    }
    None
}

/// The states that lie on the path of some accepted word: reachable from
/// the start, and able to reach an accepting state.
fn useful(dfa: &Dfa) -> Vec<bool> {
    let n = dfa.states.len();
    let mut reachable = vec![false; n];
    for s in dfa.reachable() {
        reachable[s] = true;
    }
    // Work backwards from the accepting states along reversed transitions.
    let mut into: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (s, row) in dfa.delta.iter().enumerate() {
        for &t in row {
            into[t].push(s);
        }
    }
    let mut live = vec![false; n];
    let mut stack: Vec<usize> = dfa.accept.iter().copied().collect();
    for &s in &stack {
        live[s] = true;
    }
    while let Some(t) = stack.pop() {
        for &s in &into[t] {
            if !live[s] {
                live[s] = true;
                stack.push(s);
            }
        }
    }
    (0..n).map(|s| reachable[s] && live[s]).collect()
}

/// Whether the language is finite. It is infinite exactly when some cycle
/// runs through useful states: going round it any number of times gives
/// longer and longer accepted words (this is the pumping lemma at work).
pub fn is_finite(dfa: &Dfa) -> bool {
    let useful = useful(dfa);
    // Kahn's algorithm on the graph of useful states: if every state can be
    // removed in an order where it has no incoming edges left, there is no
    // cycle. (Iterative, so a long chain of states cannot overflow the stack.)
    let n = dfa.states.len();
    let mut incoming = vec![0usize; n];
    for s in (0..n).filter(|&s| useful[s]) {
        for &t in dfa.delta[s].iter().filter(|&&t| useful[t]) {
            incoming[t] += 1;
        }
    }
    let mut ready: Vec<usize> = (0..n).filter(|&s| useful[s] && incoming[s] == 0).collect();
    let mut removed = 0;
    while let Some(s) = ready.pop() {
        removed += 1;
        for &t in dfa.delta[s].iter().filter(|&&t| useful[t]) {
            incoming[t] -= 1;
            if incoming[t] == 0 {
                ready.push(t);
            }
        }
    }
    removed == useful.iter().filter(|&&u| u).count()
}

/// How many words of length exactly `n` the DFA accepts, or `None` if the
/// number does not fit in an `i64`.
///
/// `ways[s]` is the number of words of the current length that lead from
/// the start to `s`; one more symbol moves each count along each edge. A
/// DFA follows one path per word, so no word is counted twice.
pub fn count_words(dfa: &Dfa, n: usize) -> Option<i64> {
    let mut ways = vec![0i64; dfa.states.len()];
    ways[dfa.start] = 1;
    for _ in 0..n {
        let mut next = vec![0i64; dfa.states.len()];
        for (s, &count) in ways.iter().enumerate().filter(|(_, &c)| c > 0) {
            for &t in &dfa.delta[s] {
                next[t] = next[t].checked_add(count)?;
            }
        }
        ways = next;
    }
    dfa.accept.iter().try_fold(0i64, |sum, &s| sum.checked_add(ways[s]))
}

/// Every accepted word of length at most `n`, shortest first and then in
/// alphabet order, or `None` if listing them would take more than `budget`
/// characters of work (each word tried costs its length plus one).
///
/// Only words that stay on useful states are extended, so no time is spent
/// on words that can never be accepted.
pub fn words_up_to(dfa: &Dfa, n: usize, budget: usize) -> Option<Vec<String>> {
    let useful = useful(dfa);
    let mut found = Vec::new();
    if !useful[dfa.start] {
        return Some(found);
    }
    let mut spent = 1;
    let mut level = vec![(String::new(), dfa.start)];
    for len in 0..=n {
        // `level` is sorted (the words have one length and were extended in
        // alphabet order), so `found` stays shortest first, then sorted.
        found.extend(level.iter().filter(|(_, s)| dfa.accept.contains(s)).map(|(w, _)| w.clone()));
        if len == n {
            break;
        }
        let mut next = Vec::new();
        for (word, s) in &level {
            for (i, &t) in dfa.delta[*s].iter().enumerate() {
                if useful[t] {
                    spent += len + 2;
                    if spent > budget {
                        return None;
                    }
                    next.push((format!("{word}{}", dfa.alphabet[i]), t));
                }
            }
        }
        level = next;
    }
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::tests::even_zeros;
    use crate::automata::patterns;

    #[test]
    fn shortest_words() {
        assert_eq!(shortest_word(&even_zeros()), Some(String::new()));
        let d = patterns::ends_with(&['0', '1'], &['1', '0']);
        assert_eq!(shortest_word(&d), Some("10".into()));
        let nothing = patterns::length_between(&['0'], 2, 1);
        assert_eq!(shortest_word(&nothing), None);
    }

    #[test]
    fn finite_and_infinite() {
        assert!(!is_finite(&even_zeros()));
        assert!(is_finite(&patterns::length_between(&['0', '1'], 0, 3)));
        assert!(!is_finite(&patterns::starts_with(&['0', '1'], &[])));
        // The loop on `more` (three or more 0s) can never lead to
        // acceptance, so it does not make the language infinite.
        assert!(is_finite(&patterns::count_exactly(&['0'], '0', 2)));
    }

    #[test]
    fn counting_words() {
        assert_eq!(count_words(&even_zeros(), 3), Some(4));
        assert_eq!(count_words(&patterns::length_between(&['0', '1'], 2, 2), 2), Some(4));
    }
}
