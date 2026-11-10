//! Conversions between the three descriptions of a regular language:
//!
//! ```text
//! regex --Thompson--> eps-NFA --remove eps--> NFA --subset--> DFA --elimination--> regex
//! ```

use super::{Dfa, Nfa, Regex, State};
use std::collections::{BTreeMap, BTreeSet};

/// Thompson's construction: one small NFA fragment per regex node, glued
/// together with epsilon moves. The result has exactly one accepting state.
pub fn regex_to_nfa(regex: &Regex, alphabet: &[char]) -> Nfa {
    let mut nfa = Nfa::new(alphabet.to_vec());
    let (start, end) = fragment(regex, &mut nfa);
    nfa.start = start;
    nfa.accept.insert(end);
    nfa.renamed()
}

/// Adds the fragment for `regex` to `nfa` and returns its (entry, exit) states.
fn fragment(regex: &Regex, nfa: &mut Nfa) -> (State, State) {
    let fresh = |nfa: &mut Nfa| {
        let name = format!("n{}", nfa.states.len());
        nfa.add_state(name)
    };
    match regex {
        Regex::Empty => (fresh(nfa), fresh(nfa)),
        Regex::Eps => {
            let (s, e) = (fresh(nfa), fresh(nfa));
            nfa.add_edge(s, None, e);
            (s, e)
        }
        Regex::Sym(c) => {
            let (s, e) = (fresh(nfa), fresh(nfa));
            nfa.add_edge(s, Some(*c), e);
            (s, e)
        }
        Regex::Any => {
            let (s, e) = (fresh(nfa), fresh(nfa));
            for c in nfa.alphabet.clone() {
                nfa.add_edge(s, Some(c), e);
            }
            (s, e)
        }
        Regex::Concat(a, b) => {
            let (sa, ea) = fragment(a, nfa);
            let (sb, eb) = fragment(b, nfa);
            nfa.add_edge(ea, None, sb);
            (sa, eb)
        }
        Regex::Alt(a, b) => {
            let s = fresh(nfa);
            let (sa, ea) = fragment(a, nfa);
            let (sb, eb) = fragment(b, nfa);
            let e = fresh(nfa);
            nfa.add_edge(s, None, sa);
            nfa.add_edge(s, None, sb);
            nfa.add_edge(ea, None, e);
            nfa.add_edge(eb, None, e);
            (s, e)
        }
        Regex::Star(a) => {
            let s = fresh(nfa);
            let (sa, ea) = fragment(a, nfa);
            let e = fresh(nfa);
            nfa.add_edge(s, None, sa); // enter the loop
            nfa.add_edge(s, None, e); // or skip it
            nfa.add_edge(ea, None, sa); // go round again
            nfa.add_edge(ea, None, e); // or leave
            (s, e)
        }
    }
}

/// A regex for a finite set of words: `w1 | w2 | ...`.
pub fn finite_to_regex<'a>(words: impl Iterator<Item = &'a str>) -> Regex {
    words.fold(Regex::Empty, |acc, word| {
        let w = word.chars().fold(Regex::Eps, |r, c| Regex::concat(r, Regex::Sym(c)));
        Regex::alt(acc, w)
    })
}

/// Removes epsilon moves without changing the language. State `q` gets a
/// transition on `a` to everything reachable by: eps*, then `a`, then eps*.
pub fn remove_eps(nfa: &Nfa) -> Nfa {
    let mut out = Nfa::new(nfa.alphabet.clone());
    for name in &nfa.states {
        out.add_state(name.clone());
    }
    out.start = nfa.start;
    for s in 0..nfa.states.len() {
        let closure = nfa.eps_closure(&BTreeSet::from([s]));
        if closure.iter().any(|q| nfa.accept.contains(q)) {
            out.accept.insert(s);
        }
        for &c in &nfa.alphabet {
            for t in nfa.step(&closure, c) {
                out.add_edge(s, Some(c), t);
            }
        }
    }
    // Some states were only reachable through epsilon moves; drop them.
    let order = out.reachable();
    let names = order.iter().map(|&s| out.states[s].clone()).collect();
    out.remap(&order, names)
}

