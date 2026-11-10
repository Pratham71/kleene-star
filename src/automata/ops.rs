//! Operations on languages: the closure properties of regular languages,
//! plus the questions you can ask about them.

use super::{fresh_name, Dfa, Nfa, State};
use std::collections::{BTreeMap, VecDeque};

/// The product construction: runs both DFAs side by side. A pair `(p,q)`
/// accepts when `accept(p accepts, q accepts)` says so, which gives union
/// (`||`), intersection (`&&`), difference and more from one function.
/// Both machines must have the same alphabet.
pub fn product(a: &Dfa, b: &Dfa, accept: fn(bool, bool) -> bool) -> Dfa {
    assert_eq!(a.alphabet, b.alphabet, "product needs equal alphabets");
    let mut index: BTreeMap<(State, State), State> = BTreeMap::from([((a.start, b.start), 0)]);
    let mut pairs = vec![(a.start, b.start)];
    let mut delta: Vec<Vec<State>> = Vec::new();
    let mut next = 0;
    while next < pairs.len() {
        let (p, q) = pairs[next];
        let mut row = Vec::new();
        for i in 0..a.alphabet.len() {
            let target = (a.delta[p][i], b.delta[q][i]);
            let id = *index.entry(target).or_insert_with(|| {
                pairs.push(target);
                pairs.len() - 1
            });
            row.push(id);
        }
        delta.push(row);
        next += 1;
    }
    Dfa {
        alphabet: a.alphabet.clone(),
        states: pairs.iter().map(|&(p, q)| format!("({},{})", a.states[p], b.states[q])).collect(),
        start: 0,
        accept: (0..pairs.len())
            .filter(|&i| accept(a.accept.contains(&pairs[i].0), b.accept.contains(&pairs[i].1)))
            .collect(),
        delta,
    }
}

pub fn union(a: &Dfa, b: &Dfa) -> Dfa {
    product(a, b, |x, y| x || y)
}

pub fn intersect(a: &Dfa, b: &Dfa) -> Dfa {
    product(a, b, |x, y| x && y)
}

pub fn difference(a: &Dfa, b: &Dfa) -> Dfa {
    product(a, b, |x, y| x && !y)
}

/// Swaps accepting and non-accepting states. This only works because a `Dfa`
/// is complete: every word ends in exactly one state.
pub fn complement(dfa: &Dfa) -> Dfa {
    let mut out = dfa.clone();
    out.accept = (0..dfa.states.len()).filter(|s| !dfa.accept.contains(s)).collect();
    out
}

/// True when the machine accepts no word at all.
pub fn is_empty(dfa: &Dfa) -> bool {
    dfa.reachable().iter().all(|s| !dfa.accept.contains(s))
}

/// Two DFAs are equivalent when no word is accepted by exactly one of them.
pub fn equivalent(a: &Dfa, b: &Dfa) -> bool {
    is_empty(&product(a, b, |x, y| x != y))
}

/// The shortest word accepted by exactly one of the two machines, if any.
pub fn distinguishing_word(a: &Dfa, b: &Dfa) -> Option<String> {
    let diff = product(a, b, |x, y| x != y);
    let mut word_to: Vec<Option<String>> = vec![None; diff.states.len()];
    word_to[diff.start] = Some(String::new());
    let mut queue = VecDeque::from([diff.start]);
    while let Some(s) = queue.pop_front() {
        let word = word_to[s].clone().unwrap();
        if diff.accept.contains(&s) {
            return Some(word);
        }
        for (i, &t) in diff.delta[s].iter().enumerate() {
            if word_to[t].is_none() {
                word_to[t] = Some(format!("{word}{}", diff.alphabet[i]));
                queue.push_back(t);
            }
        }
    }
    None
}

/// Copies the states and edges of `from` into `into`, renaming on clashes.
/// Returns the offset to add to a state of `from` to find its copy.
fn absorb(into: &mut Nfa, from: &Nfa) -> usize {
    let offset = into.states.len();
    for name in &from.states {
        let name = fresh_name(&into.states, name);
        into.add_state(name);
    }
    for (s, edges) in from.delta.iter().enumerate() {
        for (label, targets) in edges {
            for &t in targets {
                into.add_edge(s + offset, *label, t + offset);
            }
        }
    }
    offset
}

