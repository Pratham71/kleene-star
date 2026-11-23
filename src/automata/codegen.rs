//! Code generation: a DFA written out as a standalone Rust function.
//!
//! This is the one place where compiling makes sense. A Kleene Star program
//! is a script that builds machines; but a finished DFA is a fixed table,
//! and turning that table into code gives a matcher that runs anywhere,
//! with no Kleene Star and no dependencies:
//!
//! ```text
//! pub fn even_zeros(input: &str) -> bool {
//!     let mut state: usize = 0;
//!     for symbol in input.chars() {
//!         state = match (state, symbol) {
//!             (0, '0') => 1, // even --0--> odd
//!             ...
//!             _ => return false, // not in the alphabet
//!         };
//!     }
//!     matches!(state, 0)
//! }
//! ```
//!
//! States become numbers, in the order they are reached from the start
//! state, with their original names kept as comments. Transitions from one
//! state to the same target are joined into one arm (`'0' | '1'`). A symbol
//! outside the alphabet matches no arm, so the word is rejected. Words the
//! machine accepts and rejects are found by trying the shortest words, and
//! become a `#[cfg(test)]` module, so `cargo test` checks the function.

use super::{fmt_alphabet, Dfa, State};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Rust's keywords, which cannot be function names.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self",
    "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "abstract",
    "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
];

/// Why `name` cannot be the name of a Rust function, if it cannot.
pub fn check_fn_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !first_ok || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!("`{name}` is not a Rust function name: use letters, digits and `_`, not starting with a digit"));
    }
    if name == "_" || RUST_KEYWORDS.contains(&name) {
        return Err(format!("`{name}` is a Rust keyword, so it cannot name a function"));
    }
    Ok(())
}

/// `EvenZeros` as `even_zeros`: the usual Rust name for a function made
/// from a machine's name.
pub fn snake_case(name: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let after_lower = i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let before_lower = i > 0 && chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase()) && chars[i - 1].is_ascii_uppercase();
            if (after_lower || before_lower) && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "m_");
    }
    if RUST_KEYWORDS.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

/// Up to `count` words the DFA accepts and up to `count` it rejects, the
/// shortest first (then in alphabet order), trying words up to `max_len`
/// symbols long. Rejected words include one with a symbol from outside the
/// alphabet, when one can be found.
pub fn sample_words(dfa: &Dfa, count: usize, max_len: usize) -> (Vec<String>, Vec<String>) {
    let (mut accepted, mut rejected) = (Vec::new(), Vec::new());
    // Breadth-first over words, carrying the state each one ends in.
    let mut layer: Vec<(String, State)> = vec![(String::new(), dfa.start)];
    for _ in 0..=max_len {
        let mut next = Vec::new();
        for (word, state) in &layer {
            let list = if dfa.accept.contains(state) { &mut accepted } else { &mut rejected };
            if list.len() < count {
                list.push(word.clone());
            }
            if accepted.len() >= count && rejected.len() >= count {
                break;
            }
            for (i, &c) in dfa.alphabet.iter().enumerate() {
                next.push((format!("{word}{c}"), dfa.delta[*state][i]));
            }
        }
        if (accepted.len() >= count && rejected.len() >= count) || next.len() > 100_000 {
            break;
        }
        layer = next;
    }
    // A symbol that is not in the alphabet: rejected whatever the machine.
    if let Some(foreign) = ['x', '2', '#', 'a', '0'].into_iter().find(|c| !dfa.alphabet.contains(c)) {
        let word = format!("{}{foreign}", accepted.first().cloned().unwrap_or_default());
        rejected.push(word);
    }
    (accepted, rejected)
}

