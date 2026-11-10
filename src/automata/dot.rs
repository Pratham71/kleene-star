//! Drawing machines as Graphviz DOT text.
//!
//! Render the output with `dot -Tsvg machine.dot -o machine.svg`, or paste it
//! into any online Graphviz viewer.

use super::pda::Pda;
use super::{Dfa, Nfa, State};
use std::collections::{BTreeMap, BTreeSet};

pub fn dfa_to_dot(dfa: &Dfa) -> String {
    let mut edges: BTreeMap<(State, State), Vec<String>> = BTreeMap::new();
    for (s, row) in dfa.delta.iter().enumerate() {
        for (i, &t) in row.iter().enumerate() {
            edges.entry((s, t)).or_default().push(dfa.alphabet[i].to_string());
        }
    }
    render(&dfa.states, dfa.start, &dfa.accept, &edges, ", ")
}

pub fn nfa_to_dot(nfa: &Nfa) -> String {
    let mut edges: BTreeMap<(State, State), Vec<String>> = BTreeMap::new();
    for (s, moves) in nfa.delta.iter().enumerate() {
        for (label, targets) in moves {
            for &t in targets {
                let text = label.map_or("\u{3b5}".to_string(), |c| c.to_string()); // epsilon
                edges.entry((s, t)).or_default().push(text);
            }
        }
    }
    render(&nfa.states, nfa.start, &nfa.accept, &edges, ", ")
}

/// A PDA's moves are labelled `a, Z/AZ` (with ε for eps); several moves
/// between the same two states go on separate lines of one label.
pub fn pda_to_dot(pda: &Pda) -> String {
    let mut edges: BTreeMap<(State, State), Vec<String>> = BTreeMap::new();
    for m in &pda.moves {
        let label = pda.label(m).replace("eps", "\u{3b5}");
        edges.entry((m.from, m.to)).or_default().push(label);
    }
    // `\n` inside a DOT label is a line break.
    render(&pda.states, pda.start, &pda.accept, &edges, "\\n")
}

/// Parallel edges are merged into one arrow labelled `0, 1`.
fn render(
    states: &[String],
    start: State,
    accept: &BTreeSet<State>,
    edges: &BTreeMap<(State, State), Vec<String>>,
    separator: &str,
) -> String {
    let quote = |name: &str| format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""));
    let mut out = String::from("digraph machine {\n    rankdir=LR;\n    node [shape=circle];\n");
    out.push_str("    __start [shape=point];\n");
    for (s, name) in states.iter().enumerate() {
        if accept.contains(&s) {
            out.push_str(&format!("    {} [shape=doublecircle];\n", quote(name)));
        } else {
            out.push_str(&format!("    {};\n", quote(name)));
        }
    }
    out.push_str(&format!("    __start -> {};\n", quote(&states[start])));
    for (&(s, t), labels) in edges {
        out.push_str(&format!(
            "    {} -> {} [label=\"{}\"];\n",
            quote(&states[s]),
            quote(&states[t]),
            labels.join(separator)
        ));
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::tests::even_zeros;
    use crate::automata::nfa::tests::ends_in_01;

    #[test]
    fn draws_a_dfa() {
        let dot = dfa_to_dot(&even_zeros());
        assert!(dot.contains("\"q0\" [shape=doublecircle];"));
        assert!(dot.contains("__start -> \"q0\";"));
        assert!(dot.contains("\"q0\" -> \"q1\" [label=\"0\"];"));
    }

    #[test]
    fn merges_parallel_edges() {
        let dot = nfa_to_dot(&ends_in_01());
        assert!(dot.contains("\"a\" -> \"a\" [label=\"0, 1\"];"), "{dot}");
    }
}
