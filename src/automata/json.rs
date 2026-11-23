//! Machines and grammars as JSON: a documented, versioned format for
//! passing them to other programs and reading them back.
//!
//! Every file is one JSON object:
//!
//! ```text
//! {
//!   "format": "kleene-star",
//!   "version": 1,
//!   "kind": "dfa",
//!   "name": "EvenZeros",
//!   ...the fields of that kind...
//! }
//! ```
//!
//! `kind` is `dfa`, `nfa`, `regex`, `grammar`, `pda` or `tm`; `name` may be
//! left out. States, variables and symbols are written by *name*, never by
//! number, so a file can be read and written by hand, and the names survive
//! a round trip. `docs/reference.md` ("JSON") describes every field.
//!
//! **Reading is strict**, because a file may come from anywhere. The text is
//! limited in size, every list in length, every name in length and
//! characters; unknown fields are refused (a misspelt `"acept"` would
//! otherwise silently mean "accepts nothing"); every state, variable and
//! symbol a move mentions must be declared; and the rules a machine of that
//! kind must follow (a DFA total and deterministic, a Turing machine
//! deterministic) are checked. An error says where it is, as a path like
//! `transitions[3].to`. Nesting is limited by `serde_json` itself (128
//! levels); a valid file never nests deeper than 4.
//!
//! Writing produces the fields in a fixed, readable order, one move per
//! line, so files diff well.

use super::grammar::{Grammar, Rule, Sym};
use super::pda::{self, Pda};
use super::regex::{self, Regex};
use super::tm::{self, Dir, Tm};
use super::{Dfa, Nfa, State};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The value of the `format` field.
pub const FORMAT: &str = "kleene-star";
/// The version this code writes, and the newest it reads.
pub const VERSION: u64 = 1;
/// The longest text `from_json` reads, in bytes (1 MiB).
pub const MAX_BYTES: usize = 1 << 20;
/// The most states (or variables, or alphabet symbols) a file may declare.
pub const MAX_STATES: usize = 10_000;
/// The most transitions, rules or moves a file may list.
pub const MAX_ITEMS: usize = 100_000;
/// The longest name of a state, variable, stack symbol or machine.
pub const MAX_NAME: usize = 200;
/// The longest rule body, or push of a PDA move.
pub const MAX_BODY: usize = 1_000;

/// Anything that can be written as JSON.
#[derive(Debug, Clone, PartialEq)]
pub enum Machine {
    Dfa(Dfa),
    Nfa(Nfa),
    Regex(Regex),
    Grammar(Grammar),
    Pda(Pda),
    Tm(Tm),
}

impl Machine {
    /// The `kind` field for this machine.
    pub fn kind(&self) -> &'static str {
        match self {
            Machine::Dfa(_) => "dfa",
            Machine::Nfa(_) => "nfa",
            Machine::Regex(_) => "regex",
            Machine::Grammar(_) => "grammar",
            Machine::Pda(_) => "pda",
            Machine::Tm(_) => "tm",
        }
    }
}

/// Why a text is not a machine: where (`path`, such as `transitions[3].to`,
/// or empty for the whole text) and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "at `{}`: {}", self.path, self.message)
        }
    }
}

fn fail<T>(path: &str, message: impl Into<String>) -> Result<T, JsonError> {
    Err(JsonError { path: path.to_string(), message: message.into() })
}

// ---- writing ----

/// A JSON string literal, with JSON's escaping.
fn q(text: &str) -> String {
    serde_json::to_string(text).expect("a string always serializes")
}

fn qc(symbol: char) -> String {
    q(&symbol.to_string())
}

fn list(items: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(", "))
}

