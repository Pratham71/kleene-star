//! `std.steps`: the working of the conversions, step by step, as the usual
//! textbook working writes it out. Each function gives a string to print;
//! the text comes from `automata::steps`, which runs the same code as the
//! conversion it explains, so the working always ends where the
//! conversion does.

use super::{Failure, Function, Module, Ty};
use crate::automata::{arden, convert, minimize, steps, Nfa};
use crate::lang::codes;
use crate::lang::langs;
use crate::lang::value::Value;

/// The largest regex `elimination` shows the working for, in parts.
pub const MAX_REGEX_SIZE: usize = 5_000;

pub const MODULE: Module = Module {
    name: "steps",
    doc: "The working of the conversions, step by step: eps-closures, the subset construction table, the k-equivalence partitions, state elimination and Arden's theorem.",
    functions: &[
        Function { name: "arden", params: &[("m", Ty::Lang)], ret: Ty::Str, doc: "From a finite automaton to a regex by Arden's theorem: an equation for each state, then each solved with R = Q + RP giving R = QP* and put into the others, then the regex.", run: arden_working },
        Function { name: "elimination", params: &[("m", Ty::Lang)], ret: Ty::Str, doc: "State elimination, as `to_regex` does it: the new start and final states, then each state removed and the edges that replace the paths through it, then the regex.", run: elimination },
        Function { name: "eps_closures", params: &[("m", Ty::Lang)], ret: Ty::Str, doc: "A table of each state of `m`'s NFA and its eps-closure, the states it reaches by eps moves alone.", run: eps_closures },
        Function { name: "partitions", params: &[("m", Ty::Lang)], ret: Ty::Str, doc: "Minimisation of `m`'s DFA by k-equivalence: the partitions P0, P1, ... until one is the same as the one before.", run: partitions },
        Function { name: "subset_table", params: &[("m", Ty::Lang)], ret: Ty::Str, doc: "The subset construction for `m`'s NFA as a table: each set of states found, in order, and where each symbol leads, with the empty set written Ø.", run: subset_table },
    ],
    explain: None,
};

fn nfa_of(m: &Value) -> crate::automata::Nfa {
    langs::to_nfa(m, &langs::symbols_of(m))
}

fn dfa_of(m: &Value) -> crate::automata::Dfa {
    langs::to_dfa(m, &langs::symbols_of(m))
}

/// The machine Arden's theorem works on: a `Dfa` or `Nfa` as it is, after
/// removing eps moves if it has any; a regex or set as its minimal DFA,
/// with states renamed `q0`, `q1`, ... outward from the start.
pub fn arden_machine(m: &Value) -> Nfa {
    match m {
        Value::Dfa(d) => convert::dfa_to_nfa(d),
        Value::Nfa(n) if n.has_eps() => convert::remove_eps(n),
        Value::Nfa(n) => (**n).clone(),
        other => convert::dfa_to_nfa(&minimize::minimize(&dfa_of(other)).renamed()),
    }
}

/// Arden's working for `m`, or E0608 past `max_size` parts.
pub fn arden_solve(m: &Value, max_size: usize) -> Result<arden::Working, Failure> {
    arden::solve(&arden_machine(m), max_size).ok_or_else(|| Failure {
        code: codes::TOO_LARGE,
        arg: Some(0),
        message: format!("the equations grow past {max_size} parts as they are solved"),
        hint: "`minimize` the machine first: fewer states give much shorter equations".into(),
    })
}

fn arden_working(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::str(&arden::render(&arden_solve(&args[0], MAX_REGEX_SIZE)?)))
}

fn eps_closures(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::str(&steps::eps_closures(&nfa_of(&args[0]))))
}

fn subset_table(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::str(&steps::subset_table(&nfa_of(&args[0]))))
}

fn partitions(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::str(&steps::partitions(&dfa_of(&args[0]))))
}

fn elimination(args: &[Value]) -> Result<Value, Failure> {
    let dfa = dfa_of(&args[0]);
    if convert::dfa_to_regex_within(&dfa, MAX_REGEX_SIZE).is_none() {
        return Err(Failure {
            code: codes::TOO_LARGE,
            arg: Some(0),
            message: format!("the working would build a regex of over {MAX_REGEX_SIZE} parts, too long to show"),
            hint: "show `partitions` instead, or work on a smaller machine".into(),
        });
    }
    Ok(Value::str(&steps::elimination(&dfa)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::tests::even_zeros;

    fn call(name: &str, args: &[Value]) -> Result<Value, Failure> {
        (super::super::function("steps", name).unwrap().run)(args)
    }

    #[test]
    fn every_function_takes_a_regex_or_a_machine() {
        let r = Value::Regex(std::rc::Rc::new(crate::automata::regex::parse("(0|1)*01").unwrap()));
        for name in ["arden", "elimination", "eps_closures", "partitions", "subset_table"] {
            for m in [r.clone(), Value::dfa(even_zeros())] {
                assert!(matches!(call(name, &[m]), Ok(Value::Str(_))), "{name}");
            }
        }
    }

    #[test]
    fn a_regex_too_large_to_show() {
        // The k-th symbol from the end is 1: state elimination blows up.
        let r = Value::Regex(std::rc::Rc::new(crate::automata::regex::parse("(0|1)*1(0|1)(0|1)(0|1)(0|1)(0|1)(0|1)").unwrap()));
        assert_eq!(call("elimination", &[r]).unwrap_err().code, codes::TOO_LARGE);
    }
}