/// The subset construction. Each DFA state is the set of NFA states the NFA
/// could be in, and is named after that set, e.g. `{a,b}`. The empty set `{}`
/// is the dead state.
pub fn nfa_to_dfa(nfa: &Nfa) -> Dfa {
    let start = nfa.eps_closure(&BTreeSet::from([nfa.start]));
    let mut index: BTreeMap<BTreeSet<State>, State> = BTreeMap::from([(start.clone(), 0)]);
    let mut subsets = vec![start];
    let mut delta: Vec<Vec<State>> = Vec::new();
    let mut next = 0;
    while next < subsets.len() {
        let current = subsets[next].clone();
        let mut row = Vec::new();
        for &c in &nfa.alphabet {
            let target = nfa.step(&current, c);
            let id = match index.get(&target) {
                Some(&id) => id,
                None => {
                    let id = subsets.len();
                    index.insert(target.clone(), id);
                    subsets.push(target);
                    id
                }
            };
            row.push(id);
        }
        delta.push(row);
        next += 1;
    }
    Dfa {
        alphabet: nfa.alphabet.clone(),
        states: subsets
            .iter()
            .map(|set| {
                let names: Vec<&str> = set.iter().map(|&s| nfa.states[s].as_str()).collect();
                format!("{{{}}}", names.join(","))
            })
            .collect(),
        start: 0,
        accept: (0..subsets.len())
            .filter(|&i| subsets[i].iter().any(|s| nfa.accept.contains(s)))
            .collect(),
        delta,
    }
}

/// Every DFA is already an NFA; this just changes the representation.
pub fn dfa_to_nfa(dfa: &Dfa) -> Nfa {
    let mut nfa = Nfa::new(dfa.alphabet.clone());
    for name in &dfa.states {
        nfa.add_state(name.clone());
    }
    for (s, row) in dfa.delta.iter().enumerate() {
        for (i, &t) in row.iter().enumerate() {
            nfa.add_edge(s, Some(dfa.alphabet[i]), t);
        }
    }
    nfa.start = dfa.start;
    nfa.accept = dfa.accept.clone();
    nfa
}