/// Writes `machine` as JSON (with `name`, if given).
pub fn to_json(machine: &Machine, name: Option<&str>) -> String {
    let mut fields = vec![
        format!("\"format\": {}", q(FORMAT)),
        format!("\"version\": {VERSION}"),
        format!("\"kind\": {}", q(machine.kind())),
    ];
    if let Some(name) = name {
        fields.push(format!("\"name\": {}", q(name)));
    }
    // Lists of moves get one item per line.
    let rows = |items: Vec<String>| {
        if items.is_empty() {
            "[]".to_string()
        } else {
            format!("[\n    {}\n  ]", items.join(",\n    "))
        }
    };
    let names = |names: &[String]| list(names.iter().map(|n| q(n)));
    let symbols = |alphabet: &[char]| list(alphabet.iter().map(|&c| qc(c)));
    let states = |m: &[String], set: &BTreeSet<State>| list(set.iter().map(|&s| q(&m[s])));
    match machine {
        Machine::Dfa(d) => {
            fields.push(format!("\"alphabet\": {}", symbols(&d.alphabet)));
            fields.push(format!("\"states\": {}", names(&d.states)));
            fields.push(format!("\"start\": {}", q(&d.states[d.start])));
            fields.push(format!("\"accept\": {}", states(&d.states, &d.accept)));
            let mut moves = Vec::new();
            for (s, row) in d.delta.iter().enumerate() {
                for (i, &t) in row.iter().enumerate() {
                    moves.push(format!(
                        "{{\"from\": {}, \"symbol\": {}, \"to\": {}}}",
                        q(&d.states[s]),
                        qc(d.alphabet[i]),
                        q(&d.states[t])
                    ));
                }
            }
            fields.push(format!("\"transitions\": {}", rows(moves)));
        }
        Machine::Nfa(n) => {
            fields.push(format!("\"alphabet\": {}", symbols(&n.alphabet)));
            fields.push(format!("\"states\": {}", names(&n.states)));
            fields.push(format!("\"start\": {}", q(&n.states[n.start])));
            fields.push(format!("\"accept\": {}", states(&n.states, &n.accept)));
            let mut moves = Vec::new();
            for (s, edges) in n.delta.iter().enumerate() {
                for (label, targets) in edges {
                    let symbol = label.map_or_else(|| "null".to_string(), qc);
                    for &t in targets {
                        moves.push(format!(
                            "{{\"from\": {}, \"symbol\": {symbol}, \"to\": {}}}",
                            q(&n.states[s]),
                            q(&n.states[t])
                        ));
                    }
                }
            }
            fields.push(format!("\"transitions\": {}", rows(moves)));
        }
        Machine::Regex(r) => fields.push(format!("\"regex\": {}", q(&r.to_string()))),
        Machine::Grammar(g) => {
            fields.push(format!("\"alphabet\": {}", symbols(&g.alphabet)));
            fields.push(format!("\"variables\": {}", names(&g.variables)));
            fields.push(format!("\"start\": {}", q(&g.variables[g.start])));
            let rules = g
                .rules
                .iter()
                .map(|rule| {
                    let body = rule.body.iter().map(|sym| match sym {
                        Sym::T(c) => format!("{{\"terminal\": {}}}", qc(*c)),
                        Sym::V(v) => format!("{{\"variable\": {}}}", q(&g.variables[*v])),
                    });
                    format!("{{\"head\": {}, \"body\": {}}}", q(&g.variables[rule.head]), list(body))
                })
                .collect();
            fields.push(format!("\"rules\": {}", rows(rules)));
        }
        Machine::Pda(p) => {
            fields.push(format!("\"alphabet\": {}", symbols(&p.alphabet)));
            fields.push(format!("\"states\": {}", names(&p.states)));
            fields.push(format!("\"start\": {}", q(&p.states[p.start])));
            fields.push(format!("\"accept\": {}", states(&p.states, &p.accept)));
            fields.push(format!("\"bottom\": {}", p.bottom.as_deref().map_or_else(|| "null".to_string(), q)));
            if p.empty_stack {
                fields.push("\"accept_by\": \"empty stack\"".to_string());
            }
            let moves = p
                .moves
                .iter()
                .map(|m| {
                    format!(
                        "{{\"from\": {}, \"read\": {}, \"pop\": {}, \"push\": {}, \"to\": {}}}",
                        q(&p.states[m.from]),
                        m.read.map_or_else(|| "null".to_string(), qc),
                        m.pop.as_deref().map_or_else(|| "null".to_string(), q),
                        names(&m.push),
                        q(&p.states[m.to])
                    )
                })
                .collect();
            fields.push(format!("\"moves\": {}", rows(moves)));
        }
        Machine::Tm(m) => {
            fields.push(format!("\"alphabet\": {}", symbols(&m.alphabet)));
            fields.push(format!("\"states\": {}", names(&m.states)));
            fields.push(format!("\"start\": {}", q(&m.states[m.start])));
            fields.push(format!("\"accept\": {}", states(&m.states, &m.accept)));
            fields.push(format!("\"reject\": {}", states(&m.states, &m.reject)));
            let moves = m
                .moves
                .iter()
                .map(|mv| {
                    format!(
                        "{{\"from\": {}, \"read\": {}, \"write\": {}, \"move\": {}, \"to\": {}}}",
                        q(&m.states[mv.from]),
                        qc(mv.read),
                        qc(mv.write),
                        qc(mv.dir.letter()),
                        q(&m.states[mv.to])
                    )
                })
                .collect();
            fields.push(format!("\"moves\": {}", rows(moves)));
        }
    }
    format!("{{\n  {}\n}}", fields.join(",\n  "))
}

