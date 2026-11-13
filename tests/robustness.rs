//! Kleene Star must never crash, whatever it is given. These tests feed it
//! malformed programs and check that every one ends in a clean result or a
//! clean error, never a panic or a stack overflow:
//!
//! * random sequences of tokens;
//! * every example and test program, truncated and mutated at random;
//! * unbalanced brackets, very deep nesting, huge literals and odd Unicode;
//! * imports: random import statements and qualified names, circles, and
//!   long chains of modules.
//!
//! Each program goes through everything that happens before running: the
//! lexer, parser and checker (as `kstar check` and the language server use
//! them), the editor queries, and the formatter, which must also be
//! idempotent. The inputs come from a seeded generator, so a failure can be
//! reproduced: set `KSTAR_SEED` to the seed printed with it. For a longer
//! run, see `fuzz_for_longer` below.

mod common;
use common::{files_in, project_dir, run_with_files, seed, Rng};
use kstar::ide::Analysis;
use kstar::Session;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

/// Where fuzzed programs pretend to be, so that the imports of mutated
/// golden programs (`"modules/lib.kstar"`) find real files.
fn fuzz_path() -> PathBuf {
    project_dir().join("tests/golden/fuzz.kstar")
}

/// Puts `src` through every stage short of running it. Returns a
/// description of the failure, if there was one.
fn try_survive(src: &str) -> Result<(), String> {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let analysis = Analysis::open(src, Some(&fuzz_path()), None);
        // Editor queries at a spread of positions, including ones inside a
        // character, which must not panic either.
        let step = (src.len() / 40).max(1);
        for offset in (0..=src.len() + 1).step_by(step) {
            let _ = analysis.hover(offset);
            let _ = analysis.definition(offset);
            let _ = analysis.references(offset, true);
            let _ = analysis.completions(offset);
            let _ = analysis.signature_help(offset);
        }
        // What `kstar check` prints, which renders every error.
        let mut session = Session::new("fuzz.kstar", Box::new(std::io::sink()), std::env::temp_dir(), false);
        session.set_path(&fuzz_path());
        let checked = session.check(src);
        let _ = session.take_warnings();
        // The formatter: the same parse, and it must be idempotent.
        if let Ok(once) = kstar::format_program("fuzz.kstar", src) {
            match kstar::format_program("fuzz.kstar", &once) {
                Ok(twice) if twice == once => {}
                Ok(twice) => return Err(format!("formatting is not idempotent:\n{once}\n--- then ---\n{twice}")),
                Err(e) => return Err(format!("formatted output does not parse:\n{once}\n{e}")),
            }
            // A program that formats has no syntax errors, so the checker must
            // have looked at all of it: it cannot report a syntax error.
            if let Err(message) = checked {
                // (`return` outside a function is found by the checker.)
                let parser_error = message.contains("SyntaxError[") && !message.contains("[E0110]");
                if parser_error {
                    return Err(format!("formats, but the checker reports a syntax error:\n{message}"));
                }
            }
        }
        Ok(())
    }));
    match outcome {
        Ok(result) => result,
        Err(panic) => {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            Err(format!("panicked: {message}"))
        }
    }
}

/// Asserts that `src` survives, showing it (shortened) if not. It runs on
/// a big stack, as every `kstar` command and the language server do.
fn survive(what: &str, src: &str) {
    let owned = src.to_string();
    if let Err(problem) = kstar::with_big_stack(move || try_survive(&owned)) {
        let shown: String = src.chars().take(2000).collect();
        panic!("{what}: {problem}\n--- input ({} bytes) ---\n{shown}", src.len());
    }
}

