//! Making machines smaller.
//!
//! Both functions use partition refinement: start by splitting states into
//! accepting and non-accepting, then keep splitting any group whose members
//! disagree about which groups their transitions lead to. When no group
//! splits, the states inside each group are interchangeable and can be merged.

use super::{Dfa, Nfa, State};
use std::collections::{BTreeMap, BTreeSet};

/// The unique smallest complete DFA for the same language (Moore's algorithm).
pub fn minimize(dfa: &Dfa) -> Dfa {
    let reachable = dfa.reachable(); // unreachable states are simply dropped
    let mut group: Vec<usize> =
        (0..dfa.states.len()).map(|s| usize::from(dfa.accept.contains(&s))).collect();
    let mut count = reachable.iter().map(|&s| group[s]).collect::<BTreeSet<_>>().len();
    loop {
        // A state's signature: its own group, and the group each symbol leads to.
        let mut ids: BTreeMap<(usize, Vec<usize>), usize> = BTreeMap::new();
        let mut refined = group.clone();
        for &s in &reachable {
            let signature = (group[s], dfa.delta[s].iter().map(|&t| group[t]).collect());
            let fresh = ids.len();
            refined[s] = *ids.entry(signature).or_insert(fresh);
        }
        group = refined;
        if ids.len() == count {
            break;
        }
        count = ids.len();
    }

    // Groups are numbered in breadth-first order, so the start's group is 0.
    let mut members: Vec<Vec<State>> = vec![Vec::new(); count];
    for &s in &reachable {
        members[group[s]].push(s);
    }
    for m in &mut members {
        m.sort();
    }
    Dfa {
        alphabet: dfa.alphabet.clone(),
        states: members.iter().map(|m| merged_name(m, &dfa.states)).collect(),
        start: group[dfa.start],
        accept: (0..count).filter(|&g| dfa.accept.contains(&members[g][0])).collect(),
        delta: members
            .iter()
            .map(|m| dfa.delta[m[0]].iter().map(|&t| group[t]).collect())
            .collect(),
    }
}

/// Shrinks an NFA by removing useless states and merging equivalent ones.
///
/// Finding the *smallest* NFA is PSPACE-hard, so this does not promise a
/// minimum: it promises the same language and never more states.
pub fn reduce(nfa: &Nfa) -> Nfa {
    // 1. Keep only states on some path from the start to an accepting state.
    let reachable = nfa.reachable();
    let mut useful = vec![false; nfa.states.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for &s in &reachable {
            let leads_on = nfa.delta[s].values().flatten().any(|&t| useful[t]);
            if !useful[s] && (nfa.accept.contains(&s) || leads_on) {
                useful[s] = true;
                changed = true;
            }
        }
    }
    let kept: Vec<State> =
        reachable.iter().copied().filter(|&s| useful[s] || s == nfa.start).collect();
    let names = kept.iter().map(|&s| nfa.states[s].clone()).collect();
    let trimmed = nfa.remap(&kept, names);
    let n = trimmed.states.len();

    // 2. Partition refinement, where a signature maps each label to the *set*
    //    of groups it can lead to.
    let mut group: Vec<usize> = (0..n).map(|s| usize::from(trimmed.accept.contains(&s))).collect();
    let mut count = group.iter().collect::<BTreeSet<_>>().len();
    loop {
        type Signature = (usize, Vec<(Option<char>, BTreeSet<usize>)>);
        let mut ids: BTreeMap<Signature, usize> = BTreeMap::new();
        let mut refined = vec![0; n];
        for s in 0..n {
            let moves = trimmed.delta[s]
                .iter()
                .map(|(label, targets)| (*label, targets.iter().map(|&t| group[t]).collect()))
                .collect();
            let fresh = ids.len();
            refined[s] = *ids.entry((group[s], moves)).or_insert(fresh);
        }
        group = refined;
        if ids.len() == count {
            break;
        }
        count = ids.len();
    }

    // 3. Build the merged machine.
    let mut members: Vec<Vec<State>> = vec![Vec::new(); count];
    for s in 0..n {
        members[group[s]].push(s);
    }
    let mut out = Nfa::new(trimmed.alphabet.clone());
    for m in &members {
        out.add_state(merged_name(m, &trimmed.states));
    }
    for s in 0..n {
        for (label, targets) in &trimmed.delta[s] {
            for &t in targets {
                out.add_edge(group[s], *label, group[t]);
            }
        }
        if trimmed.accept.contains(&s) {
            out.accept.insert(group[s]);
        }
    }
    out.start = group[trimmed.start];
    out
}

/// A merged state is named after its members: `q1+q3`.
fn merged_name(members: &[State], names: &[String]) -> String {
    members.iter().map(|&s| names[s].as_str()).collect::<Vec<_>>().join("+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::convert::{nfa_to_dfa, regex_to_nfa};
    use crate::automata::ops::equivalent;
    use crate::automata::regex::parse;

    #[test]
    fn merges_equivalent_states() {
        // q1 and q2 behave identically: both accept and both loop forever.
        let dfa = Dfa {
            alphabet: vec!['a'],
            states: vec!["q0".into(), "q1".into(), "q2".into()],
            start: 0,
            accept: BTreeSet::from([1, 2]),
            delta: vec![vec![1], vec![2], vec![1]],
        };
        let min = minimize(&dfa);
        assert_eq!(min.states, vec!["q0", "q1+q2"]);
        assert!(equivalent(&dfa, &min));
    }

    #[test]
    fn drops_unreachable_states() {
        let dfa = Dfa {
            alphabet: vec!['a'],
            states: vec!["q0".into(), "lost".into()],
            start: 0,
            accept: BTreeSet::from([0]),
            delta: vec![vec![0], vec![0]],
        };
        assert_eq!(minimize(&dfa).states, vec!["q0"]);
    }

    #[test]
    fn ends_in_01_needs_three_states() {
        let dfa = nfa_to_dfa(&regex_to_nfa(&parse("(0|1)*01").unwrap(), &['0', '1']));
        let min = minimize(&dfa);
        assert_eq!(min.states.len(), 3);
        assert!(equivalent(&dfa, &min));
        // Minimising twice changes nothing.
        assert_eq!(minimize(&min).states.len(), 3);
    }

    #[test]
    fn reduce_shrinks_a_thompson_nfa() {
        let nfa = regex_to_nfa(&parse("(a|b)*abb").unwrap(), &['a', 'b']);
        let small = reduce(&nfa);
        assert!(small.states.len() < nfa.states.len());
        assert!(equivalent(&nfa_to_dfa(&nfa), &nfa_to_dfa(&small)));
    }

    #[test]
    fn reduce_keeps_the_start_of_an_empty_language() {
        let nfa = regex_to_nfa(&parse("[]").unwrap(), &['a']);
        let small = reduce(&nfa);
        assert_eq!(small.states.len(), 1);
        assert_eq!(small.accepts("a"), Ok(false));
    }
}