// ---- reading ----

/// One JSON object being read, with its path for error messages. Each
/// field is taken at most once, and `done` refuses any field nobody took.
struct Obj<'a> {
    map: &'a Map<String, Value>,
    path: String,
    taken: Vec<&'static str>,
}

impl<'a> Obj<'a> {
    fn new(value: &'a Value, path: &str) -> Result<Obj<'a>, JsonError> {
        match value {
            Value::Object(map) => Ok(Obj { map, path: path.to_string(), taken: Vec::new() }),
            other => fail(path, format!("should be an object {{...}}, but is {}", describe(other))),
        }
    }

    /// The path of field `key`, like `transitions[2].to`.
    fn at(&self, key: &str) -> String {
        if self.path.is_empty() {
            key.to_string()
        } else {
            format!("{}.{key}", self.path)
        }
    }

    fn get(&mut self, key: &'static str) -> Result<&'a Value, JsonError> {
        self.taken.push(key);
        self.map.get(key).ok_or_else(|| JsonError {
            path: self.path.clone(),
            message: format!("the field `{key}` is missing"),
        })
    }

    fn optional(&mut self, key: &'static str) -> Option<&'a Value> {
        self.taken.push(key);
        self.map.get(key)
    }

    /// Refuses fields that were never asked for: probably a typo.
    fn done(&self) -> Result<(), JsonError> {
        match self.map.keys().find(|k| !self.taken.contains(&k.as_str())) {
            Some(key) => fail(&self.path, format!("unknown field `{key}`; the fields here are {}", self.taken.join(", "))),
            None => Ok(()),
        }
    }
}

/// "a number", "an array": what a JSON value is, for messages.
fn describe(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "true or false",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn string<'a>(value: &'a Value, path: &str) -> Result<&'a str, JsonError> {
    match value {
        Value::String(s) => Ok(s),
        other => fail(path, format!("should be a string, but is {}", describe(other))),
    }
}

fn array<'a>(value: &'a Value, path: &str, max: usize) -> Result<&'a [Value], JsonError> {
    match value {
        Value::Array(items) if items.len() > max => {
            fail(path, format!("has {} items, more than the limit of {max}", items.len()))
        }
        Value::Array(items) => Ok(items),
        other => fail(path, format!("should be an array [...], but is {}", describe(other))),
    }
}