/// Concatenation: every accepting state of `a` gets an epsilon move to the
/// start of `b`, and only `b`'s accepting states still accept.
pub fn concat(a: &Nfa, b: &Nfa) -> Nfa {
    let mut out = Nfa::new(a.alphabet.clone());
    absorb(&mut out, a);
    let offset = absorb(&mut out, b);
    for &s in &a.accept {
        out.add_edge(s, None, b.start + offset);
    }
    out.start = a.start;
    out.accept = b.accept.iter().map(|&s| s + offset).collect();
    out
}

/// Kleene star: a new accepting start state (for the empty word) leads into
/// the machine, and every accepting state can jump back to it.
pub fn star(a: &Nfa) -> Nfa {
    let mut out = Nfa::new(a.alphabet.clone());
    let start = out.add_state(fresh_name(&a.states, "s"));
    let offset = absorb(&mut out, a);
    out.add_edge(start, None, a.start + offset);
    for &s in &a.accept {
        out.add_edge(s + offset, None, start);
    }
    out.start = start;
    out.accept.insert(start);
    out
}

/// Reversal: flip every edge, start where the machine used to accept, and
/// accept where it used to start.
pub fn reverse(a: &Nfa) -> Nfa {
    let mut out = Nfa::new(a.alphabet.clone());
    let start = out.add_state(fresh_name(&a.states, "s"));
    for name in &a.states {
        out.add_state(name.clone());
    }
    for (s, edges) in a.delta.iter().enumerate() {
        for (label, targets) in edges {
            for &t in targets {
                out.add_edge(t + 1, *label, s + 1);
            }
        }
    }
    for &s in &a.accept {
        out.add_edge(start, None, s + 1);
    }
    out.start = start;
    out.accept.insert(a.start + 1);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::convert::{dfa_to_nfa, nfa_to_dfa, regex_to_nfa};
    use crate::automata::dfa::tests::even_zeros;
    use crate::automata::regex::parse;

    fn dfa(regex: &str) -> Dfa {
        nfa_to_dfa(&regex_to_nfa(&parse(regex).unwrap(), &['0', '1']))
    }

    #[test]
    fn boolean_operations() {
        let (even, ends) = (even_zeros(), dfa("(0|1)*01"));
        let (u, i, d) = (union(&even, &ends), intersect(&even, &ends), difference(&even, &ends));
        for w in ["", "0", "01", "001", "0101", "11", "100"] {
            let (x, y) = (even.accepts(w).unwrap(), ends.accepts(w).unwrap());
            assert_eq!(u.accepts(w), Ok(x || y), "union on {w:?}");
            assert_eq!(i.accepts(w), Ok(x && y), "intersect on {w:?}");
            assert_eq!(d.accepts(w), Ok(x && !y), "difference on {w:?}");
            assert_eq!(complement(&even).accepts(w), Ok(!x), "complement on {w:?}");
        }
    }

    #[test]
    fn emptiness_and_equivalence() {
        let even = even_zeros();
        assert!(!is_empty(&even));
        assert!(is_empty(&intersect(&even, &complement(&even))));
        assert!(equivalent(&even, &dfa("1*(01*01*)*")));
        assert!(!equivalent(&even, &dfa("1*")));
        assert_eq!(distinguishing_word(&even, &dfa("1*")), Some("00".to_string()));
        assert_eq!(distinguishing_word(&even, &even), None);
    }

    #[test]
    fn concat_star_reverse() {
        let (a, b) = (dfa_to_nfa(&dfa("01")), dfa_to_nfa(&dfa("1*")));
        assert!(equivalent(&nfa_to_dfa(&concat(&a, &b)), &dfa("011*")));
        assert!(equivalent(&nfa_to_dfa(&star(&a)), &dfa("(01)*")));
        assert!(equivalent(&nfa_to_dfa(&reverse(&dfa_to_nfa(&dfa("001*")))), &dfa("1*00")));
    }

    #[test]
    fn concat_renames_clashing_states() {
        let a = dfa_to_nfa(&even_zeros());
        let both = concat(&a, &a);
        assert_eq!(both.states, vec!["q0", "q1", "q0'", "q1'"]);
    }
}
