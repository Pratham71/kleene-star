//! Parse trees of a word, and ambiguity.
//!
//! A grammar is ambiguous on a word when the word has two different parse
//! trees, or, what is the same, two different leftmost derivations. The
//! trees are found by trying each rule on each part of the word, using
//! which variables can derive which parts (computed first, as in the CYK
//! algorithm but for any grammar).
//!
//! A grammar with a cycle, a variable that derives itself through unit
//! rules and variables that vanish (`A -> B`, `B -> A`, or `A -> A C` with
//! `C -> eps`), gives some words infinitely many trees: the cycle can go
//! round any number of times. So the trees listed are those in which no
//! variable covers the same part of the word twice on one path from the
//! root; there are finitely many. A word with a tree that goes through a
//! cycle is ambiguous all the same, and `is_ambiguous_on` says so.

use super::grammar::{Grammar, Sym};
use std::collections::BTreeSet;

/// A parse tree: a terminal, or a variable and the trees of its rule's
/// body (none for a rule `A -> eps`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tree {
    Leaf(char),
    Node(usize, Vec<Tree>),
}

impl Tree {
    /// The tree written with brackets: `S(a S(eps) b)`.
    pub fn show(&self, g: &Grammar) -> String {
        match self {
            Tree::Leaf(c) => c.to_string(),
            Tree::Node(v, children) if children.is_empty() => format!("{}(eps)", g.variables[*v]),
            Tree::Node(v, children) => {
                let inner: Vec<String> = children.iter().map(|t| t.show(g)).collect();
                format!("{}({})", g.variables[*v], inner.join(" "))
            }
        }
    }

    /// The leftmost derivation the tree stands for: the sentential forms
    /// from the start variable, each rewriting the leftmost variable.
    pub fn leftmost(&self) -> Vec<Vec<Sym>> {
        // A form is a list of pending subtrees; terminals and expanded
        // nodes are replaced in place.
        let mut form: Vec<&Tree> = vec![self];
        let shown = |form: &[&Tree]| -> Vec<Sym> {
            form.iter()
                .map(|t| match t {
                    Tree::Leaf(c) => Sym::T(*c),
                    Tree::Node(v, _) => Sym::V(*v),
                })
                .collect()
        };
        let mut steps = vec![shown(&form)];
        while let Some(at) = form.iter().position(|t| matches!(t, Tree::Node(..))) {
            let Tree::Node(_, children) = form[at] else { unreachable!() };
            form.splice(at..=at, children.iter());
            steps.push(shown(&form));
        }
        steps
    }
}

/// What the search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trees {
    /// The trees without a cycle (see the module docs), sorted, at most
    /// the limit given.
    pub trees: Vec<Tree>,
    /// Whether there were more than the limit.
    pub more: bool,
    /// Whether some tree of the word goes through a cycle, so that it has
    /// infinitely many.
    pub cyclic: bool,
}

/// The variables that derive themselves through unit rules and vanishing
/// variables: `A =>+ A`.
fn cyclic_variables(g: &Grammar) -> Vec<bool> {
    let nullable = g.nullable();
    let n = g.variables.len();
    // edge[a][b]: a rule a -> x B y with x and y vanishing.
    let mut edge = vec![vec![false; n]; n];
    for rule in &g.rules {
        for (i, s) in rule.body.iter().enumerate() {
            let Sym::V(b) = *s else { continue };
            let vanish = |s: &Sym| matches!(s, Sym::V(v) if nullable[*v]);
            if rule.body[..i].iter().all(vanish) && rule.body[i + 1..].iter().all(vanish) {
                edge[rule.head][b] = true;
            }
        }
    }
    // Transitive closure (Warshall).
    for k in 0..n {
        let through = edge[k].clone();
        for row in edge.iter_mut() {
            if row[k] {
                for (cell, &via) in row.iter_mut().zip(&through) {
                    *cell |= via;
                }
            }
        }
    }
    (0..n).map(|v| edge[v][v]).collect()
}

struct Search<'a> {
    g: &'a Grammar,
    word: Vec<char>,
    /// derives[v][i][j]: variable v derives word[i..j].
    derives: Vec<Vec<Vec<bool>>>,
    limit: usize,
}

