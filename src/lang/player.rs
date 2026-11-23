//! The run player: `draw M on "w"` writes one HTML page that plays the run
//! of machine `M` on word `w` step by step, with play, pause, step and a
//! slider. The page is self-contained (no network, no Graphviz): the
//! machine, its layout and every step of the run are embedded in it as
//! JSON, and a small script draws them as SVG.
//!
//! This file turns a run into `Frame`s, one per step, from the same runs
//! `trace` prints (`automata::trace`, `Pda::search`, `Tm::run`), lays the
//! states out in columns by their distance from the start state, and writes
//! the page. `std.visual` uses the same frames for `dot_run`, which draws
//! the run on the DOT diagram instead.

use super::trace as show_trace;
use super::value::Value;
use crate::automata::dot::{self, Diagram, Highlight};
use crate::automata::{pda, tm, trace, State};
use std::collections::{BTreeSet, VecDeque};

/// The most states the player lays out. Past this a diagram is unreadable
/// anyway, and the page would be very large.
pub const MAX_STATES: usize = 300;

/// The most steps of a Turing machine the player records (`trace` shows
/// 100; a page can hold more).
pub const TM_STEPS: usize = 1000;

/// One step of a run, as the player shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// The states the machine is in: one, or a set for an NFA.
    pub active: Vec<State>,
    /// The edges the move into this frame used.
    pub taken: Vec<(State, State)>,
    /// How many input symbols have been read (not for a Turing machine,
    /// whose input is on its tape).
    pub read: Option<usize>,
    /// The move into this frame: a symbol, or a PDA or TM label.
    pub label: String,
    /// A PDA's stack, top first.
    pub stack: Option<Vec<String>>,
    /// A Turing machine's tape (the used part and the head's cell) and the
    /// head's index in it.
    pub tape: Option<(Vec<char>, usize)>,
}

/// A run, ready to be shown.
#[derive(Debug, Clone, PartialEq)]
pub struct Play {
    /// `DFA`, `NFA`, `PDA` or `TM`.
    pub kind: &'static str,
    pub diagram: Diagram,
    pub word: String,
    pub frames: Vec<Frame>,
    /// `ACCEPTED`, `REJECTED` or `UNKNOWN`, as `trace` says it.
    pub result: String,
    /// Why, in the words `trace` uses.
    pub reason: String,
}

/// The run of `machine` (a Dfa, Nfa, Pda or Tm) on `word`. `None` for any
/// other value, which has no states to show.
pub fn play(machine: &Value, word: &str) -> Option<Play> {
    let (kind, diagram, frames, shown) = match machine {
        Value::Dfa(dfa) => {
            let run = trace::trace_dfa(dfa, word);
            let mut frames = vec![frame(vec![run.start], vec![], Some(0), String::new())];
            for (i, step) in run.steps.iter().enumerate() {
                frames.push(frame(vec![step.to], vec![(step.from, step.to)], Some(i + 1), step.symbol.to_string()));
            }
            ("DFA", dot::dfa_diagram(dfa), frames, show_trace::render_dfa(dfa, word, &run, "-"))
        }
        Value::Nfa(nfa) => {
            let run = trace::trace_nfa(nfa, word);
            let mut frames = vec![frame(run.start.iter().copied().collect(), eps_edges(nfa, &run.start), Some(0), String::new())];
            for (i, step) in run.steps.iter().enumerate() {
                // The moves on the symbol out of the states before, then
                // the eps moves inside the states after.
                let mut taken: Vec<(State, State)> = Vec::new();
                for &s in &step.from {
                    for &t in nfa.delta[s].get(&Some(step.symbol)).into_iter().flatten() {
                        taken.push((s, t));
                    }
                }
                taken.extend(eps_edges(nfa, &step.to));
                frames.push(frame(step.to.iter().copied().collect(), taken, Some(i + 1), step.symbol.to_string()));
            }
            ("NFA", dot::nfa_diagram(nfa), frames, show_trace::render_nfa(nfa, word, &run, "-"))
        }
        Value::Pda(p) => {
            let search = p.search(word, pda::STEP_LIMIT);
            let frames = search
                .path
                .iter()
                .enumerate()
                .map(|(i, config)| {
                    let (taken, label) = match i {
                        0 => (vec![], String::new()),
                        _ => {
                            let m = &p.moves[search.moves[i - 1]];
                            (vec![(m.from, m.to)], p.label(m))
                        }
                    };
                    let mut f = frame(vec![config.state], taken, Some(config.read), label);
                    f.stack = Some(config.stack.iter().rev().cloned().collect());
                    f
                })
                .collect();
            ("PDA", dot::pda_diagram(p), frames, show_trace::render_pda(p, word, &search, "-"))
        }
        Value::Tm(m) => {
            let run = m.run(word, TM_STEPS, true);
            let frames = run
                .configs
                .iter()
                .enumerate()
                .map(|(i, config)| {
                    let (taken, label) = match i {
                        0 => (vec![], String::new()),
                        _ => {
                            let mv = &m.moves[run.moves[i - 1]];
                            (vec![(mv.from, mv.to)], m.label(mv))
                        }
                    };
                    let mut f = frame(vec![config.state], taken, None, label);
                    f.tape = Some(tape_window(config));
                    f
                })
                .collect();
            ("TM", dot::tm_diagram(m), frames, show_trace::render_tm(m, word, &run, "-"))
        }
        _ => return None,
    };
    let line = |prefix: &str| shown.lines().find_map(|l| l.strip_prefix(prefix)).unwrap_or("").to_string();
    Some(Play { kind, diagram, word: word.to_string(), frames, result: line("Result: "), reason: line("Reason: ") })
}