/// The Rust source of `pub fn fn_name(input: &str) -> bool`, accepting
/// exactly the words `dfa` accepts, with a test module. `title` describes
/// the machine in the comments (for example "DFA `EvenZeros`"). The caller
/// checks `fn_name` with `check_fn_name` first.
pub fn dfa_to_rust(dfa: &Dfa, fn_name: &str, title: &str) -> String {
    // Number the states in the order they are reached, leaving out states
    // that can never be reached: they cannot change the answer.
    let order = dfa.reachable();
    let mut number = vec![usize::MAX; dfa.states.len()];
    for (new, &old) in order.iter().enumerate() {
        number[old] = new;
    }
    let alphabet = fmt_alphabet(&dfa.alphabet);
    let mut out = String::new();
    let w = &mut out;
    let _ = writeln!(w, "// Generated by Kleene Star from {title}.");
    let _ = writeln!(w, "// It has no dependencies; edit the Kleene Star program, not this file.");
    let _ = writeln!(w);
    let _ = writeln!(w, "/// True if `input` is in the language of {title}, over the alphabet");
    let _ = writeln!(w, "/// {alphabet}. A word with any other symbol is rejected.");
    let _ = writeln!(w, "///");
    let _ = writeln!(w, "/// The states, numbered from the start state:");
    for (new, &old) in order.iter().enumerate() {
        let mut roles = Vec::new();
        if old == dfa.start {
            roles.push("start");
        }
        if dfa.accept.contains(&old) {
            roles.push("accepting");
        }
        let roles = if roles.is_empty() { String::new() } else { format!(" ({})", roles.join(", ")) };
        let _ = writeln!(w, "/// - {new}: `{}`{roles}", comment_safe(&dfa.states[old]));
    }
    let _ = writeln!(w, "pub fn {fn_name}(input: &str) -> bool {{");
    let _ = writeln!(w, "    let mut state: usize = 0;");
    let _ = writeln!(w, "    for symbol in input.chars() {{");
    let _ = writeln!(w, "        state = match (state, symbol) {{");
    for &old in &order {
        // The symbols that lead to each target, in alphabet order.
        let mut by_target: BTreeMap<usize, Vec<char>> = BTreeMap::new();
        for (i, &c) in dfa.alphabet.iter().enumerate() {
            by_target.entry(number[dfa.delta[old][i]]).or_default().push(c);
        }
        for (target, symbols) in by_target {
            let pattern: Vec<String> = symbols.iter().map(|c| format!("{c:?}")).collect();
            let shown: Vec<String> = symbols.iter().map(char::to_string).collect();
            let _ = writeln!(
                w,
                "            ({}, {}) => {target}, // {} --{}--> {}",
                number[old],
                pattern.join(" | "),
                comment_safe(&dfa.states[old]),
                comment_safe(&shown.join(",")),
                comment_safe(&dfa.states[order[target]]),
            );
        }
    }
    let _ = writeln!(w, "            _ => return false, // a symbol outside {alphabet}");
    let _ = writeln!(w, "        }};");
    let _ = writeln!(w, "    }}");
    let accepting: Vec<String> =
        order.iter().filter(|old| dfa.accept.contains(old)).map(|&old| number[old].to_string()).collect();
    if accepting.is_empty() {
        let _ = writeln!(w, "    let _ = state;");
        let _ = writeln!(w, "    false // no state accepts: the language is empty");
    } else {
        let _ = writeln!(w, "    matches!(state, {})", accepting.join(" | "));
    }
    let _ = writeln!(w, "}}");

    let (accepted, rejected) = sample_words(dfa, 3, 12);
    let _ = writeln!(w);
    let _ = writeln!(w, "#[cfg(test)]");
    let _ = writeln!(w, "mod {fn_name}_tests {{");
    let _ = writeln!(w, "    use super::{fn_name};");
    let _ = writeln!(w);
    let _ = writeln!(w, "    #[test]");
    let _ = writeln!(w, "    fn accepts_and_rejects_sample_words() {{");
    for word in &accepted {
        let _ = writeln!(w, "        assert!({fn_name}({word:?}));");
    }
    for word in &rejected {
        let _ = writeln!(w, "        assert!(!{fn_name}({word:?}));");
    }
    let _ = writeln!(w, "    }}");
    let _ = writeln!(w, "}}");
    out
}

/// Python's keywords and soft keywords, which cannot (or should not) name
/// a function.
const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
    "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal",
    "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "match", "case", "type",
];

/// `name` made usable as a Python function name: a Python keyword gets a
/// `_` at the end. `name` is already a valid Rust name (`check_fn_name`).
pub fn python_name(name: &str) -> String {
    if PYTHON_KEYWORDS.contains(&name) { format!("{name}_") } else { name.to_string() }
}

