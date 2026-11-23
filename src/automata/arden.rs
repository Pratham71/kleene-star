//! From a finite automaton to a regex by Arden's theorem.
//!
//! Each state `q` gets an equation for the words that lead from the start
//! to it: the sum, over the moves `p --a--> q` into it, of `p a`, plus `eps`
//! when `q` is the start state. Arden's theorem solves an equation of the
//! form `R = Q + RP`, when `P` does not match the empty string, as
//! `R = QP*`. Each state's equation is solved this way in turn and put into
//! the others, until every state is a regex with no states left in it; the
//! language is the sum of the accepting states.
//!
//! The machine must have no eps moves, so that no coefficient `P` matches
//! the empty string and the theorem applies.

use super::{Nfa, Regex, State};
use std::collections::BTreeMap;

/// One equation: `q = constant + sum of p coefficient`.
#[derive(Debug, Clone, PartialEq)]
pub struct Equation {
    pub constant: Regex,
    /// For each state `p` on the right, its coefficient.
    pub terms: BTreeMap<State, Regex>,
}

/// One state solved: its equation before, the `P` Arden's theorem removed
/// (if it had a loop), the solution, and the equations it changed.
#[derive(Debug, Clone, PartialEq)]
pub struct Solved {
    pub state: State,
    pub before: Equation,
    pub looped: Option<Regex>,
    pub after: Equation,
    pub changed: Vec<(State, Equation)>,
}

/// The whole working.
#[derive(Debug, Clone, PartialEq)]
pub struct Working {
    pub names: Vec<String>,
    /// States left out: those the start cannot reach, and those that
    /// cannot reach an accepting state. Their words do not matter.
    pub dropped: Vec<State>,
    pub accept: Vec<State>,
    pub equations: Vec<Equation>,
    pub steps: Vec<Solved>,
    /// Each state's regex at the end.
    pub solutions: Vec<Regex>,
    pub regex: Regex,
}

/// The states on some path from the start to an accepting state.
pub fn useful(nfa: &Nfa) -> Vec<bool> {
    let mut reached = vec![false; nfa.states.len()];
    for s in nfa.reachable() {
        reached[s] = true;
    }
    let mut useful = vec![false; nfa.states.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for s in 0..nfa.states.len() {
            if reached[s] && !useful[s] && (nfa.accept.contains(&s) || nfa.delta[s].values().flatten().any(|&t| useful[t])) {
                useful[s] = true;
                changed = true;
            }
        }
    }
    useful
}

/// The equations of an NFA without eps moves, for its useful states; the
/// others are left as `[]` with nothing on the right.
pub fn equations(nfa: &Nfa) -> Vec<Equation> {
    let useful = useful(nfa);
    let mut eqs: Vec<Equation> = (0..nfa.states.len())
        .map(|q| Equation {
            constant: if q == nfa.start && useful[q] { Regex::Eps } else { Regex::Empty },
            terms: BTreeMap::new(),
        })
        .collect();
    for (p, edges) in nfa.delta.iter().enumerate().filter(|&(p, _)| useful[p]) {
        for (label, targets) in edges {
            let symbol = Regex::Sym(label.expect("no eps moves"));
            for &q in targets.iter().filter(|&&q| useful[q]) {
                let old = eqs[q].terms.remove(&p).unwrap_or(Regex::Empty);
                eqs[q].terms.insert(p, Regex::alt(old, symbol.clone()));
            }
        }
    }
    eqs
}

