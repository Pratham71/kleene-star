//! The working of the conversions, step by step, as text: the tables and
//! lists the usual textbook working writes out by hand.
//!
//! * `eps_closures`: each state's eps-closure.
//! * `subset_table`: the subset construction as a table, with the empty set
//!   written `Ø` as the dead state.
//! * `partitions`: minimisation by k-equivalence, the partitions P0, P1, ...
//! * `elimination`: state elimination, one state removed at a time.
//!
//! Each gives the same answer as the conversion it explains, because each
//! runs the same code: `nfa_to_dfa`, `minimize` and `dfa_to_regex`.

use super::convert::{self, Elimination};
use super::minimize;
use super::{render_table, row_label, Dfa, Nfa, Regex, State};
use std::collections::BTreeSet;

/// The dead state's name in `subset_table`.
pub const DEAD: &str = "Ø";

fn set_name(nfa: &Nfa, set: &BTreeSet<State>) -> String {
    let names: Vec<&str> = set.iter().map(|&s| nfa.states[s].as_str()).collect();
    format!("{{{}}}", names.join(","))
}

/// Each state's eps-closure: the states it reaches by eps moves alone,
/// itself included.
pub fn eps_closures(nfa: &Nfa) -> String {
    let mut rows = vec![vec![String::new(), "eps-closure".to_string()]];
    for (s, name) in nfa.states.iter().enumerate() {
        rows.push(vec![
            row_label(name, s == nfa.start, nfa.accept.contains(&s)),
            set_name(nfa, &nfa.eps_closure(&BTreeSet::from([s]))),
        ]);
    }
    render_table(&rows)
}

/// The subset construction as a table: a row for each set of states reached
/// from the start, in the order they are found, and the set each symbol
/// leads to. The empty set is the dead state, written `Ø`.
pub fn subset_table(nfa: &Nfa) -> String {
    let dfa = convert::nfa_to_dfa(nfa);
    let name = |s: State| if dfa.states[s] == "{}" { DEAD.to_string() } else { dfa.states[s].clone() };
    let start = &nfa.states[nfa.start];
    let mut out = if nfa.has_eps() {
        format!("Start: eps-closure({start}) = {}\n", name(dfa.start))
    } else {
        format!("Start: {}\n", name(dfa.start))
    };
    let mut rows = vec![std::iter::once(String::new()).chain(dfa.alphabet.iter().map(char::to_string)).collect::<Vec<_>>()];
    for (s, row) in dfa.delta.iter().enumerate() {
        let mut cells = vec![row_label(&name(s), s == dfa.start, dfa.accept.contains(&s))];
        cells.extend(row.iter().map(|&t| name(t)));
        rows.push(cells);
    }
    out.push_str(&render_table(&rows));
    let n = dfa.states.len();
    out.push_str(&format!("\nThe DFA has {n} state{}", if n == 1 { "" } else { "s" }));
    if dfa.states.iter().any(|s| s == "{}") {
        out.push_str(&format!("; {DEAD} is the dead state: every symbol leads from it back to it"));
    }
    out.push('.');
    out
}

fn groups(dfa: &Dfa, partition: &[Vec<State>]) -> String {
    let shown: Vec<String> = partition
        .iter()
        .map(|g| {
            let names: Vec<&str> = g.iter().map(|&s| dfa.states[s].as_str()).collect();
            format!("{{{}}}", names.join(", "))
        })
        .collect();
    shown.join(" ")
}

/// Minimisation by k-equivalence: P0 splits the accepting states from the
/// rest, and each P(k+1) splits the groups of Pk by the groups their states
/// go to. The working stops when a partition is the same as the one before.
pub fn partitions(dfa: &Dfa) -> String {
    let mut lines = Vec::new();
    let reachable: BTreeSet<State> = dfa.reachable().into_iter().collect();
    let unreachable: Vec<&str> =
        (0..dfa.states.len()).filter(|s| !reachable.contains(s)).map(|s| dfa.states[s].as_str()).collect();
    if !unreachable.is_empty() {
        lines.push(format!("Unreachable, so dropped first: {}", unreachable.join(", ")));
    }
    let rounds = minimize::partitions(dfa);
    for (k, partition) in rounds.iter().enumerate() {
        lines.push(format!("P{k}: {}", groups(dfa, partition)));
    }
    let last = rounds.len() - 1;
    let count = rounds[last].len();
    lines.push(format!(
        "P{last} = P{}, so the minimal DFA has {count} state{}.",
        last - 1,
        if count == 1 { "" } else { "s" }
    ));
    lines.join("\n")
}