/// The Python source of `def fn_name(word: str) -> bool`, accepting exactly
/// the words `dfa` accepts: one file with no dependencies, which checks
/// itself on sample words when run as a script. `title` describes the
/// machine in the comments. The caller checks `fn_name` with
/// `check_fn_name` and passes it through `python_name` first.
pub fn dfa_to_python(dfa: &Dfa, fn_name: &str, title: &str) -> String {
    let order = dfa.reachable();
    let mut number = vec![usize::MAX; dfa.states.len()];
    for (new, &old) in order.iter().enumerate() {
        number[old] = new;
    }
    let alphabet = fmt_alphabet(&dfa.alphabet);
    let table = format!("_{}_MOVES", fn_name.trim_matches('_').to_ascii_uppercase());
    let accepting_name = format!("_{}_ACCEPTING", fn_name.trim_matches('_').to_ascii_uppercase());
    let mut out = String::new();
    let w = &mut out;
    let _ = writeln!(w, "# Generated by Kleene Star from {}.", comment_safe(title));
    let _ = writeln!(w, "# It has no dependencies; edit the Kleene Star program, not this file.");
    let _ = writeln!(w, "#");
    let _ = writeln!(w, "# The states, numbered from the start state:");
    for (new, &old) in order.iter().enumerate() {
        let mut roles = Vec::new();
        if old == dfa.start {
            roles.push("start");
        }
        if dfa.accept.contains(&old) {
            roles.push("accepting");
        }
        let roles = if roles.is_empty() { String::new() } else { format!(" ({})", roles.join(", ")) };
        let _ = writeln!(w, "# - {new}: {}{roles}", comment_safe(&dfa.states[old]));
    }
    let _ = writeln!(w);
    let _ = writeln!(w, "# (state, symbol) -> next state; a pair that is missing rejects the word.");
    let _ = writeln!(w, "{table} = {{");
    for &old in &order {
        for (i, &c) in dfa.alphabet.iter().enumerate() {
            let target = dfa.delta[old][i];
            let _ = writeln!(
                w,
                "    ({}, {}): {},  # {} --{}--> {}",
                number[old],
                python_str(&c.to_string()),
                number[target],
                comment_safe(&dfa.states[old]),
                comment_safe(&c.to_string()),
                comment_safe(&dfa.states[target]),
            );
        }
    }
    let _ = writeln!(w, "}}");
    let accepting: Vec<String> =
        order.iter().filter(|old| dfa.accept.contains(old)).map(|&old| number[old].to_string()).collect();
    if accepting.is_empty() {
        let _ = writeln!(w, "{accepting_name} = frozenset()  # no state accepts: the language is empty");
    } else {
        let _ = writeln!(w, "{accepting_name} = frozenset({{{}}})", accepting.join(", "));
    }
    let _ = writeln!(w);
    let _ = writeln!(w);
    let _ = writeln!(w, "def {fn_name}(word: str) -> bool:");
    let _ = writeln!(w, "    \"\"\"True if `word` is in the language of {}, over the", comment_safe(title).replace('"', "'"));
    let _ = writeln!(w, "    alphabet {alphabet}. A word with any other symbol is rejected.\"\"\"");
    let _ = writeln!(w, "    state = 0");
    let _ = writeln!(w, "    for symbol in word:");
    let _ = writeln!(w, "        state = {table}.get((state, symbol))");
    let _ = writeln!(w, "        if state is None:");
    let _ = writeln!(w, "            return False");
    let _ = writeln!(w, "    return state in {accepting_name}");

    let (accepted, rejected) = sample_words(dfa, 3, 12);
    let list = |words: &[String]| words.iter().map(|word| python_str(word)).collect::<Vec<_>>().join(", ");
    let _ = writeln!(w);
    let _ = writeln!(w);
    let _ = writeln!(w, "if __name__ == \"__main__\":");
    let _ = writeln!(w, "    # Checks the function on the shortest words it accepts and rejects.");
    let _ = writeln!(w, "    for sample in [{}]:", list(&accepted));
    let _ = writeln!(w, "        assert {fn_name}(sample), f\"should accept {{sample!r}}\"");
    let _ = writeln!(w, "    for sample in [{}]:", list(&rejected));
    let _ = writeln!(w, "        assert not {fn_name}(sample), f\"should reject {{sample!r}}\"");
    let _ = writeln!(w, "    print(\"{fn_name}: {} sample words answered correctly\")", accepted.len() + rejected.len());
    out
}