/// State elimination. Edges are labelled with regexes; states are removed one
/// at a time, and each removal re-routes the paths that went through it:
/// `i --a--> k --b--> j` with a loop `c` on `k` becomes `i --a c* b--> j`.
pub fn dfa_to_regex(dfa: &Dfa) -> Regex {
    // Fewer states means a much shorter regex, so minimise first.
    let dfa = super::minimize::minimize(dfa);
    let n = dfa.states.len();

    // Only states that lie on some accepting path matter.
    let reachable = dfa.reachable();
    let mut useful = vec![false; n];
    let mut changed = true;
    while changed {
        changed = false;
        for &s in &reachable {
            if !useful[s] && (dfa.accept.contains(&s) || dfa.delta[s].iter().any(|&t| useful[t])) {
                useful[s] = true;
                changed = true;
            }
        }
    }
    if !useful[dfa.start] {
        return Regex::Empty;
    }

    // edge[i][j] is the regex on the edge i -> j. Two extra states are added:
    // a new start with an epsilon edge to the old start, and a new final state
    // reached by epsilon from every accepting state.
    let (new_start, new_final) = (n, n + 1);
    let mut edge = vec![vec![Regex::Empty; n + 2]; n + 2];
    for s in (0..n).filter(|&s| useful[s]) {
        for (i, &t) in dfa.delta[s].iter().enumerate() {
            if useful[t] {
                let old = std::mem::replace(&mut edge[s][t], Regex::Empty);
                edge[s][t] = Regex::alt(old, Regex::Sym(dfa.alphabet[i]));
            }
        }
        if dfa.accept.contains(&s) {
            edge[s][new_final] = Regex::Eps;
        }
    }
    edge[new_start][dfa.start] = Regex::Eps;

    let mut alive: Vec<State> = (0..n).filter(|&s| useful[s]).collect();
    alive.extend([new_start, new_final]);
    // Eliminate from the highest-numbered state down, so the start goes last.
    for k in (0..n).rev().filter(|&s| useful[s]) {
        alive.retain(|&s| s != k);
        let looped = Regex::star(edge[k][k].clone());
        for &i in &alive {
            if edge[i][k] == Regex::Empty {
                continue;
            }
            for &j in &alive {
                if edge[k][j] == Regex::Empty {
                    continue;
                }
                let through = Regex::concat(
                    edge[i][k].clone(),
                    Regex::concat(looped.clone(), edge[k][j].clone()),
                );
                let old = std::mem::replace(&mut edge[i][j], Regex::Empty);
                edge[i][j] = Regex::alt(old, through);
            }
        }
    }
    edge[new_start][new_final].clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::tests::even_zeros;
    use crate::automata::nfa::tests::ends_in_01;
    use crate::automata::ops::equivalent;
    use crate::automata::regex::parse;

    const WORDS: [&str; 9] = ["", "0", "1", "01", "10", "001", "1101", "0100", "111101"];

    #[test]
    fn thompson_matches_the_regex() {
        let nfa = regex_to_nfa(&parse("(0|1)*01").unwrap(), &['0', '1']);
        for w in WORDS {
            assert_eq!(nfa.accepts(w), Ok(w.ends_with("01")), "word {w:?}");
        }
        assert_eq!(nfa.accept.len(), 1);
        assert_eq!(nfa.states[nfa.start], "q0");
    }

    #[test]
    fn subset_construction_keeps_the_language() {
        let nfa = ends_in_01();
        let dfa = nfa_to_dfa(&nfa);
        assert_eq!(dfa.states, vec!["{a}", "{a,b}", "{a,c}"]);
        for w in WORDS {
            assert_eq!(dfa.accepts(w), nfa.accepts(w), "word {w:?}");
        }
    }

    #[test]
    fn subset_construction_adds_a_dead_state() {
        let dfa = nfa_to_dfa(&regex_to_nfa(&parse("01").unwrap(), &['0', '1']));
        assert!(dfa.states.contains(&"{}".to_string()));
        assert_eq!(dfa.accepts("011"), Ok(false));
    }

    #[test]
    fn removing_eps_keeps_the_language() {
        let with_eps = regex_to_nfa(&parse("(0|1)*01").unwrap(), &['0', '1']);
        let without = remove_eps(&with_eps);
        assert!(with_eps.has_eps() && !without.has_eps());
        for w in WORDS {
            assert_eq!(without.accepts(w), with_eps.accepts(w), "word {w:?}");
        }
    }

    #[test]
    fn dfa_to_regex_round_trips() {
        for dfa in [even_zeros(), nfa_to_dfa(&ends_in_01())] {
            let regex = dfa_to_regex(&dfa);
            let back = nfa_to_dfa(&regex_to_nfa(&regex, &dfa.alphabet));
            assert!(equivalent(&dfa, &back), "regex {regex}");
        }
    }

    #[test]
    fn regex_of_the_empty_language() {
        let nothing = nfa_to_dfa(&regex_to_nfa(&Regex::Empty, &['0']));
        assert_eq!(dfa_to_regex(&nothing), Regex::Empty);
    }

    #[test]
    fn finite_language() {
        let regex = finite_to_regex(["a", "ab", ""].into_iter());
        let nfa = regex_to_nfa(&regex, &['a', 'b']);
        assert_eq!(nfa.accepts("ab"), Ok(true));
        assert_eq!(nfa.accepts(""), Ok(true));
        assert_eq!(nfa.accepts("b"), Ok(false));
    }
}