/// Words and punctuation the language uses, plus some it does not.
const VOCABULARY: &[&str] = &[
    "alphabet", "dfa", "nfa", "pda", "tm", "grammar", "over", "start", "accept", "reject", "eps", "fn", "let",
    "return", "if", "else", "for", "in", "print", "test", "draw", "trace", "on", "true", "false", "and", "or",
    "not", "accepts", "{", "}", "(", ")", "[", "]", ",", ":", "=", "==", "!=", "<", "<=", ">", ">=", "+", "-",
    "*", "%", "..", "--", "-->", "->", "|", "/", "B", "M", "S", "q0", "q1", "x", "n", "0", "1", "a", "b",
    "Z", "AZ", "_", "L", "R", "\"01\"", "\"\"", "/(0|1)*/", "/a*b/", "Int", "Str", "Set", "Dfa", "minimize",
    "union", "to_dfa", "len", "run", "derive", "#c", ";", "@", "'", "\\", "\"open", "/open", "9999999999999999999999",
    "é", "😀", "\u{200b}", "\u{0}", "\u{feff}", "\t", "\r", "import", "from", "as", ".", "std", "std.math",
    "\"modules/lib.kstar\"", "\"modules/cycle_a.kstar\"", "\"x.kstar\"", "lib", "lib.B", "lib.EvenZeros",
    "lib.double", "a.b.c",
];

/// A random sequence of tokens, with random spacing and line breaks.
fn token_soup(rng: &mut Rng) -> String {
    let mut out = String::new();
    for _ in 0..1 + rng.below(120) {
        out.push_str(rng.pick(VOCABULARY));
        out.push_str(match rng.below(10) {
            0..=5 => " ",
            6..=8 => "\n",
            _ => "",
        });
    }
    out
}

