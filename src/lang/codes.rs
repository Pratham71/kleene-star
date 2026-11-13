//! Stable codes for every kind of error and warning, and their long
//! explanations.
//!
//! Each diagnostic carries a code such as `E0200`. The codes never change
//! meaning, so they can be searched for and written down. `kstar explain
//! E0200` prints the explanation, which lives in `docs/errors.md`: that page
//! is both the documentation and the text the command shows, and the tests
//! run every example in it.
//!
//! Numbering: `E01..` syntax, `E02..` names, `E03..` types, `E04..` machines
//! and grammars, `E05..` alphabets, `E06..` errors while running, and `W....`
//! for warnings.

// ---- syntax ----
pub const SYNTAX: &str = "E0100";
pub const UNCLOSED: &str = "E0101";
pub const NEEDS_LET: &str = "E0102";
pub const KEYWORD_AS_NAME: &str = "E0103";
pub const BAD_CHARACTER: &str = "E0104";
pub const BAD_REGEX: &str = "E0105";
pub const NESTED_FUNCTION: &str = "E0106";
pub const BAD_EPS: &str = "E0107";
pub const BAD_SYMBOL: &str = "E0108";
pub const NEEDS_NAME: &str = "E0109";
pub const RETURN_OUTSIDE: &str = "E0110";
pub const WRONG_MACHINE_KIND: &str = "E0111";
pub const FORMAT_UNSAFE: &str = "E0112";
pub const TOO_DEEP: &str = "E0113";
pub const IMPORT_NOT_TOP: &str = "E0114";
pub const BAD_IMPORT_PATH: &str = "E0115";

// ---- names ----
pub const UNDEFINED: &str = "E0200";
pub const ALREADY_DEFINED: &str = "E0201";
pub const UNKNOWN_FUNCTION: &str = "E0202";
pub const BUILTIN_REDEFINED: &str = "E0203";
pub const USED_TOO_EARLY: &str = "E0204";
pub const VARIABLE_WITHOUT_RULES: &str = "E0205";
pub const IMPORT_NOT_FOUND: &str = "E0206";
pub const IMPORT_CYCLE: &str = "E0207";
pub const UNKNOWN_MEMBER: &str = "E0208";
pub const IMPORT_CLASH: &str = "E0209";
pub const REEXPORT: &str = "E0210";
pub const MODULE_AS_VALUE: &str = "E0211";

// ---- types ----
pub const TYPE_MISMATCH: &str = "E0300";
pub const ARGUMENT_COUNT: &str = "E0301";
pub const OPERATOR_TYPES: &str = "E0302";
pub const UNKNOWN_TYPE: &str = "E0303";
pub const MISSING_RETURN: &str = "E0304";
pub const BAD_SET: &str = "E0305";
pub const UNDECIDABLE: &str = "E0306";
pub const NOT_COMPARABLE: &str = "E0307";

// ---- machines and grammars ----
pub const BAD_MACHINE: &str = "E0400";
pub const NO_START: &str = "E0401";
pub const TWO_STARTS: &str = "E0402";
pub const EPS_IN_DFA: &str = "E0403";
pub const NOT_TOTAL: &str = "E0404";
pub const NOT_DETERMINISTIC: &str = "E0405";
pub const BLANK_IN_ALPHABET: &str = "E0406";
pub const ACCEPT_AND_REJECT: &str = "E0407";
pub const NO_RULES: &str = "E0408";
pub const NEGATIVE_INDEX: &str = "E0409";

// ---- alphabets ----
pub const NOT_IN_ALPHABET: &str = "E0500";
pub const FOREIGN_WORD: &str = "E0501";
pub const ALPHABET_MISMATCH: &str = "E0502";
pub const DUPLICATE_SYMBOL: &str = "E0503";

// ---- while running ----
pub const OVERFLOW: &str = "E0600";
pub const MODULO_ZERO: &str = "E0601";
pub const RANGE_TOO_LARGE: &str = "E0602";
pub const RECURSION: &str = "E0603";
pub const STEP_LIMIT: &str = "E0604";
pub const GAVE_UP: &str = "E0605";
pub const NO_DERIVATION: &str = "E0606";
pub const IO: &str = "E0607";
pub const TOO_LARGE: &str = "E0608";