impl Search<'_> {
    fn sym_derives(&self, s: &Sym, i: usize, j: usize) -> bool {
        match *s {
            Sym::T(c) => j == i + 1 && self.word[i] == c,
            Sym::V(v) => self.derives[v][i][j],
        }
    }

    /// Whether `body` can derive word[i..j].
    fn body_derives(&self, body: &[Sym], i: usize, j: usize) -> bool {
        match body.split_first() {
            None => i == j,
            Some((first, rest)) => (i..=j).any(|k| self.sym_derives(first, i, k) && self.body_derives(rest, k, j)),
        }
    }

    /// The trees of `v` over word[i..j] with no variable covering the same
    /// part twice on a path; `path` holds the (variable, i, j) above.
    fn trees(&self, v: usize, i: usize, j: usize, path: &mut Vec<(usize, usize, usize)>) -> Vec<Tree> {
        if !self.derives[v][i][j] || path.contains(&(v, i, j)) {
            return Vec::new();
        }
        path.push((v, i, j));
        let mut out = Vec::new();
        for rule in self.g.rules_for(v) {
            for children in self.bodies(&rule.body, i, j, path) {
                out.push(Tree::Node(v, children));
                if out.len() > self.limit {
                    break;
                }
            }
            if out.len() > self.limit {
                break;
            }
        }
        path.pop();
        out
    }

    /// Every way `body` derives word[i..j], as the list of child trees.
    fn bodies(&self, body: &[Sym], i: usize, j: usize, path: &mut Vec<(usize, usize, usize)>) -> Vec<Vec<Tree>> {
        let Some((first, rest)) = body.split_first() else {
            return if i == j { vec![Vec::new()] } else { Vec::new() };
        };
        let mut out = Vec::new();
        for k in i..=j {
            if !self.sym_derives(first, i, k) || !self.body_derives(rest, k, j) {
                continue;
            }
            let heads = match *first {
                Sym::T(c) => vec![Tree::Leaf(c)],
                Sym::V(w) => self.trees(w, i, k, path),
            };
            if heads.is_empty() {
                continue;
            }
            let tails = self.bodies(rest, k, j, path);
            for head in &heads {
                for tail in &tails {
                    let mut children = vec![head.clone()];
                    children.extend(tail.iter().cloned());
                    out.push(children);
                    if out.len() > self.limit {
                        return out;
                    }
                }
            }
        }
        out
    }
}

/// The parse trees of `word`, at most `limit` of them. The word must be
/// over the grammar's alphabet.
pub fn parse_trees(g: &Grammar, word: &str, limit: usize) -> Trees {
    let word: Vec<char> = word.chars().collect();
    let n = word.len();
    let mut search = Search { g, word, derives: vec![vec![vec![false; n + 1]; n + 1]; g.variables.len()], limit };
    // Which variables derive which parts: grow until nothing changes.
    let mut changed = true;
    while changed {
        changed = false;
        for rule in &g.rules {
            for i in 0..=n {
                for j in i..=n {
                    if !search.derives[rule.head][i][j] && search.body_derives(&rule.body, i, j) {
                        search.derives[rule.head][i][j] = true;
                        changed = true;
                    }
                }
            }
        }
    }
    let mut trees = search.trees(g.start, 0, n, &mut Vec::new());
    let mut seen = BTreeSet::new();
    trees.retain(|t| seen.insert(t.clone()));
    trees.sort();
    let more = trees.len() > limit;
    trees.truncate(limit);
    // A tree goes through a cycle when some node's variable is cyclic and
    // covers a part of the word: going round the cycle there gives another
    // tree.
    let cyclic_var = cyclic_variables(g);
    let cyclic = trees.iter().any(|t| uses(t, &cyclic_var));
    Trees { trees, more, cyclic }
}

fn uses(t: &Tree, cyclic: &[bool]) -> bool {
    match t {
        Tree::Leaf(_) => false,
        Tree::Node(v, children) => cyclic[*v] || children.iter().any(|c| uses(c, cyclic)),
    }
}