fn label(r: &Regex) -> String {
    if *r == Regex::Eps {
        "eps".to_string()
    } else {
        r.to_string()
    }
}

fn edge(working: &Elimination, (from, to, r): &(State, State, Regex)) -> String {
    format!("  {} --{}--> {}", working.names[*from], label(r), working.names[*to])
}

/// State elimination, as `to_regex` does it: the DFA minimised, a new start
/// and a new final state joined to it by eps moves, then one state removed
/// at a time, each removal replacing the paths through it,
/// `i --a--> k --b--> j` with a loop `c` on `k`, by `i --ac*b--> j`.
pub fn elimination(dfa: &Dfa) -> String {
    let working = convert::dfa_to_regex_steps(dfa);
    let n = working.names.len() - 2;
    let (start, end) = (&working.names[n], &working.names[n + 1]);
    let mut lines = vec![format!(
        "The minimal DFA has {n} state{}: {}.",
        if n == 1 { "" } else { "s" },
        working.names[..n].join(", ")
    )];
    if working.edges.is_empty() {
        lines.push("No accepting state can be reached, so the language is empty.".to_string());
    } else {
        lines.push(format!("Add a new start state {start} and a new final state {end}, joined by eps moves:"));
        lines.extend(working.edges.iter().map(|e| edge(&working, e)));
        for removal in &working.removals {
            let name = &working.names[removal.state];
            lines.push(if removal.looped == Regex::Empty {
                format!("Remove {name}:")
            } else {
                format!("Remove {name}, which loops on {}:", label(&removal.looped))
            });
            if removal.edges.is_empty() {
                lines.push("  (no path goes through it)".to_string());
            }
            lines.extend(removal.edges.iter().map(|e| edge(&working, e)));
        }
    }
    lines.push(format!("Regex: {}", working.regex));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::convert::{dfa_to_regex, nfa_to_dfa, regex_to_nfa};
    use crate::automata::dfa::tests::even_zeros;
    use crate::automata::nfa::tests::ends_in_01;
    use crate::automata::regex::parse;

    #[test]
    fn eps_closures_include_the_state() {
        let nfa = regex_to_nfa(&parse("a*").unwrap(), &['a']);
        let table = eps_closures(&nfa);
        for (s, name) in nfa.states.iter().enumerate() {
            let line = table.lines().nth(s + 1).unwrap();
            assert!(line.contains(name.as_str()), "{table}");
            let closure = line.rsplit('|').next().unwrap();
            assert!(closure.contains(name.as_str()), "{table}");
        }
    }

    #[test]
    fn the_subset_table_names_the_dead_state() {
        // Over {0, 1}, "ends in 01" never needs the empty set; with a
        // symbol nothing reads, it does.
        let mut nfa = ends_in_01();
        assert!(!subset_table(&nfa).contains(DEAD));
        nfa.alphabet.push('2');
        let table = subset_table(&nfa);
        assert!(table.contains(DEAD) && table.ends_with("every symbol leads from it back to it."), "{table}");
        assert_eq!(table.lines().count(), 2 + nfa_to_dfa(&nfa).states.len() + 1, "{table}");
    }

    #[test]
    fn partitions_end_at_the_minimal_size() {
        let text = partitions(&even_zeros());
        assert!(text.starts_with("P0: "), "{text}");
        assert!(text.ends_with("P1 = P0, so the minimal DFA has 2 states."), "{text}");
    }

    #[test]
    fn partitions_split_until_they_settle() {
        // "Ends in 01" as a DFA with a redundant copy of its start state.
        let mut d = nfa_to_dfa(&ends_in_01());
        let rounds = minimize::partitions(&d);
        assert_eq!(rounds.last(), rounds.get(rounds.len() - 2));
        assert_eq!(rounds.last().unwrap().len(), minimize::minimize(&d).states.len());
        d.states.push("lost".into());
        d.delta.push(vec![0, 0]);
        assert!(partitions(&d).starts_with("Unreachable, so dropped first: lost\n"));
    }

    #[test]
    fn elimination_ends_with_the_regex_to_regex_gives() {
        for d in [even_zeros(), nfa_to_dfa(&ends_in_01())] {
            let text = elimination(&d);
            assert!(text.ends_with(&format!("Regex: {}", dfa_to_regex(&d))), "{text}");
            let working = convert::dfa_to_regex_steps(&d);
            assert_eq!(working.removals.len(), working.names.len() - 2);
        }
    }

    #[test]
    fn elimination_of_the_empty_language() {
        let mut d = even_zeros();
        d.accept.clear();
        let text = elimination(&d);
        assert!(text.contains("the language is empty") && text.ends_with("Regex: []"), "{text}");
    }
}
