//! Regular languages as values: the rules for turning a `Dfa`, `Nfa`, `Regex`
//! or finite `Set[Str]` into a machine, and for choosing the alphabet when
//! several of them meet.
//!
//! The built-ins (`equivalent`, `union`, ...) and the standard library
//! (`std.automata.shortest_word`, ...) both use these functions, so a
//! language means the same thing everywhere:
//!
//! * a `Dfa` or `Nfa` has a fixed alphabet, and all machines in one
//!   operation must agree on it;
//! * a `Regex` or a set of words is flexible: it adopts the machines'
//!   alphabet (or one given explicitly), or failing that, the symbols it
//!   mentions.

use super::codes;
use super::value::Value;
use crate::automata::{convert, fmt_alphabet, Dfa, Nfa};
use std::collections::BTreeSet;

/// Why no alphabet fits all the languages of an operation. `arg` is the
/// index of the language that does not fit.
#[derive(Debug, Clone, PartialEq)]
pub struct AlphabetProblem {
    pub arg: usize,
    pub code: &'static str,
    pub message: String,
    pub hint: String,
}

/// Picks the alphabet an operation on `langs` works over (see the module
/// comment for the rules).
pub fn common_alphabet(langs: &[Value], explicit: Option<&[char]>) -> Result<Vec<char>, AlphabetProblem> {
    let mut fixed: Option<Vec<char>> = explicit.map(<[char]>::to_vec);
    for (arg, lang) in langs.iter().enumerate() {
        let own = match lang {
            Value::Dfa(dfa) => &dfa.alphabet,
            Value::Nfa(nfa) => &nfa.alphabet,
            _ => continue,
        };
        match &fixed {
            Some(other) if other != own => {
                return Err(AlphabetProblem {
                    arg,
                    code: codes::ALPHABET_MISMATCH,
                    message: format!("this machine is over {}, but {} is needed here", fmt_alphabet(own), fmt_alphabet(other)),
                    hint: "languages can only be combined when they share an alphabet; build both machines `over` the same one"
                        .into(),
                });
            }
            _ => fixed = Some(own.clone()),
        }
    }
    let alphabet = fixed.unwrap_or_else(|| {
        let all: BTreeSet<char> = langs.iter().flat_map(symbols_of).collect();
        all.into_iter().collect()
    });
    for (arg, lang) in langs.iter().enumerate() {
        if matches!(lang, Value::Dfa(_) | Value::Nfa(_)) {
            continue;
        }
        if let Some(c) = symbols_of(lang).into_iter().find(|c| !alphabet.contains(c)) {
            return Err(AlphabetProblem {
                arg,
                code: codes::NOT_IN_ALPHABET,
                message: format!("this uses the symbol `{c}`, which is not in the alphabet {}", fmt_alphabet(&alphabet)),
                hint: "every symbol must belong to the alphabet being used".into(),
            });
        }
    }
    Ok(alphabet)
}

/// The symbols a language value mentions, sorted: a machine's alphabet, or
/// the symbols written in a regex or a set of words.
pub fn symbols_of(lang: &Value) -> Vec<char> {
    let mut symbols = BTreeSet::new();
    match lang {
        Value::Regex(regex) => regex.symbols(&mut symbols),
        Value::Set(words) => {
            for word in words.iter() {
                if let Value::Str(word) = word {
                    symbols.extend(word.chars());
                }
            }
        }
        Value::Dfa(dfa) => symbols.extend(&dfa.alphabet),
        Value::Nfa(nfa) => symbols.extend(&nfa.alphabet),
        _ => {}
    }
    symbols.into_iter().collect()
}

/// Any language value as an NFA over `alphabet`.
pub fn to_nfa(lang: &Value, alphabet: &[char]) -> Nfa {
    match lang {
        Value::Nfa(nfa) => (**nfa).clone(),
        Value::Dfa(dfa) => convert::dfa_to_nfa(dfa),
        Value::Regex(regex) => convert::regex_to_nfa(regex, alphabet),
        Value::Set(words) => {
            let words = words.iter().filter_map(|w| match w {
                Value::Str(word) => Some(&**word),
                _ => None,
            });
            convert::regex_to_nfa(&convert::finite_to_regex(words), alphabet)
        }
        other => unreachable!("checker allowed {other:?} as a language"),
    }
}

/// Any language value as a DFA over `alphabet`.
pub fn to_dfa(lang: &Value, alphabet: &[char]) -> Dfa {
    match lang {
        Value::Dfa(dfa) => (**dfa).clone(),
        other => convert::nfa_to_dfa(&to_nfa(other, alphabet)),
    }
}

/// Why `accepts` could not answer yes or no.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// The word has a symbol outside the machine's (or grammar's) alphabet.
    Foreign { symbol: char, alphabet: Vec<char> },
    /// The PDA's search looked at `pda::STEP_LIMIT` configurations without
    /// an answer: the answer is unknown, not "no".
    PdaGaveUp,
    /// The Turing machine was still running after `tm::STEP_LIMIT` steps.
    TmStillRunning,
}

/// Whether `lang` (anything the `accepts` operator takes) contains `word`.
///
/// A machine or grammar refuses a word with a symbol outside its alphabet.
/// A regex or a set of words has no fixed alphabet, so such a word is simply
/// not in its language.
pub fn accepts(lang: &Value, word: &str) -> Result<bool, Refusal> {
    use crate::automata::pda::{self, Outcome};
    use crate::automata::tm::{self, Halt};
    let foreign = |symbol: char, alphabet: &[char]| Refusal::Foreign { symbol, alphabet: alphabet.to_vec() };
    match lang {
        Value::Dfa(dfa) => dfa.accepts(word).map_err(|c| foreign(c, &dfa.alphabet)),
        Value::Nfa(nfa) => nfa.accepts(word).map_err(|c| foreign(c, &nfa.alphabet)),
        Value::Grammar(g) => g.accepts(word).map_err(|c| foreign(c, &g.alphabet)),
        Value::Set(words) => Ok(words.contains(&Value::str(word))),
        Value::Pda(p) => match p.search(word, pda::STEP_LIMIT).outcome {
            Outcome::Accepted => Ok(true),
            Outcome::Rejected => Ok(false),
            Outcome::Foreign { symbol, .. } => Err(foreign(symbol, &p.alphabet)),
            Outcome::GaveUp => Err(Refusal::PdaGaveUp),
        },
        Value::Tm(m) => match m.run(word, tm::STEP_LIMIT, false).halt {
            Halt::Foreign { symbol, .. } => Err(foreign(symbol, &m.alphabet)),
            Halt::GaveUp => Err(Refusal::TmStillRunning),
            halt => Ok(halt == Halt::Accepted),
        },
        regex => {
            // Over the regex's own symbols plus the word's, so a foreign
            // symbol just fails to match.
            let mut alphabet = symbols_of(regex);
            alphabet.extend(word.chars());
            alphabet.sort();
            alphabet.dedup();
            Ok(to_nfa(regex, &alphabet).accepts(word) == Ok(true))
        }
    }
}

/// The languages `langs` as DFAs over one common alphabet.
pub fn to_dfas(langs: &[Value]) -> Result<Vec<Dfa>, AlphabetProblem> {
    let alphabet = common_alphabet(langs, None)?;
    Ok(langs.iter().map(|lang| to_dfa(lang, &alphabet)).collect())
}
