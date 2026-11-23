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
    for command in ["run", "check", "fmt", "new", "trace", "export", "explain", "repl", "update", "version"] {
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

/// The string literals in `src` that look like options: `"--out"`, `"-y"`.
fn option_literals(src: &str) -> Vec<String> {
    src.split('"')
        .skip(1)
        .step_by(2)
        .filter(|s| {
            let body = s.trim_start_matches('-');
            let dashes = s.len() - body.len();
            (dashes == 2 || (dashes == 1 && body.len() == 1))
                && !body.is_empty()
                && body.trim_end_matches('=').chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        .map(|s| s.trim_end_matches('=').to_string())
        .collect()
}

#[test]
fn every_option_the_parsers_take_is_in_the_help() {
    use kstar::help::{COMMANDS, GLOBAL_FLAGS};
    let documented: Vec<&str> =
        COMMANDS.iter().flat_map(|c| c.flags.iter()).chain(GLOBAL_FLAGS).flat_map(|f| f.spellings()).collect();
    // Every option spelled out in the command line code. In update.rs only
    // `kstar update`'s own parser counts: the rest are arguments to VS
    // Code's `code` command.
    let main = include_str!("../src/main.rs");
    let update = include_str!("../src/update.rs");
    let start = update.find("pub fn update(").unwrap();
    let update = &update[start..start + update[start..].find("\n}\n").unwrap()];
    for option in option_literals(main).iter().chain(&option_literals(update)) {
        assert!(documented.contains(&option.as_str()), "`{option}` is accepted but not in the help");
    }
    // Every command main.rs dispatches on is in the help.
    let start = main.find("    match command {").unwrap();
    let dispatch = &main[start..start + main[start..].find("\n    }\n").unwrap()];
    for line in dispatch.lines().filter(|l| l.trim_start().starts_with('"') && l.contains("=>")) {
        let name = line.trim_start().split('"').nth(1).unwrap();
        if !name.starts_with('-') {
            assert!(kstar::help::command(name).is_some(), "`kstar {name}` is not in the help");
        }
    }

    // And the other way: each command takes exactly the options its help
    // lists. (`update` asks GitHub, so only its source is checked, above.)
    let dir = scratch("options");
    let all: Vec<&kstar::help::Flag> = COMMANDS.iter().flat_map(|c| c.flags.iter()).collect();
    for command in COMMANDS.iter().filter(|c| !matches!(c.name, "update" | "help" | "kstar-lsp")) {
        let page = stdout(&kstar(&dir, &[command.name, "--help"]));
        for flag in all.iter().filter(|f| f.spellings().iter().all(|s| s.starts_with("--"))) {
            let spelling = flag.spellings()[0];
            let listed = command.flags.iter().any(|f| f.spellings().contains(&spelling));
            let mut args = vec![command.name, spelling];
            if flag.takes_value() {
                args.push(match spelling {
                    "--color" => "never",
                    "--theme" => "light",
                    _ => "x",
                });
            }
            let output = kstar(&dir, &args);
            let err = stderr(&output);
            let refused = err.contains(&format!("unknown option `{spelling}`")) || err.contains("takes no arguments");
            assert_eq!(!refused, listed, "kstar {}: {err}", args.join(" "));
            if command.flags.iter().any(|f| f.names == flag.names) {
                assert!(page.contains(flag.names), "`kstar {} --help` does not show {}", command.name, flag.names);
            }
        }
    }
    // `--name=value`, and a value on an option that takes none.
    assert!(!stderr(&kstar(&dir, &["run", "--out=x"])).contains("unknown option"));
    assert!(stderr(&kstar(&dir, &["run", "--watch=1", "x.kstar"])).contains("`--watch` takes no value"));
    assert!(stderr(&kstar(&dir, &["run", "--nope=1"])).contains("unknown option `--nope`"));
    // The short spellings, which the loop above leaves out.
    assert!(!stderr(&kstar(&dir, &["export", "-o"])).contains("unknown option"));
    assert!(stderr(&kstar(&dir, &["run", "-o", "x"])).contains("unknown option `-o`"));
}

#[test]
fn help_topics() {
    let dir = scratch("help-topics");
    let overview = stdout(&kstar(&dir, &["help"]));
    for heading in kstar::help::GROUPS {
        assert!(overview.contains(&format!("{heading}:\n")), "{heading}");
    }
    assert_eq!(stdout(&kstar(&dir, &[])), overview);
    let modules = stdout(&kstar(&dir, &["help", "std"]));
    assert!(modules.contains("std.automata"));
    let module = kstar(&dir, &["help", "std.collections"]);
    assert!(module.status.success());
    assert!(stdout(&module).contains("fn power_set(s: Set[T]) -> Set[Set[T]]"));
    let unknown = kstar(&dir, &["help", "std.colections"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(stderr(&unknown).contains("did you mean `std.collections`?"));
    // No colour when the output is not a terminal, as here.
    assert!(!overview.contains('\x1b'));
    // kstar-lsp has the same page.
    let lsp = Command::new(env!("CARGO_BIN_EXE_kstar-lsp")).arg("--help").output().unwrap();
    assert_eq!(stdout(&lsp), stdout(&kstar(&dir, &["help", "kstar-lsp"])).replace('\x1b', ""));
    let version = Command::new(env!("CARGO_BIN_EXE_kstar-lsp")).arg("--version").output().unwrap();
    assert_eq!(stdout(&version).trim(), format!("kstar-lsp {}", env!("CARGO_PKG_VERSION")));
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

#[test]
fn imports_lazy_eager_check_and_folders() {
    let dir = scratch("imports");
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::write(dir.join("lib/top.kstar"), "let Zero = 0\nlet Broken = 1 % Zero\nlet Fine = 1\n").unwrap();
    std::fs::write(
        dir.join("main.kstar"),
        "import \"lib/top.kstar\"\nprint \"start\"\nprint top.Fine\n",
    )
    .unwrap();

    // Lazy: the module runs at its first use, on line 3.
    let lazy = kstar(&dir, &["run", "main.kstar"]);
    assert_eq!(lazy.status.code(), Some(1));
    assert_eq!(stdout(&lazy), "start\n");
    let err = stderr(&lazy);
    assert!(err.contains("main.kstar, line 3, in <main>\n    print top.Fine"), "{err}");
    assert!(err.contains("lib/top.kstar, line 2, in <module>"), "{err}");

    // Eager: it runs at the import, on line 1, before anything is printed.
    let eager = kstar(&dir, &["run", "--eager-imports", "main.kstar"]);
    assert_eq!(eager.status.code(), Some(1));
    assert_eq!(stdout(&eager), "");
    assert!(stderr(&eager).contains("main.kstar, line 1, in <main>\n    import \"lib/top.kstar\""), "{}", stderr(&eager));

    // `check` reports a type error in a module that is never used.
    std::fs::write(dir.join("lib/bad.kstar"), "let Size = 1 + \"two\"\n").unwrap();
    std::fs::write(dir.join("uses_bad.kstar"), "import \"lib/bad.kstar\"\nprint 1\n").unwrap();
    let check = kstar(&dir, &["check", "uses_bad.kstar"]);
    assert_eq!(check.status.code(), Some(1));
    assert!(stderr(&check).contains("Error in lib/bad.kstar, line 1:"), "{}", stderr(&check));

    // A folder of files that import each other still runs file by file,
    // and `kstar trace` finds the imports of the file it runs.
    std::fs::write(dir.join("lib/ok.kstar"), "alphabet B = {0, 1}\ndfa M over B { start a  accept a  a --0,1--> a }\n").unwrap();
    std::fs::write(dir.join("lib/user.kstar"), "import \"ok.kstar\"\nprint ok.M accepts \"01\"\n").unwrap();
    let folder = kstar(&dir, &["run", "lib"]);
    let out = stdout(&folder);
    assert!(out.contains("--- lib/user.kstar\ntrue\n"), "{out}");
    let trace = kstar(&dir, &["trace", "lib/user.kstar", "ok.M", "01"]);
    assert!(trace.status.success(), "{}", stderr(&trace));
    assert!(stdout(&trace).contains("Result: ACCEPTED"), "{}", stdout(&trace));
}

#[test]
fn export_writes_rust() {
    let dir = scratch("export");
    std::fs::write(
        dir.join("m.kstar"),
        "alphabet B = {0, 1}\ndfa EvenZeros over B { start e  accept e  e --0--> o  e --1--> e  o --0--> e  o --1--> o }\nprint \"not shown\"\n",
    )
    .unwrap();
    // By default it writes <Machine>.rs next to the program and says where.
    let out = kstar(&dir, &["export", "m.kstar", "EvenZeros"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let path = dir.join("EvenZeros.rs");
    let code = std::fs::read_to_string(&path).unwrap();
    assert!(code.starts_with("// Generated by Kleene Star from `EvenZeros`."), "{code}");
    assert!(code.contains("pub fn even_zeros(input: &str) -> bool {"), "{code}");
    assert!(!code.contains("not shown") && !stdout(&out).contains("not shown"));
    let said = stdout(&out);
    assert!(said.starts_with("Wrote ") && said.contains("EvenZeros.rs (Rust, "), "{said}");
    assert!(Path::new(said.trim_start_matches("Wrote ").split(" (").next().unwrap()).is_absolute(), "{said}");
    assert!(said.trim_end().ends_with(&format!("{} lines)", code.lines().count())), "{said}");
    // It does not replace the file unless asked to.
    std::fs::write(&path, "mine").unwrap();
    let again = kstar(&dir, &["export", "m.kstar", "EvenZeros"]);
    assert_eq!(again.status.code(), Some(1));
    assert!(stderr(&again).contains("--force"), "{}", stderr(&again));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine");
    let forced = kstar(&dir, &["export", "--force", "m.kstar", "EvenZeros"]);
    assert!(forced.status.success(), "{}", stderr(&forced));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), code);
    // `-o -` prints the code, as before; `-o folder/` writes into it.
    let printed = kstar(&dir, &["export", "-o", "-", "m.kstar", "EvenZeros"]);
    assert_eq!(stdout(&printed), code);
    let folder = kstar(&dir, &["export", "-o", "gen/", "m.kstar", "EvenZeros"]);
    assert!(folder.status.success(), "{}", stderr(&folder));
    assert_eq!(std::fs::read_to_string(dir.join("gen").join("EvenZeros.rs")).unwrap(), code);

    let written = kstar(&dir, &["export", "--minimize", "--fn", "parity", "-o", "parity.rs", "m.kstar", "EvenZeros"]);
    assert!(written.status.success(), "{}", stderr(&written));
    let file = std::fs::read_to_string(dir.join("parity.rs")).unwrap();
    assert!(file.contains("pub fn parity(input: &str) -> bool {") && file.contains("minimize(EvenZeros)"), "{file}");

    let bad = kstar(&dir, &["export", "-o", "-", "m.kstar", "EvenZeros", "--fn", "fn"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(stderr(&bad).contains("E0308"), "{}", stderr(&bad));
    let usage = kstar(&dir, &["export", "m.kstar"]);
    assert_eq!(usage.status.code(), Some(2));
}

#[test]
fn export_writes_json_that_reads_back() {
    let dir = scratch("export-json");
    std::fs::write(
        dir.join("m.kstar"),
        "alphabet B = {0, 1}\ndfa EvenZeros over B { start e  accept e  e --0--> o  e --1--> e  o --0--> e  o --1--> o }\ngrammar G over B { S -> 0 S 1 | eps }\n",
    )
    .unwrap();
    let out = kstar(&dir, &["export", "--format", "json", "-o", "-", "m.kstar", "EvenZeros"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.starts_with("{\n  \"format\": \"kleene-star\",\n  \"version\": 1,\n  \"kind\": \"dfa\",\n  \"name\": \"EvenZeros\","), "{text}");
    assert!(text.contains("\"states\": [\"e\", \"o\"]"), "the state names are kept: {text}");
    // Any machine, regex or grammar can be written, not only DFAs.
    let g = kstar(&dir, &["export", "--format", "json", "-o", "g.json", "m.kstar", "G"]);
    assert!(g.status.success(), "{}", stderr(&g));
    let file = std::fs::read_to_string(dir.join("g.json")).unwrap();
    assert!(file.contains("\"kind\": \"grammar\"") && file.contains("\"name\": \"G\""), "{file}");
    // And read back by a program.
    // A string cannot span lines, but JSON does not mind being on one line.
    let one_line = text.replace('\n', " ").replace('\\', "\\\\").replace('"', "\\\"");
    std::fs::write(dir.join("back.kstar"), format!("let D = from_json(Dfa, \"{one_line}\")\nprint states(D), D accepts \"00\"\n")).unwrap();
    let back = kstar(&dir, &["run", "back.kstar"]);
    assert!(back.status.success(), "{}", stderr(&back));
    assert_eq!(stdout(&back), "2 true\n");
    // `--fn` belongs to Rust output; an unknown format is a usage error.
    let fn_json = kstar(&dir, &["export", "--format", "json", "--fn", "x", "m.kstar", "EvenZeros"]);
    assert_eq!(fn_json.status.code(), Some(2));
    let other = kstar(&dir, &["export", "--format", "yaml", "m.kstar", "EvenZeros"]);
    assert_eq!(other.status.code(), Some(2));
    assert!(stderr(&other).contains("rust") && stderr(&other).contains("json"), "{}", stderr(&other));
    // By default, next to the program.
    let file = kstar(&dir, &["export", "--format", "json", "m.kstar", "EvenZeros"]);
    assert!(stdout(&file).contains("EvenZeros.json (JSON, "), "{}", stdout(&file));
    assert_eq!(std::fs::read_to_string(dir.join("EvenZeros.json")).unwrap().replace("\r\n", "\n"), text);
}

#[test]
fn export_writes_python_that_checks_itself() {
    let dir = scratch("export-python");
    std::fs::write(
        dir.join("m.kstar"),
        "alphabet B = {0, 1}
dfa EvenZeros over B { start e  accept e  e --0--> o  e --1--> e  o --0--> e  o --1--> o }
",
    )
    .unwrap();
    let out = kstar(&dir, &["export", "--format", "python", "m.kstar", "EvenZeros"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("EvenZeros.py (Python, "), "{}", stdout(&out));
    let code = std::fs::read_to_string(dir.join("EvenZeros.py")).unwrap();
    assert!(code.contains("def even_zeros(word: str) -> bool:"), "{code}");
    let bad = kstar(&dir, &["export", "--format", "python", "--fn", "2x", "m.kstar", "EvenZeros"]);
    assert_eq!(bad.status.code(), Some(2));
    // The file runs its own checks.
    let Some(python) = ["python3", "python"].into_iter().find(|p| Command::new(p).arg("--version").output().is_ok_and(|o| o.status.success())) else {
        println!("SKIPPED: no Python on the PATH");
        return;
    };
    let run = Command::new(python).arg("EvenZeros.py").current_dir(&dir).output().unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(stdout(&run).contains("sample words answered correctly"), "{}", stdout(&run));
}

/// The Windows programs link the C runtime in (see `.cargo/config.toml`), so
/// they start on a machine without the Visual C++ Redistributable. A program
/// built without it names `VCRUNTIME140.dll` and the `api-ms-win-crt-*`
/// libraries in its import table; a program built with it names none of them.
#[test]
#[cfg(all(windows, target_env = "msvc"))]
fn windows_programs_need_no_vc_runtime() {
    for exe in [env!("CARGO_BIN_EXE_kstar"), env!("CARGO_BIN_EXE_kstar-lsp")] {
        let bytes = std::fs::read(exe).unwrap().to_ascii_lowercase();
        for dll in [&b"vcruntime140"[..], b"msvcp140", b"api-ms-win-crt"] {
            assert!(
                !bytes.windows(dll.len()).any(|w| w == dll),
                "{exe} needs {}, which a fresh Windows may lack",
                String::from_utf8_lossy(dll)
            );
        }
    }
}

/// The value of string `key` in a Windows program's version details: the
/// key in UTF-16, its terminating zero and padding, then the value up to
/// its own zero.
#[cfg(windows)]
fn version_string(bytes: &[u8], key: &str) -> Option<String> {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
    let mut wanted: Vec<u16> = key.encode_utf16().collect();
    wanted.push(0);
    // The resource is 16-bit aligned, so searching whole units finds it.
    let at = units.windows(wanted.len()).position(|w| w == wanted.as_slice())? + wanted.len();
    let start = at + units[at..].iter().take_while(|&&u| u == 0).count();
    let len = units[start..].iter().position(|&u| u == 0)?;
    String::from_utf16(&units[start..start + len]).ok()
}

/// kstar.exe and kstar-lsp.exe say what they are and which version (in
/// Properties > Details): code signing only signs files whose product name
/// and version match the release (see CODE_SIGNING_POLICY.md).
#[test]
#[cfg(all(windows, target_env = "msvc"))]
fn windows_programs_carry_their_version_details() {
    for exe in [env!("CARGO_BIN_EXE_kstar"), env!("CARGO_BIN_EXE_kstar-lsp")] {
        let bytes = std::fs::read(exe).unwrap();
        for (key, value) in [
            ("ProductName", "Kleene Star"),
            ("ProductVersion", env!("CARGO_PKG_VERSION")),
            ("FileVersion", env!("CARGO_PKG_VERSION")),
            ("CompanyName", "Kleene Star"),
        ] {
            assert_eq!(version_string(&bytes, key).as_deref(), Some(value), "{key} of {exe}");
        }
        assert!(version_string(&bytes, "LegalCopyright").is_some_and(|c| c.contains("MIT")), "{exe} has no copyright line");
    }
}

#[test]
fn std_io_uses_the_program_folder_and_allowed_ones() {
    let dir = scratch("std-io");
    std::fs::create_dir_all(dir.join("prog")).unwrap();
    std::fs::create_dir_all(dir.join("shared")).unwrap();
    std::fs::write(dir.join("shared/words.txt"), "01 10\n").unwrap();
    let program = "let n = std.io.write_text(\"out/copy.txt\", std.io.read_text(\"../shared/words.txt\"))\nprint n\n";
    std::fs::write(dir.join("prog/p.kstar"), program).unwrap();

    // Refused without --allow-path, and nothing is written.
    let refused = kstar(&dir, &["run", "prog/p.kstar"]);
    assert!(!refused.status.success());
    assert!(stderr(&refused).contains("E0618") && stderr(&refused).contains("--allow-path"), "{}", stderr(&refused));
    assert!(!dir.join("prog/out").exists());

    // Allowed: paths stay relative to the program's folder, not the current one.
    let allowed = kstar(&dir, &["run", "--allow-path", "shared", "prog/p.kstar"]);
    assert!(allowed.status.success(), "{}", stderr(&allowed));
    assert_eq!(stdout(&allowed), "6\n");
    assert_eq!(std::fs::read_to_string(dir.join("prog/out/copy.txt")).unwrap(), "01 10\n");

    // A folder that does not exist is a usage mistake.
    let missing = kstar(&dir, &["run", "--allow-path", "nowhere", "prog/p.kstar"]);
    assert!(!missing.status.success());
    assert!(stderr(&missing).contains("cannot allow nowhere"), "{}", stderr(&missing));
}

/// Runs `kstar update` with GitHub replaced by `KSTAR_LATEST`, VS Code left
/// alone and no standard input; release files come from `base`, if given.
fn update(program: &Path, dir: &Path, latest: &str, args: &[&str], base: Option<&str>) -> Output {
    let mut command = Command::new(program);
    command
        .arg("update")
        .args(args)
        .current_dir(dir)
        .env("KSTAR_LATEST", latest)
        .env("KSTAR_LATEST_NOTES", "## v9\n\nBetter things, in one paragraph.\n\n### Added\n- more")
        .env("KSTAR_NO_VSCODE", "1")
        .stdin(Stdio::null());
    if let Some(base) = base {
        command.env("KSTAR_UPDATE_BASE_URL", base);
    }
    command.output().unwrap()
}

/// A web server on this machine that serves the files under `root` (as
/// `/<path>`) and answers 404 for anything else, until the test ends.
/// Returns its address.
fn serve(root: PathBuf) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let root = root.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead, Write};
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                if reader.read_line(&mut request).is_err() {
                    return;
                }
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
                        break;
                    }
                }
                let path = request.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/').to_string();
                let found = if path.contains("..") { None } else { std::fs::read(root.join(&path)).ok() };
                let (status, body) = match found {
                    Some(body) => ("200 OK", body),
                    None => ("404 Not Found", b"not here".to_vec()),
                };
                let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(&body);
            });
        }
    });
    format!("http://{address}")
}

/// SHA-256 of `bytes`, in hex, with the system's own tool, so the test does
/// not check kstar's checksums with kstar's code.
fn sha256(dir: &Path, bytes: &[u8]) -> String {
    let file = dir.join("to-sum");
    std::fs::write(&file, bytes).unwrap();
    let out = if cfg!(windows) {
        Command::new("certutil").arg("-hashfile").arg(&file).arg("SHA256").output().unwrap()
    } else if Command::new("sha256sum").arg("--version").output().is_ok() {
        Command::new("sha256sum").arg(&file).output().unwrap()
    } else {
        Command::new("shasum").args(["-a", "256"]).arg(&file).output().unwrap()
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let hex = text.split_whitespace().find(|w| w.len() == 64 && w.chars().all(|c| c.is_ascii_hexdigit())).unwrap();
    hex.to_ascii_lowercase()
}

/// A release `tag` under `root/<tag>/` as the release workflow makes it:
/// this platform's archive with the given programs (and a man page), its
/// `.sha256` file and `SHA256SUMS`. `own_sum` replaces the archive's
/// `.sha256` file, to make it wrong.
fn fake_release(root: &Path, tag: &str, kstar: &[u8], lsp: &[u8], own_sum: Option<&str>) {
    let dir = root.join(tag);
    std::fs::create_dir_all(&dir).unwrap();
    let version = Command::new(env!("CARGO_BIN_EXE_kstar")).arg("version").output().unwrap();
    let target = stdout(&version).lines().find_map(|l| l.strip_prefix("target ")).unwrap().trim().to_string();
    let exe = if cfg!(windows) { ".exe" } else { "" };
    let files: [(String, &[u8]); 4] = [
        (format!("kstar{exe}"), kstar),
        (format!("kstar-lsp{exe}"), lsp),
        ("README.md".into(), b"readme"),
        ("man/kstar.1".into(), b".TH KSTAR 1 new"),
    ];
    let (name, archive) = if cfg!(windows) {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (path, body) in &files {
            zip.start_file(path.as_str(), zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(body).unwrap();
        }
        (format!("kstar-{target}.zip"), zip.finish().unwrap().into_inner())
    } else {
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (path, body) in &files {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, format!("./{path}"), *body).unwrap();
        }
        (format!("kstar-{target}.tar.gz"), tar.into_inner().unwrap().finish().unwrap())
    };
    let sum = sha256(root, &archive);
    std::fs::write(dir.join(&name), &archive).unwrap();
    let own = own_sum.map_or(format!("{sum}  {name}\n"), |s| format!("{s}  {name}\n"));
    std::fs::write(dir.join(format!("{name}.sha256")), own).unwrap();
    std::fs::write(dir.join("SHA256SUMS"), format!("{}  other.zip\n{sum}  {name}\n", "0".repeat(64))).unwrap();
}

#[test]
fn update_asks_first_and_never_hangs() {
    let dir = scratch("update-asks");
    let kstar = Path::new(env!("CARGO_BIN_EXE_kstar"));
    let same = update(kstar, &dir, "v0.0.1", &[], None);
    assert!(same.status.success(), "{}", stderr(&same));
    assert!(stdout(&same).contains("is up to date"), "{}", stdout(&same));

    let check = update(kstar, &dir, "v99.0.0", &["--check"], None);
    assert!(check.status.success(), "{}", stderr(&check));
    let out = stdout(&check);
    assert!(out.contains("latest:    kstar 99.0.0") && out.contains("Better things, in one paragraph."), "{out}");

    // Without a terminal and without --yes it refuses, and says how.
    let refused = update(kstar, &dir, "v99.0.0", &[], None);
    assert!(!refused.status.success());
    assert!(stderr(&refused).contains("--yes"), "{}", stderr(&refused));
}

#[test]
fn update_beta_offers_the_newest_pre_release() {
    let dir = scratch("update-beta");
    let run = |args: &[&str], beta: &str| {
        Command::new(env!("CARGO_BIN_EXE_kstar"))
            .arg("update")
            .args(args)
            .current_dir(&dir)
            .env("KSTAR_LATEST", "v0.0.1")
            .env("KSTAR_LATEST_BETA", beta)
            .env("KSTAR_LATEST_BETA_NOTES", "A beta with lists.")
            .env("KSTAR_NO_VSCODE", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    // Without --beta, betas are not offered.
    let stable = run(&["--check"], "v99.0.0-beta");
    assert!(stdout(&stable).contains("is up to date"), "{}", stdout(&stable));
    let beta = run(&["--check", "--beta"], "v99.0.0-beta");
    let out = stdout(&beta);
    assert!(out.contains("latest:    kstar 99.0.0-beta") && out.contains("A beta with lists."), "{out}");
    assert!(out.contains("kstar update --beta"), "{out}");
    // No beta at all: the stable release stands.
    let none = run(&["--check", "--beta"], "");
    assert!(stdout(&none).contains("is up to date (the latest release, betas included, is v0.0.1)"), "{}", stdout(&none));
    // Without a terminal there is no full-screen view, whatever the flags.
    let refused = run(&["--beta"], "v99.0.0-beta");
    assert!(stderr(&refused).contains("--yes"), "{}", stderr(&refused));
}

#[test]
fn tui_needs_a_terminal() {
    let dir = scratch("tui");
    let out = kstar(&dir, &["tui"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("needs a terminal"), "{}", stderr(&out));
    let bad = kstar(&dir, &["tui", "no-such-folder"]);
    assert!(stderr(&bad).contains("no-such-folder is not a folder"), "{}", stderr(&bad));
}

#[test]
fn update_yes_replaces_the_programs_or_leaves_them() {
    let dir = scratch("update-yes");
    let home = dir.join("home");
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let exe = if cfg!(windows) { ".exe" } else { "" };
    let (kstar, lsp) = (bin.join(format!("kstar{exe}")), bin.join(format!("kstar-lsp{exe}")));
    std::fs::copy(env!("CARGO_BIN_EXE_kstar"), &kstar).unwrap();
    std::fs::write(&lsp, "old lsp").unwrap();
    // The install script put a man page here, so the update refreshes it.
    let man = home.join("share/man/man1/kstar.1");
    std::fs::create_dir_all(man.parent().unwrap()).unwrap();
    std::fs::write(&man, "old man page").unwrap();

    let releases = dir.join("releases");
    fake_release(&releases, "v99.0.0", b"new kstar", b"new lsp", None);
    fake_release(&releases, "v98.0.0", b"bad kstar", b"bad lsp", Some(&"0".repeat(64)));
    let base = serve(releases);
    let unchanged = |what: &str| {
        assert_eq!(std::fs::read_to_string(&lsp).unwrap(), "old lsp", "{what}");
        assert!(Command::new(&kstar).arg("version").output().unwrap().status.success(), "{what}: kstar was not put back");
        assert_eq!(std::fs::read_to_string(&man).unwrap(), "old man page", "{what}");
    };

    // A release that is not there: the HTTP status is named.
    let missing = update(&kstar, &dir, "v97.0.0", &["--yes"], Some(&base));
    assert!(!missing.status.success());
    let err = stderr(&missing);
    assert!(err.contains("step 1 of 4 (Download and check the release)") && err.contains("HTTP 404"), "{err}");
    assert!(err.contains("kstar was left as it was"), "{err}");
    unchanged("missing release");

    // A wrong checksum: refused, and nothing changes.
    let bad = update(&kstar, &dir, "v98.0.0", &["--yes"], Some(&base));
    assert!(!bad.status.success());
    let err = stderr(&bad);
    assert!(err.contains("checksum") && err.contains("does not match") && err.contains(".sha256"), "{err}");
    unchanged("bad checksum");

    // The real thing: both programs are swapped and the old ones go.
    let done = update(&kstar, &dir, "v99.0.0", &["-y"], Some(&base));
    assert!(done.status.success(), "{}{}", stdout(&done), stderr(&done));
    let out = stdout(&done);
    assert!(out.contains("Updated kstar to v99.0.0"), "{out}");
    assert!(out.contains("SHA256SUMS"), "{out}");
    assert_eq!(std::fs::read_to_string(&kstar).unwrap(), "new kstar");
    assert_eq!(std::fs::read_to_string(&lsp).unwrap(), "new lsp");
    assert!(!bin.join("kstar-lsp.old").exists());
    if cfg!(unix) {
        // The running kstar was renamed, so it could be removed too.
        assert!(!bin.join("kstar.old").exists());
        assert_eq!(std::fs::read_to_string(&man).unwrap(), ".TH KSTAR 1 new");
    }
    // The folder it unpacked into is gone.
    let left: Vec<_> = std::fs::read_dir(&home).unwrap().flatten().map(|e| e.file_name()).collect();
    assert!(left.iter().all(|n| !n.to_string_lossy().starts_with(".kstar-update-")), "{left:?}");
}

/// `kstar update` installs by itself: it never starts PowerShell, a shell,
/// curl or gh, and never pipes a downloaded script into one. Virus scanners
/// (Windows Defender among them) stop and quarantine programs that do.
#[test]
fn updating_starts_no_shell() {
    let mut sources = Vec::new();
    let mut folders = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(folder).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources.push(path);
            }
        }
    }
    assert!(sources.len() > 10);
    let banned = [
        "ExecutionPolicy",
        "| iex",
        "|iex",
        "Invoke-Expression",
        "irm http",
        "Command::new(\"powershell",
        "Command::new(\"pwsh",
        "Command::new(\"sh\")",
        "Command::new(\"bash",
        "Command::new(\"cmd",
        "Command::new(\"curl",
        "Command::new(\"wget",
        "Command::new(\"gh\")",
    ];
    for path in sources {
        let text = std::fs::read_to_string(&path).unwrap();
        for word in banned {
            assert!(!text.contains(word), "{} contains `{word}`", path.display());
        }
    }
}

#[test]
fn draw_on_writes_a_page_whose_script_parses() {
    let dir = scratch("draw-on");
    std::fs::write(
        dir.join("m.kstar"),
        "alphabet B = {0, 1}\ndfa D over B { start a  accept b  a --0--> a  a --1--> b  b --0,1--> b }\ndraw D on \"001\"\n",
    )
    .unwrap();
    let out = kstar(&dir, &["run", "m.kstar"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "wrote D.html\n");
    let page = std::fs::read_to_string(dir.join("D.html")).unwrap();
    // The run is embedded as JSON that parses, with a frame per step.
    let start = page.find("id=\"kstar-run\">").unwrap() + "id=\"kstar-run\">".len();
    let json = &page[start..start + page[start..].find("</script>").unwrap()];
    let data: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(data["frames"].as_array().unwrap().len(), 4);
    assert_eq!(data["result"], "ACCEPTED");
    // The player's script is valid JavaScript, if Node is here to say so.
    let script_start = page.rfind("<script>").unwrap() + "<script>".len();
    let script = &page[script_start..script_start + page[script_start..].find("</script>").unwrap()];
    std::fs::write(dir.join("player.js"), script).unwrap();
    match Command::new("node").arg("--check").arg("player.js").current_dir(&dir).output() {
        Ok(check) => assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr)),
        Err(_) => println!("SKIPPED: no Node on the PATH to check the player's script"),
    }
}

/// Runs `kstar` in `dir` with extra environment variables, and with no
/// theme from the machine running the tests.
fn kstar_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kstar"));
    command.args(args).current_dir(dir).env_remove("NO_COLOR").env_remove("KSTAR_THEME");
    // A config folder of the test's own, so a real config file is not read.
    let config = dir.join("config-home");
    command.env("XDG_CONFIG_HOME", &config).env("APPDATA", &config);
    for (k, v) in env {
        command.env(k, v);
    }
    command.output().unwrap()
}