/// Whether `text` can be a name in a file: of a state, variable, stack
/// symbol or machine. It needs 1 to `MAX_NAME` characters, none of them
/// control characters.
pub fn check_name(text: &str) -> Result<(), String> {
    let length = text.chars().count();
    if length == 0 || length > MAX_NAME {
        return Err(format!("a name must have 1 to {MAX_NAME} characters, but this has {length}"));
    }
    match text.chars().find(|c| c.is_control()) {
        Some(c) => Err(format!("a name cannot contain the control character {c:?}")),
        None => Ok(()),
    }
}

fn name(value: &Value, path: &str) -> Result<String, JsonError> {
    let text = string(value, path)?;
    check_name(text).map_err(|message| JsonError { path: path.to_string(), message })?;
    Ok(text.to_string())
}

/// A symbol: a string of one letter or digit (or `_` where `blank_ok`).
fn symbol(value: &Value, path: &str, blank_ok: bool) -> Result<char, JsonError> {
    let text = string(value, path)?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_alphanumeric() || (blank_ok && c == tm::BLANK) => Ok(c),
        (Some(tm::BLANK), None) => fail(path, "`_` is the blank of a Turing machine's tape; it cannot be an input symbol"),
        _ => fail(path, format!("a symbol is one letter or digit, but this is {text:?}")),
    }
}

/// The `alphabet` field: distinct symbols, returned sorted.
fn alphabet(obj: &mut Obj) -> Result<Vec<char>, JsonError> {
    let path = obj.at("alphabet");
    let mut seen = BTreeSet::new();
    for (i, item) in array(obj.get("alphabet")?, &path, MAX_STATES)?.iter().enumerate() {
        let at = format!("{path}[{i}]");
        if !seen.insert(symbol(item, &at, false)?) {
            return fail(&at, "this symbol is listed twice");
        }
    }
    Ok(seen.into_iter().collect())
}

/// A list of distinct names (states or variables), in order, with an index
/// to look them up.
fn declared(obj: &mut Obj, key: &'static str) -> Result<(Vec<String>, HashMap<String, usize>), JsonError> {
    let path = obj.at(key);
    let items = array(obj.get(key)?, &path, MAX_STATES)?;
    if items.is_empty() {
        return fail(&path, "there must be at least one; `start` names one of them");
    }
    let mut names = Vec::new();
    let mut index = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        let at = format!("{path}[{i}]");
        let n = name(item, &at)?;
        if index.insert(n.clone(), i).is_some() {
            return fail(&at, format!("{n:?} is listed twice"));
        }
        names.push(n);
    }
    Ok((names, index))
}

/// A reference to one of the declared names.
fn reference(value: &Value, path: &str, index: &HashMap<String, usize>, what: &str) -> Result<usize, JsonError> {
    let text = string(value, path)?;
    index.get(text).copied().ok_or_else(|| JsonError {
        path: path.to_string(),
        message: format!("{text:?} is not one of the declared {what}"),
    })
}

/// A list of references, such as the accepting states.
fn references(obj: &mut Obj, key: &'static str, index: &HashMap<String, usize>, what: &str) -> Result<BTreeSet<usize>, JsonError> {
    let path = obj.at(key);
    let mut set = BTreeSet::new();
    for (i, item) in array(obj.get(key)?, &path, MAX_STATES)?.iter().enumerate() {
        set.insert(reference(item, &format!("{path}[{i}]"), index, what)?);
    }
    Ok(set)
}

/// A symbol that must be in `alphabet`.
fn symbol_in(value: &Value, path: &str, alphabet: &[char]) -> Result<char, JsonError> {
    let c = symbol(value, path, false)?;
    if !alphabet.contains(&c) {
        return fail(path, format!("symbol `{c}` is not in the alphabet"));
    }
    Ok(c)
}