fn frame(active: Vec<State>, taken: Vec<(State, State)>, read: Option<usize>, label: String) -> Frame {
    Frame { active, taken, read, label, stack: None, tape: None }
}

/// The eps moves between the states of `set` (which is closed under them).
fn eps_edges(nfa: &crate::automata::Nfa, set: &BTreeSet<State>) -> Vec<(State, State)> {
    let mut edges = Vec::new();
    for &s in set {
        for &t in nfa.delta[s].get(&None).into_iter().flatten() {
            if set.contains(&t) && s != t {
                edges.push((s, t));
            }
        }
    }
    edges
}

/// The used part of the tape, widened to include the head, and where the
/// head is in it.
fn tape_window(config: &tm::Config) -> (Vec<char>, usize) {
    // `show` writes the head's cell in brackets: 10[1]1.
    let shown = config.tape.show(config.head);
    let mut cells = Vec::new();
    let mut head = 0;
    let mut chars = shown.chars();
    while let Some(c) = chars.next() {
        if c == '[' {
            head = cells.len();
            cells.push(chars.next().unwrap_or('_'));
            chars.next(); // ']'
        } else {
            cells.push(c);
        }
    }
    (cells, head)
}

/// What a run used, for `dot_run`: every state it was in, every edge it
/// took, and the states it ended in.
pub fn highlight(play: &Play) -> Highlight {
    let mut h = Highlight::default();
    for f in &play.frames {
        h.visited.extend(f.active.iter().copied());
        h.edges.extend(f.taken.iter().copied());
    }
    if let Some(last) = play.frames.last() {
        h.last = last.active.iter().copied().collect();
    }
    h
}

/// Where each state goes: in columns by its distance from the start state
/// (breadth first, following the edges), in the order the search finds
/// them; states that cannot be reached go in one more column at the end.
/// Each column is centred on the tallest one.
pub fn layout(diagram: &Diagram) -> Vec<(i64, i64)> {
    let n = diagram.states.len();
    let mut level = vec![usize::MAX; n];
    let mut order = Vec::new();
    let mut queue = VecDeque::from([diagram.start]);
    level[diagram.start] = 0;
    while let Some(s) = queue.pop_front() {
        order.push(s);
        for &(from, to) in diagram.edges.keys() {
            if from == s && level[to] == usize::MAX {
                level[to] = level[s] + 1;
                queue.push_back(to);
            }
        }
    }
    let reached = order.iter().map(|&s| level[s]).max().unwrap_or(0);
    for (s, l) in level.iter_mut().enumerate() {
        if *l == usize::MAX {
            *l = reached + 1;
            order.push(s);
        }
    }
    let columns = level.iter().max().map_or(1, |m| m + 1);
    let mut column: Vec<Vec<State>> = vec![Vec::new(); columns];
    for &s in &order {
        column[level[s]].push(s);
    }
    let tallest = column.iter().map(Vec::len).max().unwrap_or(1) as i64;
    let mut pos = vec![(0, 0); n];
    for (c, states) in column.iter().enumerate() {
        let top = (tallest - states.len() as i64) * GAP_Y / 2;
        for (row, &s) in states.iter().enumerate() {
            pos[s] = (MARGIN + c as i64 * GAP_X, MARGIN_TOP + top + row as i64 * GAP_Y);
        }
    }
    pos
}

const MARGIN: i64 = 70;
/// Room above the first row for self-loops and their labels.
const MARGIN_TOP: i64 = 100;
const GAP_X: i64 = 170;
const GAP_Y: i64 = 120;