// ---- warnings ----
pub const UNREACHABLE_STATE: &str = "W0100";
pub const DEAD_STATE: &str = "W0101";
pub const UNUSED: &str = "W0200";
pub const UNUSED_FUNCTION: &str = "W0201";
pub const UNUSED_IMPORT: &str = "W0202";
pub const CONSTANT_TEST: &str = "W0300";

/// Every code, in order.
pub const ALL: &[&str] = &[
    SYNTAX, UNCLOSED, NEEDS_LET, KEYWORD_AS_NAME, BAD_CHARACTER, BAD_REGEX, NESTED_FUNCTION, BAD_EPS,
    BAD_SYMBOL, NEEDS_NAME, RETURN_OUTSIDE, WRONG_MACHINE_KIND, FORMAT_UNSAFE, TOO_DEEP, IMPORT_NOT_TOP,
    BAD_IMPORT_PATH,
    UNDEFINED, ALREADY_DEFINED, UNKNOWN_FUNCTION, BUILTIN_REDEFINED, USED_TOO_EARLY, VARIABLE_WITHOUT_RULES,
    IMPORT_NOT_FOUND, IMPORT_CYCLE, UNKNOWN_MEMBER, IMPORT_CLASH, REEXPORT, MODULE_AS_VALUE,
    TYPE_MISMATCH, ARGUMENT_COUNT, OPERATOR_TYPES, UNKNOWN_TYPE, MISSING_RETURN, BAD_SET, UNDECIDABLE,
    NOT_COMPARABLE,
    BAD_MACHINE, NO_START, TWO_STARTS, EPS_IN_DFA, NOT_TOTAL, NOT_DETERMINISTIC, BLANK_IN_ALPHABET,
    ACCEPT_AND_REJECT, NO_RULES, NEGATIVE_INDEX,
    NOT_IN_ALPHABET, FOREIGN_WORD, ALPHABET_MISMATCH, DUPLICATE_SYMBOL,
    OVERFLOW, MODULO_ZERO, RANGE_TOO_LARGE, RECURSION, STEP_LIMIT, GAVE_UP, NO_DERIVATION, IO, TOO_LARGE,
    UNREACHABLE_STATE, DEAD_STATE, UNUSED, UNUSED_FUNCTION, UNUSED_IMPORT, CONSTANT_TEST,
];

/// The explanations: one `## CODE: title` section per code.
const PAGE: &str = include_str!("../../docs/errors.md");

/// The explanation of `code` (in any letter case), as Markdown-ish text
/// ready for a terminal: the section from its heading to the next one.
pub fn explain(code: &str) -> Option<String> {
    let code = code.trim().to_ascii_uppercase();
    let heading = format!("## {code}:");
    let start = PAGE.find(&heading)?;
    let rest = &PAGE[start + 3..];
    let end = rest.find("\n## ").map_or(rest.len(), |i| i + 1);
    Some(rest[..end].trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for code in ALL {
            assert!(seen.insert(code), "{code} is listed twice");
            assert_eq!(code.len(), 5, "{code}");
            assert!(code.starts_with('E') || code.starts_with('W'), "{code}");
        }
    }

    #[test]
    fn every_code_is_explained() {
        for code in ALL {
            let text = explain(code).unwrap_or_else(|| panic!("docs/errors.md has no section for {code}"));
            assert!(text.lines().count() > 2, "the explanation of {code} is too short");
        }
        assert!(explain("e0200").unwrap().starts_with("E0200:"));
        assert_eq!(explain("E9999"), None);
    }

    #[test]
    fn every_explained_code_exists() {
        for line in PAGE.lines().filter(|l| l.starts_with("## ")) {
            let code = &line[3..8];
            assert!(ALL.contains(&code), "docs/errors.md explains {code}, which is not a code");
        }
    }
}