/// Solves the equations of `nfa`, which must have no eps moves, one state
/// at a time, the cheapest first. Gives up, returning `None`, as soon as a regex
/// has more than `max_size` parts.
pub fn solve(nfa: &Nfa, max_size: usize) -> Option<Working> {
    assert!(!nfa.has_eps(), "Arden's theorem needs a machine without eps moves");
    let mut eqs = equations(nfa);
    let equations = eqs.clone();
    let mut steps = Vec::new();
    let useful = useful(nfa);
    let dropped = (0..eqs.len()).filter(|&s| !useful[s]).collect();
    let mut remaining: Vec<State> = (0..eqs.len()).filter(|&s| useful[s]).collect();
    while !remaining.is_empty() {
        // The order does not change the language, but it changes the size
        // of the answer a great deal, as in state elimination: solve first
        // the state that is cheapest to put into the others, the earliest
        // of those that cost the same.
        let cost = |k: State| {
            let used: usize = eqs.iter().enumerate().filter(|&(i, _)| i != k).filter_map(|(_, e)| e.terms.get(&k)).map(Regex::size).sum();
            let own = size(&eqs[k]);
            used.saturating_mul(own).saturating_add(own)
        };
        let (index, &k) = remaining.iter().enumerate().min_by_key(|&(_, &k)| cost(k)).expect("not empty");
        remaining.remove(index);
        let before = eqs[k].clone();
        let mut eq = before.clone();
        // R = Q + RP becomes R = QP*: every other part gets P* after it.
        let looped = eq.terms.remove(&k);
        if let Some(p) = &looped {
            let star = Regex::star(p.clone());
            eq.constant = Regex::concat(eq.constant, star.clone());
            for coefficient in eq.terms.values_mut() {
                *coefficient = Regex::concat(std::mem::replace(coefficient, Regex::Empty), star.clone());
            }
        }
        // Put the solution into every equation that uses state k.
        let mut changed = Vec::new();
        for i in (0..eqs.len()).filter(|&i| i != k) {
            let Some(r) = eqs[i].terms.remove(&k) else { continue };
            let constant = std::mem::replace(&mut eqs[i].constant, Regex::Empty);
            eqs[i].constant = Regex::alt(constant, Regex::concat(eq.constant.clone(), r.clone()));
            for (&j, a) in &eq.terms {
                let old = eqs[i].terms.remove(&j).unwrap_or(Regex::Empty);
                eqs[i].terms.insert(j, Regex::alt(old, Regex::concat(a.clone(), r.clone())));
            }
            if size(&eqs[i]) > max_size {
                return None;
            }
            changed.push((i, eqs[i].clone()));
        }
        eqs[k] = eq.clone();
        steps.push(Solved { state: k, before, looped, after: eq, changed });
    }
    let solutions: Vec<Regex> = eqs.into_iter().map(|e| e.constant).collect();
    let accept: Vec<State> = nfa.accept.iter().copied().collect();
    let regex = accept.iter().fold(Regex::Empty, |r, &q| Regex::alt(r, solutions[q].clone()));
    Some(Working { names: nfa.states.clone(), dropped, accept, equations, steps, solutions, regex })
}

fn size(eq: &Equation) -> usize {
    eq.constant.size() + eq.terms.values().map(Regex::size).sum::<usize>()
}

/// A regex as a coefficient after a state's name: in brackets if it is a
/// choice, so `p (a|b)` is not read as `p a | b`.
fn coefficient(r: &Regex) -> String {
    let text = if *r == Regex::Eps { "eps".to_string() } else { r.to_string() };
    if matches!(r, Regex::Alt(..)) && !text.ends_with('?') {
        format!("({text})")
    } else {
        text
    }
}

/// An equation written out: `q1 = eps + q0 a + q1 (a|b)`.
pub fn show(names: &[String], q: State, eq: &Equation) -> String {
    let mut parts = Vec::new();
    if eq.constant != Regex::Empty {
        parts.push(coefficient(&eq.constant));
    }
    for (&p, r) in &eq.terms {
        parts.push(format!("{} {}", names[p], coefficient(r)));
    }
    if parts.is_empty() {
        parts.push("[]".to_string());
    }
    // A choice standing alone needs no brackets.
    if parts.len() == 1 && parts[0].starts_with('(') && eq.terms.is_empty() {
        parts[0] = if eq.constant == Regex::Eps { "eps".to_string() } else { eq.constant.to_string() };
    }
    format!("{} = {}", names[q], parts.join(" + "))
}