/// The run as JSON for the page.
fn data(play: &Play, title: &str) -> serde_json::Value {
    use serde_json::json;
    let pos = layout(&play.diagram);
    let d = &play.diagram;
    let states: Vec<serde_json::Value> = d
        .states
        .iter()
        .enumerate()
        .map(|(s, name)| {
            json!({
                "name": name, "x": pos[s].0, "y": pos[s].1,
                "start": s == d.start, "accept": d.accept.contains(&s), "reject": d.reject.contains(&s),
            })
        })
        .collect();
    let edges: Vec<serde_json::Value> = d
        .edges
        .iter()
        .map(|(&(from, to), labels)| {
            // Symbols go on one line (`0, 1`); PDA and TM moves one per line.
            let labels = if d.separator == ", " { vec![labels.join(", ")] } else { labels.clone() };
            json!({ "from": from, "to": to, "labels": labels })
        })
        .collect();
    let frames: Vec<serde_json::Value> = play
        .frames
        .iter()
        .map(|f| {
            json!({
                "active": f.active, "taken": f.taken, "read": f.read, "label": f.label, "stack": f.stack,
                "tape": f.tape.as_ref().map(|(cells, head)| json!({ "cells": cells.iter().collect::<String>(), "head": head })),
            })
        })
        .collect();
    json!({
        "title": title, "kind": play.kind, "word": play.word, "states": states, "edges": edges,
        "frames": frames, "result": play.result, "reason": play.reason,
    })
}

/// The self-contained HTML page that plays `play`. `title` names the
/// machine on the page.
pub fn html(play: &Play, title: &str) -> String {
    // `<` is written as `\u003c` so nothing in the data can close the
    // script element it sits in.
    let json = data(play, title).to_string().replace('<', "\\u003c");
    let page_title = escape_html(&format!("{title} on \"{}\" - Kleene Star", play.word));
    PAGE.replace("{{THEME_CSS}}", &crate::theme::player_css())
        .replace("{{THEME}}", crate::theme::current().name())
        .replace("{{TITLE}}", &page_title)
        .replace("{{DATA}}", &json)
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const PAGE: &str = include_str!("player.html");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::tests::even_zeros;
    use crate::automata::nfa::tests::ends_in_01;
    use std::rc::Rc;

    #[test]
    fn a_dfa_run_has_a_frame_per_symbol() {
        let p = play(&Value::dfa(even_zeros()), "001").unwrap();
        assert_eq!(p.kind, "DFA");
        assert_eq!(p.frames.len(), 4);
        assert_eq!(p.frames[0].active, vec![0]);
        assert_eq!(p.frames[1].taken, vec![(0, 1)]);
        assert_eq!(p.frames[3].read, Some(3));
        assert_eq!(p.frames[3].label, "1");
        assert_eq!(p.result, "ACCEPTED");
        assert!(p.reason.contains("accepting"), "{}", p.reason);
        let h = highlight(&p);
        assert_eq!(h.last, BTreeSet::from([0]));
        assert!(h.edges.contains(&(0, 1)) && h.edges.contains(&(1, 0)));
    }

    #[test]
    fn an_nfa_run_shows_sets_and_the_moves_taken() {
        let p = play(&Value::nfa(ends_in_01()), "01").unwrap();
        assert_eq!(p.frames.len(), 3);
        assert!(p.frames[1].active.len() >= 2, "{:?}", p.frames[1]);
        assert!(!p.frames[1].taken.is_empty());
        assert_eq!(p.result, "ACCEPTED");
    }

    #[test]
    fn other_values_have_no_run() {
        assert!(play(&Value::Int(3), "0").is_none());
    }

    #[test]
    fn columns_follow_the_distance_from_the_start() {
        let pos = layout(&dot::dfa_diagram(&even_zeros()));
        assert_eq!(pos[0].0, MARGIN);
        assert_eq!(pos[1].0, MARGIN + GAP_X);
        // A third state no move reaches goes last.
        let mut d = even_zeros();
        d.states.push("lost".into());
        d.delta.push(vec![2, 2]);
        let pos = layout(&dot::dfa_diagram(&d));
        assert_eq!(pos[2].0, MARGIN + 2 * GAP_X);
    }

    #[test]
    fn the_page_holds_the_run_safely() {
        let mut d = even_zeros();
        d.states[0] = "</script><b>".into();
        let p = play(&Value::Dfa(Rc::new(d)), "0").unwrap();
        let page = html(&p, "E<v>");
        assert!(page.starts_with("<!DOCTYPE html>"));
        assert!(!page.contains("</script><b>"), "a state name must not end the script");
        assert!(page.contains("\\u003c/script>\\u003cb>"));
        assert!(page.contains("<title>E&lt;v&gt; on &quot;0&quot; - Kleene Star</title>"));
        assert!(!page.contains("{{"), "every placeholder is filled");
        assert!(!page.contains(" src=") && !page.contains("<link") && !page.contains("https://"), "the page loads nothing");
    }
}