/// `text` as a Python string literal.
fn python_str(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if (c as u32) < 0x20 || c == '\u{7f}' => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Text that is safe inside a `//` comment: one line.
fn comment_safe(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Words over {0, 1} with an even number of 0s.
    fn even_zeros() -> Dfa {
        Dfa {
            alphabet: vec!['0', '1'],
            states: vec!["even".into(), "odd".into(), "unused".into()],
            start: 0,
            accept: BTreeSet::from([0]),
            delta: vec![vec![1, 0], vec![0, 1], vec![2, 2]],
        }
    }

    #[test]
    fn writes_a_match_table_with_state_names() {
        let code = dfa_to_rust(&even_zeros(), "even_zeros", "DFA `EvenZeros`");
        assert!(code.contains("pub fn even_zeros(input: &str) -> bool {"));
        assert!(code.contains("            (0, '0') => 1, // even --0--> odd\n"));
        assert!(code.contains("            (1, '1') => 1, // odd --1--> odd\n"));
        assert!(code.contains("            _ => return false, // a symbol outside {0, 1}\n"));
        assert!(code.contains("    matches!(state, 0)\n"));
        assert!(!code.contains("unused"), "unreachable states are left out");
        assert!(code.contains("assert!(even_zeros(\"\"));"));
        assert!(code.contains("assert!(!even_zeros(\"0\"));"));
    }

    #[test]
    fn joins_symbols_with_the_same_target() {
        let all = Dfa { alphabet: vec!['a', 'b'], states: vec!["s".into()], start: 0, accept: BTreeSet::from([0]), delta: vec![vec![0, 0]] };
        assert!(dfa_to_rust(&all, "f", "x").contains("(0, 'a' | 'b') => 0, // s --a,b--> s"));
        let none = Dfa { accept: BTreeSet::new(), ..all };
        assert!(dfa_to_rust(&none, "f", "x").contains("false // no state accepts"));
    }

    #[test]
    fn writes_a_python_table_and_a_self_test() {
        let code = dfa_to_python(&even_zeros(), "even_zeros", "DFA `EvenZeros`");
        assert!(code.contains("def even_zeros(word: str) -> bool:\n"));
        assert!(code.contains("    (0, \"0\"): 1,  # even --0--> odd\n"));
        assert!(code.contains("_EVEN_ZEROS_ACCEPTING = frozenset({0})\n"));
        assert!(code.contains("if __name__ == \"__main__\":\n"));
        assert!(code.contains("for sample in [\"\", \"1\", \"00\"]:"));
        assert!(!code.contains("unused"), "unreachable states are left out");
        let none = Dfa { accept: BTreeSet::new(), ..even_zeros() };
        assert!(dfa_to_python(&none, "f", "x").contains("frozenset()  # no state accepts"));
        assert_eq!(python_str("a\"b\\c\n"), "\"a\\\"b\\\\c\\x0a\"");
        assert_eq!(python_name("lambda"), "lambda_");
        assert_eq!(python_name("even_zeros"), "even_zeros");
    }

    #[test]
    fn samples_are_shortest_first() {
        let (accepted, rejected) = sample_words(&even_zeros(), 3, 8);
        assert_eq!(accepted, ["", "1", "00"]);
        assert_eq!(rejected, ["0", "01", "10", "x"]);
    }

    #[test]
    fn names() {
        assert_eq!(snake_case("EvenZeros"), "even_zeros");
        assert_eq!(snake_case("DFAMachine"), "dfa_machine");
        assert_eq!(snake_case("div3"), "div3");
        assert_eq!(snake_case("3way"), "m_3way");
        assert_eq!(snake_case("Match"), "match_");
        assert!(check_fn_name("even_zeros").is_ok());
        assert!(check_fn_name("2x").is_err());
        assert!(check_fn_name("fn").is_err());
        assert!(check_fn_name("a-b").is_err());
    }
}
