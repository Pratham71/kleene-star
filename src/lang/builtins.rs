//! The built-in functions and their types.
//!
//! The checker asks `result_type` what a call returns; the interpreter in
//! `interp.rs` holds the code that runs each one.

use super::ast::Type;
use super::codes;

pub const NAMES: &[&str] = &[
    "to_dfa",
    "to_nfa",
    "to_regex",
    "minimize",
    "reduce",
    "remove_eps",
    "rename",
    "union",
    "intersect",
    "difference",
    "complement",
    "concat",
    "star",
    "reverse",
    "equivalent",
    "is_empty",
    "states",
    "len",
    "to_cnf",
    "to_pda",
    "to_grammar",
    "derive",
    "run",
    "to_rust",
];

/// What kind of argument a built-in accepts.
enum Param {
    /// Any description of a regular language: Dfa, Nfa, Regex, or a finite Set[Str].
    Lang,
    /// A regular language or a Grammar.
    LangOrGrammar,
    /// A Dfa or an Nfa.
    Machine,
    /// A Dfa, an Nfa, a Pda or a Tm.
    AnyMachine,
    Tm,
    /// Something with a length: Str, Set or Alphabet.
    Sized,
    Alphabet,
    Grammar,
    Str,
}

/// True for the types that describe a regular language.
pub fn is_lang(ty: &Type) -> bool {
    match ty {
        Type::Dfa | Type::Nfa | Type::Regex | Type::EmptySet => true,
        Type::Set(inner) => **inner == Type::Str,
        _ => false,
    }
}

/// True for the types that describe a context-free language that may not
/// be regular.
pub fn is_context_free(ty: &Type) -> bool {
    matches!(ty, Type::Grammar | Type::Pda)
}

/// Why a call does not type-check: the offending argument (if the problem
/// is about one argument), a message, and perhaps a hint.
pub struct CallError {
    pub arg: Option<usize>,
    pub code: &'static str,
    pub message: String,
    pub hint: Option<String>,
}

/// A problem with argument `arg`, or with the number of arguments if `arg`
/// is `None`.
fn error(arg: Option<usize>, message: String) -> CallError {
    let code = if arg.is_some() { codes::TYPE_MISMATCH } else { codes::ARGUMENT_COUNT };
    CallError { arg, code, message, hint: None }
}