/// Reads a machine written by `to_json` (or by hand, or by another
/// program), and its name if it has one.
pub fn from_json(text: &str) -> Result<(Machine, Option<String>), JsonError> {
    if text.len() > MAX_BYTES {
        return fail("", format!("the text is {} bytes, more than the limit of {MAX_BYTES} (1 MiB)", text.len()));
    }
    let value: Value = serde_json::from_str(text).map_err(|e| JsonError { path: String::new(), message: format!("this is not valid JSON: {e}") })?;
    let mut root = Obj::new(&value, "")?;
    match string(root.get("format")?, "format")? {
        FORMAT => {}
        other => return fail("format", format!("should be {FORMAT:?}, but is {other:?}: this is not a Kleene Star file")),
    }
    match root.get("version")?.as_u64() {
        Some(VERSION) => {}
        Some(v) if v > VERSION => {
            return fail("version", format!("version {v} was written by a newer Kleene Star; this one reads version {VERSION}"))
        }
        _ => return fail("version", format!("should be the number {VERSION}")),
    }
    let kind = string(root.get("kind")?, "kind")?;
    let name = root.optional("name").map(|n| name(n, "name")).transpose()?;
    let machine = match kind {
        "dfa" => Machine::Dfa(read_dfa(&mut root)?),
        "nfa" => Machine::Nfa(read_nfa(&mut root)?),
        "regex" => {
            let text = string(root.get("regex")?, "regex")?;
            Machine::Regex(regex::parse(text).map_err(|(_, message)| JsonError { path: "regex".into(), message: format!("not a regex: {message}") })?)
        }
        "grammar" => Machine::Grammar(read_grammar(&mut root)?),
        "pda" => Machine::Pda(read_pda(&mut root)?),
        "tm" => Machine::Tm(read_tm(&mut root)?),
        other => return fail("kind", format!("{other:?} is not a kind; the kinds are \"dfa\", \"nfa\", \"regex\", \"grammar\", \"pda\" and \"tm\"")),
    };
    root.done()?;
    Ok((machine, name))
}

/// The shared part of a DFA and an NFA, as read.
struct Finite {
    alphabet: Vec<char>,
    states: Vec<String>,
    start: State,
    accept: BTreeSet<State>,
    /// Each transition as (from, symbol or eps, to, its path for messages).
    transitions: Vec<(State, Option<char>, State, String)>,
}

fn read_finite(root: &mut Obj, eps_ok: bool) -> Result<Finite, JsonError> {
    let alphabet = alphabet(root)?;
    let (states, index) = declared(root, "states")?;
    let start = reference(root.get("start")?, "start", &index, "states")?;
    let accept = references(root, "accept", &index, "states")?;
    let path = root.at("transitions");
    let mut transitions = Vec::new();
    for (i, item) in array(root.get("transitions")?, &path, MAX_ITEMS)?.iter().enumerate() {
        let at = format!("{path}[{i}]");
        let mut t = Obj::new(item, &at)?;
        let from = reference(t.get("from")?, &t.at("from"), &index, "states")?;
        let symbol = match t.get("symbol")? {
            Value::Null if eps_ok => None,
            Value::Null => return fail(&t.at("symbol"), "a DFA has no eps moves (`null`); use kind \"nfa\""),
            other => Some(symbol_in(other, &t.at("symbol"), &alphabet)?),
        };
        let to = reference(t.get("to")?, &t.at("to"), &index, "states")?;
        t.done()?;
        transitions.push((from, symbol, to, at));
    }
    Ok(Finite { alphabet, states, start, accept, transitions })
}

fn read_dfa(root: &mut Obj) -> Result<Dfa, JsonError> {
    let Finite { alphabet, states, start, accept, transitions } = read_finite(root, false)?;
    let mut delta: Vec<Vec<Option<State>>> = vec![vec![None; alphabet.len()]; states.len()];
    for (from, symbol, to, at) in transitions {
        let c = symbol.expect("read_finite refused eps for a DFA");
        let i = alphabet.iter().position(|&a| a == c).expect("the symbol is in the alphabet");
        if delta[from][i].replace(to).is_some() {
            return fail(&at, format!("a second transition from {:?} on `{c}`: a DFA has exactly one", states[from]));
        }
    }
    let mut table = Vec::new();
    for (s, row) in delta.into_iter().enumerate() {
        let mut targets = Vec::new();
        for (i, t) in row.into_iter().enumerate() {
            match t {
                Some(t) => targets.push(t),
                None => {
                    return fail("transitions", format!("no transition from {:?} on `{}`: a DFA has one for every state and symbol", states[s], alphabet[i]))
                }
            }
        }
        table.push(targets);
    }
    Ok(Dfa { alphabet, states, start, accept, delta: table })
}

