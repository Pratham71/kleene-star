//! The automata library.
//!
//! Everything in here is plain Rust with no knowledge of the Kleene Star language:
//! machines go in, machines come out. The interpreter in `crate::lang` is
//! just one user of it.

pub mod convert;
pub mod dfa;
pub mod dot;
pub mod grammar;
pub mod minimize;
pub mod nfa;
pub mod ops;
pub mod pda;
pub mod tm;
pub mod regex;
pub mod trace;

pub use dfa::Dfa;
pub use nfa::Nfa;
pub use regex::Regex;

/// A state is an index into a machine's `states` list.
pub type State = usize;

/// Formats an alphabet the way it is written in source: `{0, 1}`.
pub fn fmt_alphabet(alphabet: &[char]) -> String {
    let parts: Vec<String> = alphabet.iter().map(|c| c.to_string()).collect();
    format!("{{{}}}", parts.join(", "))
}

/// Lays out rows of cells as an aligned text table. The first row is the header.
pub fn render_table(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut widths = vec![0; columns];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for row in rows {
        let mut line = String::new();
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                line.push_str(" | ");
            }
            line.push_str(cell);
            for _ in cell.chars().count()..widths[i] {
                line.push(' ');
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.pop(); // no trailing newline
    out
}

/// The label shown in the first column of a transition table:
/// `->` marks the start state and `*` marks an accepting state.
pub(crate) fn row_label(name: &str, is_start: bool, is_accept: bool) -> String {
    format!(
        "{} {}{}",
        if is_start { "->" } else { "  " },
        if is_accept { "*" } else { " " },
        name
    )
}

/// Returns `base`, or `base` with primes added, so it is not in `taken`.
pub(crate) fn fresh_name(taken: &[String], base: &str) -> String {
    let mut name = base.to_string();
    while taken.contains(&name) {
        name.push('\'');
    }
    name
}
