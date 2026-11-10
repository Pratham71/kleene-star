//! The built-in functions and their types.
//!
//! The checker asks `result_type` what a call returns; the interpreter in
//! `interp.rs` holds the code that runs each one.

use super::ast::Type;

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
];

/// What kind of argument a built-in accepts.
enum Param {
    /// Any description of a language: Dfa, Nfa, Regex, or a finite Set[Str].
    Lang,
    /// A Dfa or an Nfa.
    Machine,
    /// Something with a length: Str, Set or Alphabet.
    Sized,
    Alphabet,
}

/// True for the types that describe a language.
pub fn is_lang(ty: &Type) -> bool {
    match ty {
        Type::Dfa | Type::Nfa | Type::Regex | Type::EmptySet => true,
        Type::Set(inner) => **inner == Type::Str,
        _ => false,
    }
}

/// The type a call to built-in `name` returns for these argument types.
/// On failure returns the index of the offending argument (if it is about
/// one argument) and a message.
pub fn result_type(name: &str, args: &[Type]) -> Result<Type, (Option<usize>, String)> {
    use Param::*;
    let (params, ret): (&[Param], Type) = match name {
        "to_dfa" | "to_nfa" => {
            let ret = if name == "to_dfa" { Type::Dfa } else { Type::Nfa };
            match args.len() {
                1 => (&[Lang], ret),
                2 => (&[Lang, Alphabet], ret),
                n => {
                    return Err((None, format!("`{name}` takes 1 or 2 arguments, but {n} were given")))
                }
            }
        }
        "minimize" | "complement" => (&[Lang], Type::Dfa),
        "union" | "intersect" | "difference" => (&[Lang, Lang], Type::Dfa),
        "reduce" | "remove_eps" | "star" | "reverse" => (&[Lang], Type::Nfa),
        "concat" => (&[Lang, Lang], Type::Nfa),
        "to_regex" => (&[Lang], Type::Regex),
        "equivalent" => (&[Lang, Lang], Type::Bool),
        "is_empty" => (&[Lang], Type::Bool),
        "states" => (&[Machine], Type::Int),
        "len" => (&[Sized], Type::Int),
        "rename" => (&[Machine], args.first().cloned().unwrap_or(Type::Dfa)),
        _ => unreachable!("not a built-in: {name}"),
    };
    if args.len() != params.len() {
        let n = params.len();
        return Err((
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
            Lang => (is_lang(arg), "a language (Dfa, Nfa, Regex or Set[Str])"),
            Machine => (matches!(arg, Type::Dfa | Type::Nfa), "a machine (Dfa or Nfa)"),
            Sized => (
                matches!(arg, Type::Str | Type::Set(_) | Type::EmptySet | Type::Alphabet),
                "a Str, Set or Alphabet",
            ),
            Alphabet => (*arg == Type::Alphabet, "an Alphabet"),
        };
        if !ok {
            return Err((
                Some(i),
                format!("argument {} of `{name}` must be {wanted}, but this is {}", i + 1, article(arg)),
            ));
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