fn read_nfa(root: &mut Obj) -> Result<Nfa, JsonError> {
    let Finite { alphabet, states, start, accept, transitions } = read_finite(root, true)?;
    let mut delta = vec![BTreeMap::<Option<char>, BTreeSet<State>>::new(); states.len()];
    for (from, symbol, to, _) in transitions {
        delta[from].entry(symbol).or_default().insert(to);
    }
    Ok(Nfa { alphabet, states, start, accept, delta })
}

fn read_grammar(root: &mut Obj) -> Result<Grammar, JsonError> {
    let alphabet = alphabet(root)?;
    let (variables, index) = declared(root, "variables")?;
    let start = reference(root.get("start")?, "start", &index, "variables")?;
    let path = root.at("rules");
    let mut rules = Vec::new();
    for (i, item) in array(root.get("rules")?, &path, MAX_ITEMS)?.iter().enumerate() {
        let at = format!("{path}[{i}]");
        let mut r = Obj::new(item, &at)?;
        let head = reference(r.get("head")?, &r.at("head"), &index, "variables")?;
        let body_path = r.at("body");
        let mut body = Vec::new();
        for (j, sym) in array(r.get("body")?, &body_path, MAX_BODY)?.iter().enumerate() {
            let sym_path = format!("{body_path}[{j}]");
            let mut s = Obj::new(sym, &sym_path)?;
            let read = match (s.optional("terminal"), s.optional("variable")) {
                (Some(t), None) => Sym::T(symbol_in(t, &s.at("terminal"), &alphabet)?),
                (None, Some(v)) => Sym::V(reference(v, &s.at("variable"), &index, "variables")?),
                _ => return fail(&sym_path, "each symbol of a body is {\"terminal\": ...} or {\"variable\": ...}"),
            };
            s.done()?;
            body.push(read);
        }
        r.done()?;
        rules.push(Rule { head, body });
    }
    Ok(Grammar { alphabet, variables, start, rules })
}

fn read_pda(root: &mut Obj) -> Result<Pda, JsonError> {
    let alphabet = alphabet(root)?;
    let (states, index) = declared(root, "states")?;
    let start = reference(root.get("start")?, "start", &index, "states")?;
    let accept = references(root, "accept", &index, "states")?;
    let bottom = match root.get("bottom")? {
        Value::Null => None,
        other => Some(name(other, "bottom")?),
    };
    let empty_stack = match root.optional("accept_by") {
        None => false,
        Some(v) => match string(v, "accept_by")? {
            "final state" => false,
            "empty stack" => true,
            _ => return fail("accept_by", "should be \"final state\" or \"empty stack\""),
        },
    };
    let path = root.at("moves");
    let mut moves = Vec::new();
    for (i, item) in array(root.get("moves")?, &path, MAX_ITEMS)?.iter().enumerate() {
        let at = format!("{path}[{i}]");
        let mut m = Obj::new(item, &at)?;
        let from = reference(m.get("from")?, &m.at("from"), &index, "states")?;
        let read = match m.get("read")? {
            Value::Null => None,
            other => Some(symbol_in(other, &m.at("read"), &alphabet)?),
        };
        let pop = match m.get("pop")? {
            Value::Null => None,
            other => Some(name(other, &m.at("pop"))?),
        };
        let push_path = m.at("push");
        let push = array(m.get("push")?, &push_path, MAX_BODY)?
            .iter()
            .enumerate()
            .map(|(j, s)| name(s, &format!("{push_path}[{j}]")))
            .collect::<Result<Vec<_>, _>>()?;
        let to = reference(m.get("to")?, &m.at("to"), &index, "states")?;
        m.done()?;
        moves.push(pda::Move { from, read, pop, push, to });
    }
    Ok(Pda { alphabet, states, start, accept, moves, bottom, empty_stack })
}