/// A random char boundary of `s`.
fn boundary(rng: &mut Rng, s: &str) -> usize {
    let mut at = rng.below(s.len() + 1);
    while !s.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// `src` with one random change: cut short, a piece deleted, duplicated or
/// moved, a token inserted, or two lines swapped.
fn mutate(rng: &mut Rng, src: &str) -> String {
    let (a, b) = {
        let (x, y) = (boundary(rng, src), boundary(rng, src));
        (x.min(y), x.max(y))
    };
    match rng.below(6) {
        0 => src[..a].to_string(),
        1 => format!("{}{}", &src[..a], &src[b..]),
        2 => format!("{}{}{}", &src[..b], &src[a..b], &src[b..]),
        3 => format!("{}{}{}", &src[..a], rng.pick(VOCABULARY), &src[a..]),
        4 => {
            let mut lines: Vec<&str> = src.lines().collect();
            if lines.len() > 1 {
                let (i, j) = (rng.below(lines.len()), rng.below(lines.len()));
                lines.swap(i, j);
            }
            lines.join("\n")
        }
        _ => format!("{}{}{}", &src[..a], &src[b..], &src[a..b]),
    }
}

/// Every program in the repository.
fn all_programs() -> Vec<(String, String)> {
    let root = project_dir();
    let mut files = files_in(&root.join("examples"), "kstar");
    files.extend(files_in(&root.join("tests/golden"), "kstar"));
    files.extend(files_in(&root.join("tests/golden/modules"), "kstar"));
    files.extend(files_in(&root.join("tests/format"), "kstar"));
    files
        .into_iter()
        .map(|f| (f.file_name().unwrap().to_string_lossy().to_string(), std::fs::read_to_string(&f).unwrap()))
        .collect()
}

fn fuzz(cases: usize, seed: u64) {
    let mut rng = Rng::new(seed);
    for case in 0..cases {
        let soup = token_soup(&mut rng);
        survive(&format!("token soup {case} (KSTAR_SEED={seed})"), &soup);
    }
    for (name, src) in all_programs() {
        let src = src.replace("\r\n", "\n");
        for case in 0..cases / 20 {
            let mut changed = mutate(&mut rng, &src);
            if rng.chance(30) {
                changed = mutate(&mut rng, &changed);
            }
            survive(&format!("{name}, mutation {case} (KSTAR_SEED={seed})"), &changed);
        }
    }
}

#[test]
fn random_and_mutated_programs_never_crash() {
    fuzz(400, seed());
}

/// The same with many more cases. Run it with
/// `cargo test --release --test robustness -- --ignored`, and with
/// `KSTAR_SEED=<n>` for different inputs each time.
#[test]
#[ignore]
fn fuzz_for_longer() {
    fuzz(20_000, seed());
}

#[test]
fn unbalanced_brackets() {
    let mut rng = Rng::new(seed());
    let brackets = ["{", "}", "(", ")", "[", "]", "/", "\"", "--", "-->"];
    for case in 0..300 {
        let mut src = String::from("alphabet B = {0, 1}\n");
        for _ in 0..rng.below(60) {
            src.push_str(rng.pick(&brackets));
            if rng.chance(20) {
                src.push_str(rng.pick(VOCABULARY));
            }
            if rng.chance(15) {
                src.push('\n');
            }
        }
        survive(&format!("brackets {case}"), &src);
    }
}

/// Programs that nest far past the limit must be refused with an error,
/// not overflow the stack; programs just inside it must work.
#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    let deep = 100_000;
    let cases = [
        format!("print {}1{}", "(".repeat(deep), ")".repeat(deep)),
        format!("print {}", "(".repeat(deep)),
        format!("print {}true", "not ".repeat(deep)),
        format!("print {}1", "- ".repeat(deep)),
        format!("print 1{}", " + 1".repeat(deep)),
        format!("print true{}", " and true".repeat(deep)),
        format!("{}print 1{}", "if true {\n".repeat(deep / 10), "}\n".repeat(deep / 10)),
        format!("{}print 1", "if true {\n".repeat(deep / 10)),
        format!("if true {{ print 1 }}{}", " else if true { print 1 }".repeat(deep / 10)),
        format!("print {}1{}", "{".repeat(deep), "}".repeat(deep)),
        format!("print len({}1{})", "len(".repeat(deep / 10), ")".repeat(deep / 10)),
        format!("let r = /{}a{}/", "(".repeat(deep), ")".repeat(deep)),
        format!("let r = /{}/", "a".repeat(deep)),
        format!("let r = /{}/", "a|".repeat(deep / 2) + "a"),
        format!("alphabet B = {{0}}\ndfa M over B {{ start q {}", "for i in 0..1 { ".repeat(deep / 10)),
    ];
    // Repeated stars collapse (`a**` is `a*`), so this one is fine.
    survive("many stars", &format!("let r = /a{}/", "*".repeat(deep)));
    // `(a+)+` is `a+`, so repeating `+` does not make the regex grow.
    survive("many pluses", &format!("let r = /{}a{}/", "(".repeat(150), ")+".repeat(150)));
    for (i, src) in cases.iter().enumerate() {
        survive(&format!("deep case {i}"), src);
        let owned = src.clone();
        let refused = kstar::with_big_stack(move || Analysis::new(&owned).errors().next().is_some());
        assert!(refused, "deep case {i} was accepted");
    }
    // Just inside the limits is fine, and still runs.
    let ok = [
        format!("print {}1{}", "(".repeat(150), ")".repeat(150)),
        format!("print 1{}", " + 1".repeat(150)),
        format!("{}print 1{}", "if true {\n".repeat(150), "}\n".repeat(150)),
        format!("print /{}/ accepts \"{}\"", "a".repeat(400), "a".repeat(400)),
    ];
    for (i, src) in ok.iter().enumerate() {
        survive(&format!("ok case {i}"), src);
        let owned = src.clone();
        let errors = kstar::with_big_stack(move || Analysis::new(&owned).errors().count());
        assert_eq!(errors, 0, "ok case {i} was refused");
        let outcome = common::run_program("deep.kstar", src);
        assert!(outcome.ok, "ok case {i} failed:\n{}", outcome.output);
    }
}

