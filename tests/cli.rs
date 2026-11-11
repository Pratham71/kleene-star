//! Tests of the `kstar` command itself: its subcommands, options and exit
//! codes. Each test works in its own scratch folder.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// A fresh, empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("kstar-cli-tests").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs `kstar` in `dir` with colour off unless asked for.
fn kstar(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kstar"))
        .args(args)
        .current_dir(dir)
        .env_remove("NO_COLOR")
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).replace("\r\n", "\n")
}

const GOOD: &str = "alphabet B = {0, 1}\nlet r = /(0|1)*1/\ntest r accepts \"01\"\n";
const FAILING: &str = "alphabet B = {0, 1}\ntest /0*/ accepts \"1\"\n";
const BROKEN: &str = "let x = 1;\nprint y\nlet z = 1 + true\n";

#[test]
fn help_for_every_command() {
    let dir = scratch("help");
    let general = stdout(&kstar(&dir, &["help"]));
    for command in ["run", "check", "fmt", "new", "trace", "explain", "repl"] {
        assert!(general.contains(command), "help does not mention {command}");
        let specific = kstar(&dir, &[command, "--help"]);
        assert!(specific.status.success());
        assert!(stdout(&specific).starts_with(&format!("Usage: kstar {command}")), "{command}");
        assert_eq!(stdout(&specific), stdout(&kstar(&dir, &["help", command])));
    }
    let unknown = kstar(&dir, &["chek", "x.kstar"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(stderr(&unknown).contains("did you mean `check`?"));
}

#[test]
fn run_several_files_and_folders() {
    let dir = scratch("run");
    std::fs::create_dir_all(dir.join("more")).unwrap();
    std::fs::write(dir.join("good.kstar"), GOOD).unwrap();
    std::fs::write(dir.join("more/also_good.kstar"), GOOD).unwrap();
    std::fs::write(dir.join("more/notes.txt"), "not a program").unwrap();
    let ok = kstar(&dir, &["run", "good.kstar", "more"]);
    let out = stdout(&ok);
    assert!(ok.status.success(), "{out}");
    assert!(out.contains("--- good.kstar\n"), "{out}");
    assert!(out.contains("--- more/also_good.kstar\n"), "{out}");
    assert!(out.contains("2 files: 2 ok, 0 failed"), "{out}");

    std::fs::write(dir.join("more/failing.kstar"), FAILING).unwrap();
    let failed = kstar(&dir, &["run", "more"]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(stdout(&failed).contains("failed: more/failing.kstar"));

    // A single file is shown without a header, as before.
    let single = stdout(&kstar(&dir, &["good.kstar"]));
    assert_eq!(single, "tests: 1 passed, 0 failed\n");

    let missing = kstar(&dir, &["run", "nowhere.kstar"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(stderr(&missing).contains("nowhere.kstar does not exist"));
}

#[test]
fn check_reports_every_error_and_warning() {
    let dir = scratch("check");
    std::fs::write(dir.join("broken.kstar"), BROKEN).unwrap();
    let broken = kstar(&dir, &["check", "broken.kstar"]);
    assert_eq!(broken.status.code(), Some(1));
    let err = stderr(&broken);
    assert!(err.contains("SyntaxError[E0104]"), "{err}");
    assert!(err.contains("TypeError[E0302]"), "{err}");
    assert!(err.contains("\n2 errors found\n"), "{err}");
    assert!(err.ends_with("(for more about an error, run `kstar explain E0104`)\n"), "{err}");

    std::fs::write(dir.join("warns.kstar"), "let unused = 1\nprint 2\n").unwrap();
    let warns = kstar(&dir, &["check", "warns.kstar"]);
    assert!(warns.status.success(), "warnings do not fail a check");
    assert!(stderr(&warns).contains("Warning[W0200]"));
    assert_eq!(stdout(&warns), "warns.kstar: no errors found (1 warning)\n");
}

#[test]
fn color_follows_the_flag_and_no_color() {
    let dir = scratch("color");
    std::fs::write(dir.join("broken.kstar"), BROKEN).unwrap();
    // Output to a pipe is plain by default...
    assert!(!stderr(&kstar(&dir, &["check", "broken.kstar"])).contains('\x1b'));
    // ...coloured when asked...
    assert!(stderr(&kstar(&dir, &["check", "--color", "always", "broken.kstar"])).contains("\x1b[1;31m"));
    assert!(stderr(&kstar(&dir, &["check", "--color=always", "broken.kstar"])).contains("\x1b[1;31m"));
    // ...and a bad value is a usage error.
    assert_eq!(kstar(&dir, &["check", "--color", "pink", "broken.kstar"]).status.code(), Some(2));
    let with_no_color = Command::new(env!("CARGO_BIN_EXE_kstar"))
        .args(["check", "broken.kstar"])
        .current_dir(&dir)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(!stderr(&with_no_color).contains('\x1b'));
}

#[test]
fn trace_calls_shows_calls_and_returns() {
    let dir = scratch("trace_calls");
    let program = "fn fact(n: Int) -> Int {\n    if n < 2 {\n        return 1\n    }\n    return n * fact(n - 1)\n}\nprint fact(3)\n";
    std::fs::write(dir.join("fact.kstar"), program).unwrap();
    let out = stdout(&kstar(&dir, &["run", "--trace-calls", "fact.kstar"]));
    assert_eq!(
        out,
        "-> fact(n=3)   line 7\n  -> fact(n=2)   line 5\n    -> fact(n=1)   line 5\n    <- fact returned 1\n  <- fact returned 2\n<- fact returned 6\n6\n"
    );
}

#[test]
fn fmt_folders_check_and_stdin() {
    let dir = scratch("fmt");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/messy.kstar"), "let x=1\nprint   x\n").unwrap();
    std::fs::write(dir.join("src/tidy.kstar"), "let x = 1\nprint x\n").unwrap();
    let check = kstar(&dir, &["fmt", "--check", "src"]);
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(stdout(&check), "src/messy.kstar: would be reformatted\n");
    assert!(kstar(&dir, &["fmt", "src"]).status.success());
    assert_eq!(std::fs::read_to_string(dir.join("src/messy.kstar")).unwrap(), "let x = 1\nprint x\n");
    assert!(kstar(&dir, &["fmt", "--check", "src"]).status.success());

    let mut child = Command::new(env!("CARGO_BIN_EXE_kstar"))
        .args(["fmt", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), b"let  y =2\n").unwrap();
    let formatted = child.wait_with_output().unwrap();
    assert!(formatted.status.success());
    assert_eq!(stdout(&formatted), "let y = 2\n");
}

#[test]
fn new_writes_a_clean_starter_once() {
    let dir = scratch("new");
    let made = kstar(&dir, &["new", "hello"]);
    assert!(made.status.success());
    assert!(stdout(&made).contains("kstar run hello.kstar"));
    let text = std::fs::read_to_string(dir.join("hello.kstar")).unwrap();
    assert!(text.starts_with("# hello: a Kleene Star program."));
    // It runs with no warnings, and is already formatted.
    let run = kstar(&dir, &["run", "hello.kstar"]);
    assert!(run.status.success());
    assert_eq!(stderr(&run), "");
    assert!(kstar(&dir, &["fmt", "--check", "hello.kstar"]).status.success());
    // Never overwrites.
    let again = kstar(&dir, &["new", "hello.kstar"]);
    assert_eq!(again.status.code(), Some(1));
    assert!(stderr(&again).contains("already exists"));
}

// Regression: `kstar fmt` ran on the main thread, whose stack (1 MB on
// Windows) was too small for programs near the nesting limit.
#[test]
fn fmt_handles_deep_programs() {
    let dir = scratch("fmt_deep");
    let deep = format!("print {}1{}\n", "(".repeat(190), ")".repeat(190));
    std::fs::write(dir.join("deep.kstar"), &deep).unwrap();
    let out = kstar(&dir, &["fmt", "--check", "deep.kstar"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(kstar(&dir, &["check", "deep.kstar"]).status.success());
}

#[test]
fn trace_runs_the_file_quietly_then_traces() {
    let dir = scratch("trace");
    let program = "alphabet B = {0, 1}\ndfa M over B { start a  accept b  a --0--> a  a --1--> b  b --0,1--> b }\nprint \"not shown\"\ntest M accepts \"1\"\n";
    std::fs::write(dir.join("m.kstar"), program).unwrap();
    let out = kstar(&dir, &["trace", "m.kstar", "M", "01"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.starts_with("Input: 01\n"), "{text}");
    assert!(text.contains("Result: ACCEPTED"), "{text}");
    assert!(!text.contains("not shown") && !text.contains("tests:"), "{text}");
    // Any expression works as the machine, and the word may be empty.
    assert!(kstar(&dir, &["trace", "m.kstar", "minimize(M)", ""]).status.success());
    let missing = kstar(&dir, &["trace", "m.kstar", "Nope", "0"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(stderr(&missing).contains("`Nope` is not defined"));
    assert_eq!(kstar(&dir, &["trace", "m.kstar"]).status.code(), Some(2));
}

#[test]
fn explain_prints_the_documentation() {
    let dir = scratch("explain");
    let out = kstar(&dir, &["explain", "e0200"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.starts_with("E0200: A name that is not defined\n"), "{text}");
    assert!(text.contains("    print len(alphabet_b)"), "examples are indented, without fences:\n{text}");
    assert!(!text.contains("```"));
    let list = stdout(&kstar(&dir, &["explain", "--list"]));
    assert!(list.lines().count() > 40);
    assert!(list.contains("W0300: A test that always gives the same answer"));
    assert_eq!(kstar(&dir, &["explain", "E9999"]).status.code(), Some(1));
}
