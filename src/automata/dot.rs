//! Drawing machines as Graphviz DOT text.
//!
//! Render the output with `dot -Tsvg machine.dot -o machine.svg`, or paste it
//! into any online Graphviz viewer.

use super::pda::Pda;
use super::tm::Tm;
use super::{Dfa, Nfa, State};
use std::collections::{BTreeMap, BTreeSet};

/// A machine as a graph: its states, which are the start, accepting and
/// rejecting ones, and its moves, with parallel moves between two states
/// joined into one edge with several labels. `draw`, `std.visual` and the
/// player (`draw M on "w"`) all draw from this.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    pub states: Vec<String>,
    pub start: State,
    pub accept: BTreeSet<State>,
    /// Only a Turing machine has rejecting states.
    pub reject: BTreeSet<State>,
    pub edges: BTreeMap<(State, State), Vec<String>>,
    /// Between the labels of one edge in DOT: `, ` for symbols, a line
    /// break (`\n`) for PDA and Turing machine moves.
    pub separator: &'static str,
}

/// What `render` shows in colour: the states and edges a run used, and the
/// states it ended in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Highlight {
    pub visited: BTreeSet<State>,
    pub edges: BTreeSet<(State, State)>,
    pub last: BTreeSet<State>,
}

pub fn dfa_diagram(dfa: &Dfa) -> Diagram {
    let mut edges: BTreeMap<(State, State), Vec<String>> = BTreeMap::new();
    for (s, row) in dfa.delta.iter().enumerate() {
        for (i, &t) in row.iter().enumerate() {
            edges.entry((s, t)).or_default().push(dfa.alphabet[i].to_string());
        }
    }
    Diagram { states: dfa.states.clone(), start: dfa.start, accept: dfa.accept.clone(), reject: BTreeSet::new(), edges, separator: ", " }
}

pub fn nfa_diagram(nfa: &Nfa) -> Diagram {
    let mut edges: BTreeMap<(State, State), Vec<String>> = BTreeMap::new();
    for (s, moves) in nfa.delta.iter().enumerate() {
        for (label, targets) in moves {
            for &t in targets {
                let text = label.map_or("\u{3b5}".to_string(), |c| c.to_string()); // epsilon
                edges.entry((s, t)).or_default().push(text);
            }
        }
    }
    Diagram { states: nfa.states.clone(), start: nfa.start, accept: nfa.accept.clone(), reject: BTreeSet::new(), edges, separator: ", " }
}

/// A PDA's moves are labelled `a, Z/AZ` (with ε for eps); several moves
/// between the same two states go on separate lines of one label.
pub fn pda_diagram(pda: &Pda) -> Diagram {
    let mut edges: BTreeMap<(State, State), Vec<String>> = BTreeMap::new();
    for m in &pda.moves {
        let label = pda.label(m).replace("eps", "\u{3b5}");
        edges.entry((m.from, m.to)).or_default().push(label);
    }
    // `\n` inside a DOT label is a line break.
    Diagram { states: pda.states.clone(), start: pda.start, accept: pda.accept.clone(), reject: BTreeSet::new(), edges, separator: "\\n" }
}

/// A Turing machine's moves are labelled `0/1,R`. Rejecting states are
/// drawn as boxes so they stand out from ordinary states.
pub fn tm_diagram(m: &Tm) -> Diagram {
    let mut edges: BTreeMap<(State, State), Vec<String>> = BTreeMap::new();
    for mv in &m.moves {
        edges.entry((mv.from, mv.to)).or_default().push(m.label(mv));
    }
    Diagram { states: m.states.clone(), start: m.start, accept: m.accept.clone(), reject: m.reject.clone(), edges, separator: "\\n" }
}

pub fn dfa_to_dot(dfa: &Dfa) -> String {
    render(&dfa_diagram(dfa), None)
}

pub fn nfa_to_dot(nfa: &Nfa) -> String {
    render(&nfa_diagram(nfa), None)
}

pub fn pda_to_dot(pda: &Pda) -> String {
    render(&pda_diagram(pda), None)
}

pub fn tm_to_dot(m: &Tm) -> String {
    render(&tm_diagram(m), None)
}

/// The DOT text of a diagram. With a highlight, the states and edges a run
/// used are drawn bold and coloured, and the states it ended in filled.
pub fn render(diagram: &Diagram, highlight: Option<&Highlight>) -> String {
    let quote = |name: &str| format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""));
    let mut out = String::from("digraph machine {\n    rankdir=LR;\n    node [shape=circle];\n");
    out.push_str("    __start [shape=point];\n");
    for (s, name) in diagram.states.iter().enumerate() {
        let mut attrs = Vec::new();
        if diagram.accept.contains(&s) {
            attrs.push("shape=doublecircle".to_string());
        } else if diagram.reject.contains(&s) {
            attrs.push("shape=box".to_string());
        }
        if let Some(h) = highlight {
            if h.last.contains(&s) {
                attrs.push("style=\"bold,filled\", color=\"#1f6feb\", fillcolor=\"#cfe3ff\"".to_string());
            } else if h.visited.contains(&s) {
                attrs.push("style=bold, color=\"#1f6feb\"".to_string());
            }
        }
        if attrs.is_empty() {
            out.push_str(&format!("    {};\n", quote(name)));
        } else {
            out.push_str(&format!("    {} [{}];\n", quote(name), attrs.join(", ")));
        }
    }
    out.push_str(&format!("    __start -> {};\n", quote(&diagram.states[diagram.start])));
    for (&(s, t), labels) in &diagram.edges {
        let used = highlight.is_some_and(|h| h.edges.contains(&(s, t)));
        let style = if used { ", style=bold, color=\"#1f6feb\", fontcolor=\"#1f6feb\"" } else { "" };
        out.push_str(&format!(
            "    {} -> {} [label=\"{}\"{style}];\n",
            quote(&diagram.states[s]),
            quote(&diagram.states[t]),
            labels.join(diagram.separator).replace('"', "\\\"")
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
    fn highlights_a_run() {
        let diagram = dfa_diagram(&even_zeros());
        let h = Highlight { visited: BTreeSet::from([0, 1]), edges: BTreeSet::from([(0, 1)]), last: BTreeSet::from([1]) };
        let dot = render(&diagram, Some(&h));
        assert!(dot.contains("\"q0\" [shape=doublecircle, style=bold, color=\"#1f6feb\"];"), "{dot}");
        assert!(dot.contains("\"q1\" [style=\"bold,filled\""), "{dot}");
        assert!(dot.contains("\"q0\" -> \"q1\" [label=\"0\", style=bold"), "{dot}");
        assert!(dot.contains("\"q1\" -> \"q1\" [label=\"1\"];"), "{dot}");
        assert_eq!(render(&diagram, None), dfa_to_dot(&even_zeros()));
    }

    #[test]
    fn merges_parallel_edges() {
        let dot = nfa_to_dot(&ends_in_01());
        assert!(dot.contains("\"a\" -> \"a\" [label=\"0, 1\"];"), "{dot}");
    }
}