/// The type a call to built-in `name` returns for these argument types.
pub fn result_type(name: &str, args: &[Type]) -> Result<Type, CallError> {
    use Param::*;
    if name == "equivalent" {
        if let Some(i) = args.iter().position(|ty| *ty == Type::Tm) {
            return Err(CallError {
                arg: Some(i),
                code: codes::UNDECIDABLE,
                message: "`equivalent` cannot compare Turing machines".into(),
                hint: Some(
                    "whether two Turing machines accept the same language is undecidable (by Rice's theorem, no program can decide any such question about every machine). Compare them on sample words with `accepts` instead"
                        .into(),
                ),
            });
        }
        if let Some(i) = args.iter().position(is_context_free) {
            return Err(CallError {
                arg: Some(i),
                code: codes::UNDECIDABLE,
                message: format!(
                    "`equivalent` cannot compare context-free languages, and this is {}",
                    article(&args[i])
                ),
                hint: Some(
                    "whether two grammars or PDAs describe the same language is undecidable: no algorithm can answer it for every pair. Compare them on sample words with `accepts` instead"
                        .into(),
                ),
            });
        }
    }
    let (params, ret): (&[Param], Type) = match name {
        "to_dfa" | "to_nfa" => {
            let ret = if name == "to_dfa" { Type::Dfa } else { Type::Nfa };
            match args.len() {
                1 => (&[Lang], ret),
                2 => (&[Lang, Alphabet], ret),
                n => return Err(error(None, format!("`{name}` takes 1 or 2 arguments, but {n} were given"))),
            }
        }
        "minimize" | "complement" => (&[Lang], Type::Dfa),
        "union" | "intersect" | "difference" => (&[Lang, Lang], Type::Dfa),
        "reduce" | "remove_eps" | "star" | "reverse" => (&[Lang], Type::Nfa),
        "concat" => (&[Lang, Lang], Type::Nfa),
        "to_regex" => (&[Lang], Type::Regex),
        "equivalent" => (&[Lang, Lang], Type::Bool),
        "is_empty" => (&[LangOrGrammar], Type::Bool),
        "states" => (&[AnyMachine], Type::Int),
        "len" => (&[Sized], Type::Int),
        "rename" => (&[Machine], args.first().cloned().unwrap_or(Type::Dfa)),
        "to_cnf" => (&[Grammar], Type::Grammar),
        "to_pda" => (&[Grammar], Type::Pda),
        "to_grammar" => (&[Lang], Type::Grammar),
        "derive" => (&[Grammar, Str], Type::Str),
        "run" => (&[Tm, Str], Type::Str),
        "to_rust" => (&[Lang, Str], Type::Str),
        _ => unreachable!("not a built-in: {name}"),
    };
    if args.len() != params.len() {
        let n = params.len();
        return Err(error(
            None,
            format!(
                "`{name}` takes {n} argument{}, but {} {} given",
                if n == 1 { "" } else { "s" },
                args.len(),
                if args.len() == 1 { "was" } else { "were" }
            ),
        ));
    }
    for (i, (param, arg)) in params.iter().zip(args).enumerate() {
        let (ok, wanted) = match param {
            Lang => (is_lang(arg), "a regular language (Dfa, Nfa, Regex or Set[Str])"),
            LangOrGrammar => (is_lang(arg) || *arg == Type::Grammar, "a Grammar or a regular language"),
            Machine => (matches!(arg, Type::Dfa | Type::Nfa), "a machine (Dfa or Nfa)"),
            AnyMachine => (matches!(arg, Type::Dfa | Type::Nfa | Type::Pda | Type::Tm), "a machine (Dfa, Nfa, Pda or Tm)"),
            Tm => (*arg == Type::Tm, "a Turing machine (Tm)"),
            Sized => (
                matches!(arg, Type::Str | Type::Set(_) | Type::EmptySet | Type::Alphabet),
                "a Str, Set or Alphabet",
            ),
            Alphabet => (*arg == Type::Alphabet, "an Alphabet"),
            Grammar => (*arg == Type::Grammar, "a Grammar"),
            Str => (*arg == Type::Str, "a Str"),
        };
        if !ok {
            let mut err = error(
                Some(i),
                format!("argument {} of `{name}` must be {wanted}, but this is {}", i + 1, article(arg)),
            );
            if name == "to_rust" && i == 0 && (is_context_free(arg) || *arg == Type::Tm) {
                err.hint = Some(
                    match arg {
                        Type::Grammar => "the Rust code is a DFA's fixed table of states, and a grammar's language may not be regular (like 0^n 1^n), which no finite table can recognise",
                        Type::Pda => "the Rust code is a DFA's fixed table of states, and a PDA's stack can grow without limit, so it has no such table",
                        _ => "the Rust code is a DFA's fixed table of states, and a Turing machine's tape can grow without limit (and it may never halt), so it has no such table",
                    }
                    .to_string(),
                );
            } else if matches!(param, Lang) && (is_context_free(arg) || *arg == Type::Tm) {
                err.hint = Some(format!(
                    "`{name}` works on regular languages only; {} can describe a language that is not regular",
                    article(arg)
                ));
            }
            if matches!(param, Grammar) && *arg == Type::Pda {
                err.hint = Some("only grammars can be converted this way; there is no built-in that turns a PDA back into a grammar".into());
            }
            return Err(err);
        }
    }
    Ok(ret)
}

/// "an Int", "a Dfa": a type name with the right article.
pub fn article(ty: &Type) -> String {
    let name = ty.to_string();
    let an = name.starts_with(['A', 'E', 'I', 'O', 'U']);
    format!("{} {name}", if an { "an" } else { "a" })
}

