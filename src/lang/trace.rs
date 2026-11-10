//! How a `trace` statement shows a run.
//!
//! The steps themselves come from `automata::trace`; this file only turns
//! them into the table a person reads:
//!
//! ```text
//! Input: 10110
//!
//! Step  Symbol  From  To
//! 0     —       —     q0
//! 1     1       q0    q1
//! ...
//! Result: REJECTED
//! Reason: Ended in non-accepting state q2
//! ```

use crate::automata::trace::{End, Trace};
use crate::automata::{fmt_alphabet, Dfa, Nfa, State};
use std::collections::BTreeSet;

/// Shows a DFA run. `dash` fills the cells of step 0 that have no value.
pub fn render_dfa(dfa: &Dfa, word: &str, trace: &Trace<State>, dash: &str) -> String {
    let name = |s: &State| dfa.states[*s].clone();
    let last = *trace.last();
    let reason = match trace.end {
        End::Accepted => format!("Ended in accepting state {}", name(&last)),
        End::Rejected => format!("Ended in non-accepting state {}", name(&last)),
        End::Foreign { .. } => String::new(), // filled in by `render`
    };
    render(word, trace, name, &dfa.alphabet, reason, dash)
}

/// Shows an NFA run. Each cell is the set of states the NFA could be in.
pub fn render_nfa(nfa: &Nfa, word: &str, trace: &Trace<BTreeSet<State>>, dash: &str) -> String {
    let name = |set: &BTreeSet<State>| {
        let names: Vec<&str> = set.iter().map(|&s| nfa.states[s].as_str()).collect();
        format!("{{{}}}", names.join(","))
    };
    let last = trace.last();
    let accepting: Vec<&str> =
        last.iter().filter(|s| nfa.accept.contains(s)).map(|&s| nfa.states[s].as_str()).collect();
    let reason = match trace.end {
        End::Accepted => format!(
            "Ended in {}, which includes accepting state {}",
            name(last),
            accepting.join(", ")
        ),
        End::Rejected if last.is_empty() => {
            "No state is possible any more, so no accepting state was reached".to_string()
        }
        End::Rejected => format!("Ended in {}, which includes no accepting state", name(last)),
        End::Foreign { .. } => String::new(),
    };
    render(word, trace, name, &nfa.alphabet, reason, dash)
}

/// The shared layout: the input, a table of steps, the result and the reason.
fn render<S: Clone>(
    word: &str,
    trace: &Trace<S>,
    name: impl Fn(&S) -> String,
    alphabet: &[char],
    reason: String,
    dash: &str,
) -> String {
    let mut rows: Vec<[String; 4]> = vec![
        ["Step".into(), "Symbol".into(), "From".into(), "To".into()],
        ["0".into(), dash.into(), dash.into(), name(&trace.start)],
    ];
    for (i, step) in trace.steps.iter().enumerate() {
        rows.push([(i + 1).to_string(), step.symbol.to_string(), name(&step.from), name(&step.to)]);
    }

    let (result, reason) = match trace.end {
        End::Accepted => ("ACCEPTED", reason),
        End::Rejected => ("REJECTED", reason),
        End::Foreign { position, symbol } => (
            "REJECTED",
            format!(
                "Symbol `{symbol}` (position {position}) is not in the alphabet {}, so the run stops",
                fmt_alphabet(alphabet)
            ),
        ),
    };

    let shown_word = if word.is_empty() { "(the empty string)" } else { word };
    let mut out = format!("Input: {shown_word}\n\n");
    out.push_str(&table(&rows));
    out.push_str(&format!("Result: {result}\nReason: {reason}"));
    out
}

/// Lines up the cells in columns two spaces apart. Each column is as wide as
/// its widest cell, so long state names simply make their column wider.
fn table(rows: &[[String; 4]]) -> String {
    let mut widths = [0; 4];
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for row in rows {
        let mut line = String::new();
        for (cell, width) in row.iter().zip(widths) {
            line.push_str(cell);
            line.push_str(&" ".repeat(width - cell.chars().count() + 2));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::trace::{trace_dfa, trace_nfa};

    fn even_zeros() -> Dfa {
        Dfa {
            alphabet: vec!['0', '1'],
            states: vec!["even".into(), "odd".into()],
            start: 0,
            accept: BTreeSet::from([0]),
            delta: vec![vec![1, 0], vec![0, 1]],
        }
    }

    #[test]
    fn dfa_layout() {
        let dfa = even_zeros();
        let shown = render_dfa(&dfa, "01", &trace_dfa(&dfa, "01"), "-");
        assert_eq!(
            shown,
            "Input: 01\n\n\
             Step  Symbol  From  To\n\
             0     -       -     even\n\
             1     0       even  odd\n\
             2     1       odd   odd\n\
             Result: REJECTED\n\
             Reason: Ended in non-accepting state odd"
        );
    }

    #[test]
    fn foreign_symbol_and_empty_word() {
        let dfa = even_zeros();
        let shown = render_dfa(&dfa, "2", &trace_dfa(&dfa, "2"), "-");
        assert!(shown.ends_with("Reason: Symbol `2` (position 1) is not in the alphabet {0, 1}, so the run stops"));
        let shown = render_dfa(&dfa, "", &trace_dfa(&dfa, ""), "-");
        assert!(shown.starts_with("Input: (the empty string)"));
        assert!(shown.ends_with("Result: ACCEPTED\nReason: Ended in accepting state even"));
    }

    #[test]
    fn nfa_cells_are_sets() {
        let mut nfa = Nfa::new(vec!['a']);
        let (p, q) = (nfa.add_state("p"), nfa.add_state("q"));
        nfa.add_edge(p, Some('a'), p);
        nfa.add_edge(p, Some('a'), q);
        nfa.accept.insert(q);
        let shown = render_nfa(&nfa, "a", &trace_nfa(&nfa, "a"), "-");
        assert!(shown.contains("1     a       {p}   {p,q}"), "{shown}");
        assert!(shown.ends_with("Reason: Ended in {p,q}, which includes accepting state q"));
    }
}
