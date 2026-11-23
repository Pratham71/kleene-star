//! `std.visual`: drawings and traces as values, to print, test or save
//! with `std.io`.
//!
//! The statements `draw M`, `draw M on "w"` and `trace M on "w"` write
//! files or print; these functions give the same things as strings
//! instead, from the same code: `dot` is the DOT text `draw` writes,
//! `dot_run` the same diagram with a run drawn on it, `trace_text` the
//! table `trace` prints, and `player` the page `draw M on "w"` writes.

use super::{str_arg, Failure, Function, Module, Ty};
use crate::automata::dot;
use crate::lang::codes;
use crate::lang::langs;
use crate::lang::player;
use crate::lang::value::Value;

pub const MODULE: Module = Module {
    name: "visual",
    doc: "Drawings and traces as strings: DOT diagrams, a run drawn on its diagram, the trace table, and the page that plays a run.",
    functions: &[
        Function { name: "dot", params: &[("m", Ty::Acceptor)], ret: Ty::Str, doc: "The Graphviz DOT text that `draw m` writes; a regex or set is drawn as its NFA.", run: dot_text },
        Function { name: "dot_run", params: &[("m", Ty::Acceptor), ("w", Ty::Str)], ret: Ty::Str, doc: "The DOT text of machine `m` with its run on `w` drawn on it: the states and moves used in bold, the states it ends in filled.", run: dot_run },
        Function { name: "player", params: &[("m", Ty::Acceptor), ("w", Ty::Str)], ret: Ty::Str, doc: "The self-contained HTML page that `draw m on w` writes, which plays the run step by step.", run: player_page },
        Function { name: "trace_text", params: &[("m", Ty::Acceptor), ("w", Ty::Str)], ret: Ty::Str, doc: "The table `trace m on w` prints, as a string, with `-` in the empty cells.", run: trace_text },
    ],
    explain: None,
};

/// A value that has no states, for a function that needs a machine.
fn no_states(value: &Value) -> Failure {
    let (what, hint) = match value {
        Value::Grammar(_) => ("a grammar", "use its PDA, `to_pda(g)`"),
        _ => ("a regex or set", "convert it first, for example with `to_dfa(r)`"),
    };
    Failure {
        code: codes::NO_STATES,
        arg: Some(0),
        message: format!("this is {what}, which has no states to show a run on"),
        hint: hint.into(),
    }
}

fn dot_text(args: &[Value]) -> Result<Value, Failure> {
    let text = match &args[0] {
        Value::Dfa(d) => dot::dfa_to_dot(d),
        Value::Nfa(n) => dot::nfa_to_dot(n),
        Value::Pda(p) => dot::pda_to_dot(p),
        Value::Tm(m) => dot::tm_to_dot(m),
        Value::Grammar(_) => {
            return Err(Failure {
                code: codes::NO_STATES,
                arg: Some(0),
                message: "a grammar has no states to draw".into(),
                hint: "draw its PDA: `std.visual.dot(to_pda(g))`".into(),
            })
        }
        other => dot::nfa_to_dot(&langs::to_nfa(other, &langs::symbols_of(other))),
    };
    Ok(Value::str(&text))
}

fn run_of(args: &[Value]) -> Result<player::Play, Failure> {
    player::play(&args[0], str_arg(args, 1)).ok_or_else(|| no_states(&args[0]))
}

fn dot_run(args: &[Value]) -> Result<Value, Failure> {
    let play = run_of(args)?;
    Ok(Value::str(&dot::render(&play.diagram, Some(&player::highlight(&play)))))
}

fn player_page(args: &[Value]) -> Result<Value, Failure> {
    let play = run_of(args)?;
    let states = play.diagram.states.len();
    if states > player::MAX_STATES {
        return Err(Failure {
            code: codes::RESULT_TOO_LARGE,
            arg: Some(0),
            message: format!("this machine has {states} states; the player lays out at most {}", player::MAX_STATES),
            hint: "`minimize` it first, or use `trace_text` to see the run as a table".into(),
        });
    }
    Ok(Value::str(&player::html(&play, "machine")))
}

fn trace_text(args: &[Value]) -> Result<Value, Failure> {
    use crate::automata::{pda, trace};
    use crate::lang::trace as show;
    let word = str_arg(args, 1);
    let text = match &args[0] {
        Value::Dfa(d) => show::render_dfa(d, word, &trace::trace_dfa(d, word), "-"),
        Value::Nfa(n) => show::render_nfa(n, word, &trace::trace_nfa(n, word), "-"),
        Value::Pda(p) => show::render_pda(p, word, &p.search(word, pda::STEP_LIMIT), "-"),
        Value::Tm(m) => show::render_tm(m, word, &m.run(word, show::TM_TRACE_LIMIT, true), "-"),
        other => return Err(no_states(other)),
    };
    Ok(Value::str(&text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::tests::even_zeros;

    fn call(name: &str, args: &[Value]) -> Result<Value, Failure> {
        (super::super::function("visual", name).unwrap().run)(args)
    }

    fn text(v: Result<Value, Failure>) -> String {
        match v {
            Ok(Value::Str(s)) => s.to_string(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_same_things_as_the_statements() {
        let m = Value::dfa(even_zeros());
        assert_eq!(text(call("dot", std::slice::from_ref(&m))), dot::dfa_to_dot(&even_zeros()));
        let run = text(call("dot_run", &[m.clone(), Value::str("0")]));
        assert!(run.contains("\"q1\" [style=\"bold,filled\""), "{run}");
        let table = text(call("trace_text", &[m.clone(), Value::str("01")]));
        assert!(table.starts_with("Input: 01") && table.contains("Result: REJECTED"), "{table}");
        let page = text(call("player", &[m, Value::str("01")]));
        assert!(page.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn values_without_states() {
        let r = Value::Regex(std::rc::Rc::new(crate::automata::regex::parse("ab*").unwrap()));
        assert!(text(call("dot", std::slice::from_ref(&r))).starts_with("digraph"));
        for name in ["dot_run", "trace_text", "player"] {
            assert_eq!(call(name, &[r.clone(), Value::str("a")]).unwrap_err().code, codes::NO_STATES);
        }
    }
}