/// Programs made of random imports and qualified names, over a few small
/// files that import each other (sometimes in a circle), are checked and
/// run without crashing.
#[test]
fn random_imports_never_crash() {
    let mut rng = Rng::new(seed());
    let names = ["a", "b", "c", "lib/d", "../e", "a.b", "std.math", "std.x"];
    let members = ["X", "f", "B", "M", "g", "nope", "a", "b"];
    for case in 0..200 {
        let mut files: Vec<(String, String)> = Vec::new();
        for file in ["a", "b", "c", "lib/d"] {
            let mut text = String::from("alphabet B = {0, 1}\nlet X = 1\nfn f(n: Int) -> Int {\n    return n + X\n}\n");
            for _ in 0..rng.below(3) {
                text.insert_str(0, &format!("import \"{}.kstar\"\n", rng.pick(&names)));
            }
            if rng.chance(30) {
                text.push_str("let Y = 1 % 0\n");
            }
            files.push((format!("{file}.kstar"), text));
        }
        let mut main = String::new();
        for _ in 0..1 + rng.below(4) {
            let lib = rng.pick(&names);
            let lib = if lib.starts_with("std") { lib.to_string() } else { format!("\"{lib}.kstar\"") };
            main.push_str(&match rng.below(3) {
                0 => format!("import {lib}\n"),
                1 => format!("import {lib} as {}\n", rng.pick(&members)),
                _ => format!("from {lib} import {}, {} as z\n", rng.pick(&members), rng.pick(&members)),
            });
        }
        for _ in 0..rng.below(5) {
            let first = rng.pick(&["a", "b", "c", "d", "std", "z", "X", "std.math"]);
            main.push_str(&format!("print {first}.{}\n", rng.pick(&members)));
            if rng.chance(30) {
                main.push_str(&format!("print {first}.{}(1)\n", rng.pick(&members)));
            }
        }
        let files: Vec<(&str, &str)> = files.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        let outcome = catch_unwind(AssertUnwindSafe(|| run_with_files("main.kstar", &main, &files)));
        assert!(outcome.is_ok(), "case {case} (KSTAR_SEED={}) crashed:\n{main}", seed());
    }
}

/// A chain of modules, each importing the next, as long as a program could
/// sensibly have and much longer: loading must not overflow the stack, and
/// running a chain longer than the call limit is an error, not a crash.
#[test]
fn long_chains_of_modules() {
    for (length, should_run) in [(50, true), (2000, false)] {
        let files: Vec<(String, String)> = (0..length)
            .map(|i| {
                let text = if i + 1 < length {
                    format!("import \"m{}.kstar\"\nlet X = m{}.X + 1\n", i + 1, i + 1)
                } else {
                    "let X = 0\n".to_string()
                };
                (format!("m{i}.kstar"), text)
            })
            .collect();
        let files: Vec<(&str, &str)> = files.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        let outcome = run_with_files("main.kstar", "import \"m0.kstar\"\nprint m0.X\n", &files);
        if should_run {
            assert_eq!(outcome.output, format!("{}\n", length - 1));
        } else {
            assert!(outcome.output.contains("RecursionError"), "{}", &outcome.output[..outcome.output.len().min(500)]);
        }
    }
}

#[test]
fn huge_literals() {
    let cases = [
        format!("print {}", "9".repeat(10_000)),
        format!("print \"{}\"", "x".repeat(1_000_000)),
        format!("let {} = 1", "n".repeat(100_000)),
        format!("alphabet B = {{{}}}", vec!["0"; 10_000].join(", ")),
        format!("print {{{}}}", (0..10_000).map(|i| i.to_string()).collect::<Vec<_>>().join(", ")),
        format!("# {}\nprint 1", "comment ".repeat(100_000)),
        "\n".repeat(200_000),
    ];
    for (i, src) in cases.iter().enumerate() {
        survive(&format!("huge case {i}"), src);
    }
}

#[test]
fn odd_unicode() {
    let cases = [
        "\u{feff}alphabet B = {0, 1}\nprint B",
        "let café = 1\nprint café",
        "let 名前 = \"テスト\"\nprint 名前",
        "alphabet U = {é, 中, 😀}\nnfa M over U { start s  accept s  s --😀--> s }\nprint M accepts \"😀😀\"",
        "alphabet U = {e\u{301}}\nprint U",
        "print \"\u{202e}reversed\u{202c}\"",
        "let x\u{200b}y = 1",
        "print \"a\u{0}b\"",
        "let x = 1\r\rprint x",
        "print 1\u{2028}print 2",
        "alphabet B = {٣, ௫}\nprint B",
        "dfa M over B { start q😀 }",
        "😀😀😀",
        "\"\u{1F600}",
        "/\u{1F600}",
    ];
    for (i, src) in cases.iter().enumerate() {
        survive(&format!("unicode case {i}"), src);
        // These are small and cannot loop, so they are also run.
        let _ = common::run_program("unicode.kstar", src);
    }
}