#[test]
fn the_theme_comes_from_the_option_the_environment_or_the_config_file() {
    let dir = scratch("theme");
    std::fs::write(dir.join("m.kstar"), "alphabet B = {0, 1}\ndfa M over B {\n    start a\n    accept a\n    a --0,1--> a\n}\ndraw M\ndraw M on \"01\"\n").unwrap();
    let drawn = |args: &[&str], env: &[(&str, &str)]| {
        let out = kstar_env(&dir, args, env);
        assert!(out.status.success(), "{}", stderr(&out));
        (std::fs::read_to_string(dir.join("M.dot")).unwrap(), std::fs::read_to_string(dir.join("M.html")).unwrap())
    };
    // By default the drawing is light and the page follows the system.
    let (dot, page) = drawn(&["run", "m.kstar"], &[]);
    assert!(dot.contains(&format!("bgcolor=\"{}\"", kstar::theme::LIGHT.background)));
    assert!(page.contains("data-theme=\"auto\""));
    let (dot, page) = drawn(&["run", "--theme", "dark", "m.kstar"], &[]);
    assert!(dot.contains(&format!("bgcolor=\"{}\"", kstar::theme::DARK.background)), "{dot}");
    assert!(page.contains("data-theme=\"dark\""));
    let (dot, _) = drawn(&["run", "m.kstar"], &[("KSTAR_THEME", "high-contrast")]);
    assert!(dot.contains(&format!("bgcolor=\"{}\"", kstar::theme::HIGH_CONTRAST.background)), "{dot}");
    // The option wins over the environment.
    let (dot, _) = drawn(&["run", "--theme=light", "m.kstar"], &[("KSTAR_THEME", "dark")]);
    assert!(dot.contains(&format!("bgcolor=\"{}\"", kstar::theme::LIGHT.background)), "{dot}");

    let config = dir.join("config-home").join("kstar");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.toml"), "# my settings\ntheme = \"dark\"\n").unwrap();
    let (dot, _) = drawn(&["run", "m.kstar"], &[]);
    assert!(dot.contains(&format!("bgcolor=\"{}\"", kstar::theme::DARK.background)), "{dot}");

    // A name that is not a theme says where it came from.
    let bad = kstar_env(&dir, &["run", "--theme", "blue", "m.kstar"], &[]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(stderr(&bad).contains("`--theme` is `blue`; it must be auto, light, dark or high-contrast"), "{}", stderr(&bad));
    let bad = kstar_env(&dir, &["run", "m.kstar"], &[("KSTAR_THEME", "sepia")]);
    assert!(stderr(&bad).contains("KSTAR_THEME is `sepia`"), "{}", stderr(&bad));
    // ...but only for commands that draw.
    assert!(kstar_env(&dir, &["check", "m.kstar"], &[("KSTAR_THEME", "sepia")]).status.success());
}