/// What editor tools show for a built-in: its signature, the names of its
/// parameters (for signature help), and a sentence about what it does.
pub struct BuiltinDoc {
    pub signature: &'static str,
    pub params: &'static [&'static str],
    pub doc: &'static str,
}

/// The documentation of built-in `name`, if it is one.
pub fn doc(name: &str) -> Option<BuiltinDoc> {
    let (signature, params, doc): (&str, &[&str], &str) = match name {
        "to_dfa" => ("to_dfa(lang, alphabet?) -> Dfa", &["lang", "alphabet"], "The subset construction: a DFA for the language. States are named after sets, like `{a,b}`."),
        "to_nfa" => ("to_nfa(lang, alphabet?) -> Nfa", &["lang", "alphabet"], "An NFA for the language; a regex becomes an eps-NFA by Thompson's construction."),
        "to_regex" => ("to_regex(lang) -> Regex", &["lang"], "A regular expression for the language, by state elimination."),
        "minimize" => ("minimize(lang) -> Dfa", &["lang"], "The unique smallest DFA for the language. Merged states are named `q1+q2`."),
        "reduce" => ("reduce(lang) -> Nfa", &["lang"], "A smaller NFA: useless states removed and equivalent ones merged (not guaranteed smallest)."),
        "remove_eps" => ("remove_eps(lang) -> Nfa", &["lang"], "The same language as an NFA without eps moves."),
        "rename" => ("rename(machine) -> same type", &["machine"], "Renames the states `q0, q1, ...` outward from the start."),
        "union" => ("union(a, b) -> Dfa", &["a", "b"], "Strings in `a` or `b` (product construction)."),
        "intersect" => ("intersect(a, b) -> Dfa", &["a", "b"], "Strings in both `a` and `b` (product construction)."),
        "difference" => ("difference(a, b) -> Dfa", &["a", "b"], "Strings in `a` but not in `b`."),
        "complement" => ("complement(lang) -> Dfa", &["lang"], "Strings over the alphabet that are not in the language."),
        "concat" => ("concat(a, b) -> Nfa", &["a", "b"], "A string of `a` followed by a string of `b`."),
        "star" => ("star(lang) -> Nfa", &["lang"], "Zero or more strings of the language joined together."),
        "reverse" => ("reverse(lang) -> Nfa", &["lang"], "The strings of the language written backwards."),
        "equivalent" => ("equivalent(a, b) -> Bool", &["a", "b"], "Do two regular languages contain exactly the same strings? (Undecidable for grammars, PDAs and Turing machines.)"),
        "is_empty" => ("is_empty(lang) -> Bool", &["lang"], "Does the language (or grammar) contain no strings at all?"),
        "states" => ("states(machine) -> Int", &["machine"], "The number of states of a Dfa, Nfa, Pda or Tm."),
        "len" => ("len(x) -> Int", &["x"], "The length of a Str, or the size of a Set or Alphabet."),
        "to_cnf" => ("to_cnf(grammar) -> Grammar", &["grammar"], "An equivalent grammar in Chomsky normal form."),
        "to_pda" => ("to_pda(grammar) -> Pda", &["grammar"], "The three-state PDA that simulates leftmost derivations of the grammar."),
        "to_grammar" => ("to_grammar(lang) -> Grammar", &["lang"], "A right-linear grammar for a regular language."),
        "derive" => ("derive(grammar, word) -> Str", &["grammar", "word"], "A shortest leftmost derivation of the word, one sentential form per line."),
        "run" => ("run(tm, word) -> Str", &["tm", "word"], "Runs a Turing machine until it halts and returns what is left on the tape."),
        "to_rust" => ("to_rust(lang, fn_name) -> Str", &["lang", "fn_name"], "Rust source for `pub fn fn_name(input: &str) -> bool`, a dependency-free matcher made from the language's DFA, with tests."),
        _ => return None,
    };
    Some(BuiltinDoc { signature, params, doc })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_builtin_is_documented() {
        for name in super::NAMES {
            let doc = super::doc(name).unwrap_or_else(|| panic!("`{name}` has no documentation"));
            assert!(doc.signature.starts_with(&format!("{name}(")));
        }
    }
}