/// The trees in Graphviz DOT, side by side, each under its number.
pub fn to_dot(g: &Grammar, trees: &[Tree]) -> String {
    fn node(g: &Grammar, t: &Tree, next: &mut usize, out: &mut String) -> usize {
        let id = *next;
        *next += 1;
        let (label, shape) = match t {
            Tree::Leaf(c) => (c.to_string(), "plaintext"),
            Tree::Node(v, _) => (g.variables[*v].clone(), "ellipse"),
        };
        let label = label.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!("    n{id} [label=\"{label}\", shape={shape}];\n"));
        if let Tree::Node(_, children) = t {
            if children.is_empty() {
                let eps = *next;
                *next += 1;
                out.push_str(&format!("    n{eps} [label=\"eps\", shape=plaintext];\n    n{id} -> n{eps};\n"));
            }
            for child in children {
                let c = node(g, child, next, out);
                out.push_str(&format!("    n{id} -> n{c};\n"));
            }
        }
        id
    }
    let t = crate::theme::current().theme();
    let mut out = String::from("digraph parse_trees {\n");
    out.push_str(&super::dot::palette(t));
    out.push_str(&format!("    node [fontname=\"Helvetica\"];\n    graph [fontcolor=\"{}\", color=\"{}\"];\n", t.text, t.border));
    let mut next = 0;
    for (k, t) in trees.iter().enumerate() {
        out.push_str(&format!("  subgraph cluster_{k} {{\n    label=\"tree {}\";\n", k + 1));
        node(g, t, &mut next, &mut out);
        out.push_str("  }\n");
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::cfg;

    /// E -> E + E | E * E | a, written with p for + and m for *.
    fn expressions() -> Grammar {
        let e = 0;
        Grammar {
            alphabet: vec!['a', 'm', 'p'],
            variables: vec!["E".into()],
            start: e,
            rules: vec![
                crate::automata::grammar::Rule { head: e, body: vec![Sym::V(e), Sym::T('p'), Sym::V(e)] },
                crate::automata::grammar::Rule { head: e, body: vec![Sym::V(e), Sym::T('m'), Sym::V(e)] },
                crate::automata::grammar::Rule { head: e, body: vec![Sym::T('a')] },
            ],
        }
    }

    #[test]
    fn an_ambiguous_word_has_two_trees() {
        let g = expressions();
        let found = parse_trees(&g, "apama", 10);
        assert_eq!(found.trees.len(), 2, "{:?}", found.trees);
        let shown: Vec<String> = found.trees.iter().map(|t| t.show(&g)).collect();
        assert!(shown.contains(&"E(E(E(a) p E(a)) m E(a))".to_string()), "{shown:?}");
        assert!(shown.contains(&"E(E(a) p E(E(a) m E(a)))".to_string()), "{shown:?}");
        assert!(!found.cyclic && !found.more);
        // The two leftmost derivations differ.
        let (d1, d2) = (found.trees[0].leftmost(), found.trees[1].leftmost());
        assert_ne!(d1, d2);
        assert_eq!(d1.last(), d2.last());
        assert_eq!(parse_trees(&g, "apa", 10).trees.len(), 1);
        assert_eq!(parse_trees(&g, "ap", 10).trees.len(), 0);
    }

    #[test]
    fn unambiguous_grammars_give_one_tree() {
        let g = cfg::anbn('a', 'b');
        for w in ["", "ab", "aabb", "aaabbb"] {
            let found = parse_trees(&g, w, 10);
            assert_eq!(found.trees.len(), 1, "{w}");
            let steps = found.trees[0].leftmost();
            assert_eq!(steps.len(), g.derive(w, 10_000).unwrap().len(), "{w}");
        }
    }

    #[test]
    fn a_cycle_is_noticed_and_kept_finite() {
        // S -> S | a: every word has infinitely many trees.
        let g = Grammar {
            alphabet: vec!['a'],
            variables: vec!["S".into()],
            start: 0,
            rules: vec![
                crate::automata::grammar::Rule { head: 0, body: vec![Sym::V(0)] },
                crate::automata::grammar::Rule { head: 0, body: vec![Sym::T('a')] },
            ],
        };
        let found = parse_trees(&g, "a", 10);
        assert_eq!(found.trees.len(), 1);
        assert!(found.cyclic);
    }

    #[test]
    fn the_limit_holds() {
        let g = expressions();
        // a p a p a p a p a: Catalan(4) = 14 trees.
        assert_eq!(parse_trees(&g, "apapapapa", 100).trees.len(), 14);
        let found = parse_trees(&g, "apapapapa", 5);
        assert_eq!(found.trees.len(), 5);
        assert!(found.more);
        assert!(to_dot(&g, &found.trees[..2]).contains("cluster_1"));
    }
}