fn read_tm(root: &mut Obj) -> Result<Tm, JsonError> {
    let alphabet = alphabet(root)?;
    let (states, index) = declared(root, "states")?;
    let start = reference(root.get("start")?, "start", &index, "states")?;
    let accept = references(root, "accept", &index, "states")?;
    let reject = references(root, "reject", &index, "states")?;
    if let Some(&s) = accept.intersection(&reject).next() {
        return fail("reject", format!("{:?} is both accepting and rejecting", states[s]));
    }
    let path = root.at("moves");
    let mut moves = Vec::new();
    let mut seen = BTreeSet::new();
    for (i, item) in array(root.get("moves")?, &path, MAX_ITEMS)?.iter().enumerate() {
        let at = format!("{path}[{i}]");
        let mut m = Obj::new(item, &at)?;
        let from = reference(m.get("from")?, &m.at("from"), &index, "states")?;
        let read = symbol(m.get("read")?, &m.at("read"), true)?;
        let write = symbol(m.get("write")?, &m.at("write"), true)?;
        let dir = match string(m.get("move")?, &m.at("move"))? {
            "L" => Dir::Left,
            "R" => Dir::Right,
            "S" => Dir::Stay,
            other => return fail(&m.at("move"), format!("should be \"L\", \"R\" or \"S\", but is {other:?}")),
        };
        let to = reference(m.get("to")?, &m.at("to"), &index, "states")?;
        m.done()?;
        if !seen.insert((from, read)) {
            return fail(&at, format!("a second move from {:?} reading `{read}`: a Turing machine here is deterministic", states[from]));
        }
        moves.push(tm::Move { from, read, write, dir, to });
    }
    Ok(Tm { alphabet, states, start, accept, reject, moves })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::tests::even_zeros;

    fn error(text: &str) -> String {
        from_json(text).expect_err("expected an error").to_string()
    }

    /// A small valid DFA file, with `extra` spliced in as more fields.
    fn dfa_file(extra: &str) -> String {
        format!(
            r#"{{"format": "kleene-star", "version": 1, "kind": "dfa", "alphabet": ["0"], "states": ["a"], "start": "a", "accept": [], "transitions": [{{"from": "a", "symbol": "0", "to": "a"}}]{extra}}}"#
        )
    }

    #[test]
    fn a_dfa_is_written_readably() {
        let text = to_json(&Machine::Dfa(even_zeros()), Some("EvenZeros"));
        assert!(text.starts_with("{\n  \"format\": \"kleene-star\",\n  \"version\": 1,\n  \"kind\": \"dfa\",\n  \"name\": \"EvenZeros\","), "{text}");
        assert!(text.contains("\n    {\"from\": \"q0\", \"symbol\": \"0\", \"to\": \"q1\"},\n"), "{text}");
        assert_eq!(from_json(&text), Ok((Machine::Dfa(even_zeros()), Some("EvenZeros".into()))));
    }

    #[test]
    fn the_header_is_checked() {
        assert!(from_json(&dfa_file("")).is_ok());
        assert!(error("[1, 2]").contains("should be an object"));
        assert!(error("{").starts_with("this is not valid JSON"));
        assert!(error(&dfa_file("").replace("kleene-star", "other")).contains("not a Kleene Star file"));
        assert!(error(&dfa_file("").replace("\"version\": 1", "\"version\": 2")).contains("newer Kleene Star"));
        assert!(error(&dfa_file("").replace("\"version\": 1", "\"version\": \"1\"")).contains("should be the number 1"));
        assert!(error(&dfa_file("").replace("\"dfa\"", "\"fsm\"")).contains("is not a kind"));
        assert_eq!(error(&dfa_file(", \"acept\": []")), "unknown field `acept`; the fields here are format, version, kind, name, alphabet, states, start, accept, transitions");
    }

    #[test]
    fn machines_must_be_well_formed() {
        assert_eq!(
            error(&dfa_file("").replace("\"to\": \"a\"", "\"to\": \"b\"")),
            "at `transitions[0].to`: \"b\" is not one of the declared states"
        );
        assert!(error(&dfa_file("").replace("\"symbol\": \"0\"", "\"symbol\": \"7\"")).contains("not in the alphabet"));
        assert!(error(&dfa_file("").replace("\"symbol\": \"0\"", "\"symbol\": null")).contains("no eps moves"));
        assert!(error(&dfa_file("").replace("[\"0\"]", "[\"0\", \"1\"]")).contains("no transition from \"a\" on `1`"));
        assert!(error(&dfa_file("").replace("[\"0\"]", "[\"0\", \"0\"]")).contains("listed twice"));
        assert!(error(&dfa_file("").replace("[\"0\"]", "[\"01\"]")).contains("one letter or digit"));
        assert!(error(&dfa_file("").replace("[\"a\"]", "[\"\"]")).contains("1 to 200 characters"));
        assert!(error(&dfa_file("").replace("[\"a\"]", "[\"a\\u0007\"]")).contains("control character"));
        assert!(error(&dfa_file("").replace("[\"a\"]", "[]")).contains("at least one"));
        let twice = dfa_file("").replace("}]", "}, {\"from\": \"a\", \"symbol\": \"0\", \"to\": \"a\"}]");
        assert!(error(&twice).contains("a second transition"));
    }

    #[test]
    fn limits_on_untrusted_text() {
        let huge = format!("{} {}", dfa_file(""), " ".repeat(MAX_BYTES));
        assert!(error(&huge).contains("more than the limit of 1048576"));
        let deep = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
        assert!(error(&deep).starts_with("this is not valid JSON"), "serde_json refuses deep nesting");
        let many: Vec<String> = (0..=MAX_STATES).map(|i| format!("\"s{i}\"")).collect();
        let text = dfa_file("").replace("[\"a\"]", &format!("[{}]", many.join(",")));
        assert!(error(&text).contains("more than the limit of 10000"));
    }

    #[test]
    fn grammars_tell_terminals_from_variables() {
        // `to_grammar` over an alphabet with capital letters makes a
        // terminal `A` and a variable `A`; the file keeps them apart.
        let g = Grammar {
            alphabet: vec!['A'],
            variables: vec!["S".into(), "A".into()],
            start: 0,
            rules: vec![Rule { head: 0, body: vec![Sym::T('A'), Sym::V(1)] }, Rule { head: 1, body: vec![] }],
        };
        let text = to_json(&Machine::Grammar(g.clone()), None);
        assert!(text.contains(r#"{"head": "S", "body": [{"terminal": "A"}, {"variable": "A"}]}"#), "{text}");
        assert_eq!(from_json(&text), Ok((Machine::Grammar(g), None)));
    }

    #[test]
    fn turing_machines_stay_deterministic() {
        let tm = r#"{"format": "kleene-star", "version": 1, "kind": "tm", "alphabet": ["a"], "states": ["s", "y"], "start": "s", "accept": ["y"], "reject": [],
            "moves": [{"from": "s", "read": "a", "write": "_", "move": "R", "to": "s"}, {"from": "s", "read": "_", "write": "_", "move": "S", "to": "y"}]}"#;
        assert!(from_json(tm).is_ok());
        assert!(error(&tm.replace("\"move\": \"R\"", "\"move\": \"U\"")).contains("\"L\", \"R\" or \"S\""));
        assert!(error(&tm.replace("\"read\": \"_\"", "\"read\": \"a\"")).contains("a second move"));
        assert!(error(&tm.replace("\"reject\": []", "\"reject\": [\"y\"]")).contains("both accepting and rejecting"));
        assert!(error(&tm.replace("[\"a\"]", "[\"_\"]")).contains("blank"));
    }
}
