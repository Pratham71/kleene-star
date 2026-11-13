//! `to_rust` and `kstar export` write Rust that really compiles and agrees
//! with the interpreter. Each machine's generated function is compiled with
//! `rustc` into a small program that answers for words given on its command
//! line; those answers are compared with `accepts` on random words. The
//! generated `#[cfg(test)]` module is also compiled and run with `--test`.
//!
//! Without `rustc` on the PATH the test says so and is skipped.

mod common;
use common::{seed, Rng};
use kstar::Session;
use std::path::{Path, PathBuf};
use std::process::Command;

const MACHINES: &str = r#"
alphabet B = {0, 1}
alphabet ABC = {a, b, c}

dfa EvenZeros over B {
    start  even
    accept even
    even --0--> odd
    even --1--> even
    odd  --0--> even
    odd  --1--> odd
}

fn div_by(n: Int) -> Dfa {
    return dfa over B {
        start  q[0]
        accept q[0]
        for i in 0..n {
            q[i] --0--> q[(2 * i) % n]
            q[i] --1--> q[(2 * i + 1) % n]
        }
    }
}

nfa ThirdFromEnd over B {
    start  a
    accept d
    a --0,1--> a
    a --1-->   b
    b --0,1--> c
    c --0,1--> d
}

let NoAbc = complement(to_dfa(/(a|b|c)*abc(a|b|c)*/, ABC))
let Empty = intersect(/0*/, /1(0|1)*/)
"#;

/// The machines to export: an expression, and the alphabet its words use.
const CASES: &[(&str, &str)] = &[
    ("EvenZeros", "01"),
    ("div_by(5)", "01"),
    ("ThirdFromEnd", "01"),
    ("NoAbc", "abc"),
    ("/(ab|ba)*a?/", "ab"),
    ("minimize(/(0|1)*01/)", "01"),
    ("Empty", "01"),
];

fn rustc_available() -> bool {
    Command::new("rustc").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join("kstar-export-tests");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs the machines program and then `entry`, returning what it printed.
fn kstar_print(entry: &str) -> String {
    let entry = entry.to_string();
    kstar::with_big_stack(move || {
        let out = common::Capture::default();
        let mut session = Session::new("machines.kstar", Box::new(std::io::sink()), std::env::temp_dir(), true);
        session.run(MACHINES).unwrap();
        session.set_output(Box::new(out.clone()));
        session.run(&entry).unwrap_or_else(|e| panic!("{entry}: {e}"));
        out.text()
    })
}

fn compile(source: &Path, output: &Path, test: bool) {
    let mut command = Command::new("rustc");
    command.args(["--edition", "2021", "-O", "-o"]).arg(output).arg(source);
    if test {
        command.arg("--test");
    }
    let result = command.output().unwrap();
    assert!(result.status.success(), "rustc failed on {}:\n{}", source.display(), String::from_utf8_lossy(&result.stderr));
}

#[test]
fn generated_rust_compiles_and_agrees_with_the_interpreter() {
    if !rustc_available() {
        println!("SKIPPED: `rustc` is not on the PATH, so the generated Rust cannot be compiled");
        return;
    }
    let dir = scratch();
    let mut rng = Rng::new(seed());
    for (i, (machine, symbols)) in CASES.iter().enumerate() {
        let name = format!("machine_{i}");
        let code = kstar_print(&format!("print to_rust({machine}, \"{name}\")"));
        // A program that prints 1 or 0 for each word on its command line.
        let program = format!(
            "{code}\nfn main() {{\n    for word in std::env::args().skip(1) {{\n        print!(\"{{}}\", if {name}(&word) {{ 1 }} else {{ 0 }});\n    }}\n}}\n"
        );
        let source = dir.join(format!("{name}.rs"));
        std::fs::write(&source, &program).unwrap();
        let exe = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        compile(&source, &exe, false);

        // Random words, mostly over the alphabet, a few with a foreign
        // symbol, sorted the way a Set[Str] is.
        let alphabet: Vec<char> = symbols.chars().collect();
        let mut words: Vec<String> = vec![String::new()];
        for _ in 0..300 {
            let len = rng.below(12);
            let mut word: String = (0..len).map(|_| *rng.pick(&alphabet)).collect();
            if rng.chance(5) {
                word.push('z');
            }
            words.push(word);
        }
        words.sort();
        words.dedup();
        let output = Command::new(&exe).args(&words).output().unwrap();
        let compiled = String::from_utf8(output.stdout).unwrap();

        // The interpreter's answers, in the same order. A word with a symbol
        // outside the alphabet is an error for a machine, and false for the
        // generated function.
        let (foreign, own): (Vec<&String>, Vec<&String>) =
            words.iter().partition(|w| w.chars().any(|c| !alphabet.contains(&c)));
        let quoted: Vec<String> = own.iter().map(|w| format!("{w:?}")).collect();
        let printed = kstar_print(&format!("for w in {{{}}} {{ print {machine} accepts w }}", quoted.join(", ")));
        let mut by_word: std::collections::HashMap<&str, char> =
            own.iter().zip(printed.lines()).map(|(w, line)| (w.as_str(), if line == "true" { '1' } else { '0' })).collect();
        by_word.extend(foreign.iter().map(|w| (w.as_str(), '0')));
        let expected: String = words.iter().map(|w| by_word[w.as_str()]).collect();
        assert_eq!(compiled, expected, "{machine}: the compiled function disagrees with the interpreter on {words:?}");

        // The generated tests pass.
        let tests = dir.join(format!("{name}_tests{}", std::env::consts::EXE_SUFFIX));
        compile(&source, &tests, true);
        let run = Command::new(&tests).output().unwrap();
        assert!(run.status.success(), "{machine}: generated tests failed:\n{}", String::from_utf8_lossy(&run.stdout));
    }
}