/// The working as text.
pub fn render(w: &Working) -> String {
    let mut lines = Vec::new();
    if !w.dropped.is_empty() {
        let names: Vec<&str> = w.dropped.iter().map(|&q| w.names[q].as_str()).collect();
        lines.push(format!("Left out, as they are not on a path from the start to an accepting state: {}", names.join(", ")));
    }
    if w.steps.is_empty() {
        lines.push("No accepting state can be reached, so the language is empty.".to_string());
        lines.push(format!("Regex: {}", w.regex));
        return lines.join("\n");
    }
    lines.push("The equations, one for the words that lead to each state:".to_string());
    let kept = w.equations.iter().enumerate().filter(|(q, _)| !w.dropped.contains(q));
    lines.extend(kept.map(|(q, eq)| format!("  {}", show(&w.names, q, eq))));
    for step in &w.steps {
        let name = &w.names[step.state];
        match &step.looped {
            Some(p) => {
                lines.push(format!(
                    "Solve {name}: {} has the form R = Q + RP with P = {}, so by Arden's theorem R = QP*:",
                    show(&w.names, step.state, &step.before),
                    coefficient(p)
                ));
            }
            None => lines.push(format!("Solve {name}: it does not refer to itself, so it stays as it is:")),
        }
        lines.push(format!("  {}", show(&w.names, step.state, &step.after)));
        if !step.changed.is_empty() {
            lines.push(format!("Put {name} into the equations that use it:"));
            lines.extend(step.changed.iter().map(|(q, eq)| format!("  {}", show(&w.names, *q, eq))));
        }
    }
    let finals: Vec<&str> = w.accept.iter().filter(|q| !w.dropped.contains(q)).map(|&q| w.names[q].as_str()).collect();
    match finals.len() {
        0 => lines.push("No state is accepting, so the language is empty.".to_string()),
        1 => lines.push(format!("The accepting state is {}, so the language is its regex.", finals[0])),
        _ => lines.push(format!("The accepting states are {}, so the language is the sum of their regexes.", finals.join(", "))),
    }
    lines.push(format!("Regex: {}", w.regex));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::convert::{dfa_to_nfa, nfa_to_dfa, regex_to_nfa, remove_eps};
    use crate::automata::dfa::tests::even_zeros;
    use crate::automata::nfa::tests::ends_in_01;
    use crate::automata::minimize::minimize;
    use crate::automata::ops::equivalent;
    use crate::automata::regex::parse;

    fn same_language(nfa: &Nfa) {
        let w = solve(nfa, usize::MAX).unwrap();
        let theirs = nfa_to_dfa(nfa);
        let ours = nfa_to_dfa(&regex_to_nfa(&w.regex, &nfa.alphabet));
        assert!(equivalent(&ours, &theirs), "{}", w.regex);
    }

    #[test]
    fn arden_gives_the_same_language() {
        same_language(&dfa_to_nfa(&even_zeros()));
        same_language(&ends_in_01());
        for r in ["(0|1)*01", "0*1*", "(01|10)*", "1(0|1)*0?", "[]", "()"] {
            // The NFA left after removing eps moves from Thompson's
            // construction has many states; its minimal DFA has few.
            let nfa = remove_eps(&regex_to_nfa(&parse(r).unwrap(), &['0', '1']));
            same_language(&dfa_to_nfa(&minimize(&nfa_to_dfa(&nfa))));
        }
    }

    #[test]
    fn a_loop_becomes_a_star() {
        // q0 = eps + q0 0 + q1 1, q1 = q0 1 + q1 0.
        let w = solve(&dfa_to_nfa(&even_zeros()), usize::MAX).unwrap();
        assert_eq!(w.steps[0].looped, Some(Regex::Sym('1')));
        let text = render(&w);
        assert!(text.contains("by Arden's theorem R = QP*"), "{text}");
        assert!(text.ends_with(&format!("Regex: {}", w.regex)), "{text}");
    }

    #[test]
    fn leaves_out_useless_states() {
        let mut nfa = ends_in_01();
        let lost = nfa.add_state("lost");
        nfa.add_edge(nfa.start, Some('1'), lost);
        let w = solve(&nfa, usize::MAX).unwrap();
        assert_eq!(w.dropped, vec![lost]);
        assert!(render(&w).starts_with("Left out, as they are not on a path from the start to an accepting state: lost
"));
        nfa.accept.clear();
        let w = solve(&nfa, usize::MAX).unwrap();
        assert_eq!(w.regex, Regex::Empty);
        assert!(render(&w).ends_with("so the language is empty.
Regex: []"));
    }

    #[test]
    fn gives_up_past_a_size() {
        let n = remove_eps(&regex_to_nfa(&parse("(0|1)*1(0|1)(0|1)(0|1)(0|1)(0|1)").unwrap(), &['0', '1']));
        let d = dfa_to_nfa(&nfa_to_dfa(&n));
        assert!(solve(&d, 1000).is_none());
    }
}
