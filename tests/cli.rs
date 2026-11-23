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
/// alone and no standard input.
fn update(program: &Path, dir: &Path, latest: &str, args: &[&str], script: Option<&Path>) -> Output {
    let mut command = Command::new(program);
    command
        .arg("update")
        .args(args)
        .current_dir(dir)
        .env("KSTAR_LATEST", latest)
        .env("KSTAR_LATEST_NOTES", "## v9\n\nBetter things, in one paragraph.\n\n### Added\n- more")
        .env("KSTAR_NO_VSCODE", "1")
        .stdin(Stdio::null());
    if let Some(script) = script {
        command.env("KSTAR_INSTALL_SCRIPT", script);
    }
    command.output().unwrap()
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
fn update_yes_replaces_the_programs_or_leaves_them() {
    let dir = scratch("update-yes");
    let bin = dir.join("home").join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let exe = if cfg!(windows) { ".exe" } else { "" };
    let (kstar, lsp) = (bin.join(format!("kstar{exe}")), bin.join(format!("kstar-lsp{exe}")));
    std::fs::copy(env!("CARGO_BIN_EXE_kstar"), &kstar).unwrap();
    std::fs::write(&lsp, "old lsp").unwrap();
    // A stand-in for the install script: it writes new programs where it is
    // told, or fails.
    let (good, bad) = if cfg!(windows) {
        let good = dir.join("good.ps1");
        std::fs::write(&good, "$b = Join-Path $env:KSTAR_INSTALL_DIR 'bin'\nSet-Content (Join-Path $b 'kstar.exe') \"new $env:KSTAR_VERSION\"\nSet-Content (Join-Path $b 'kstar-lsp.exe') 'new lsp'\n").unwrap();
        let bad = dir.join("bad.ps1");
        std::fs::write(&bad, "Set-Content (Join-Path $env:KSTAR_INSTALL_DIR 'bin/kstar.exe') 'half'\nexit 1\n").unwrap();
        (good, bad)
    } else {
        let good = dir.join("good.sh");
        std::fs::write(&good, "printf 'new %s' \"$KSTAR_VERSION\" > \"$KSTAR_INSTALL_DIR/bin/kstar\"\nprintf 'new lsp' > \"$KSTAR_INSTALL_DIR/bin/kstar-lsp\"\n").unwrap();
        let bad = dir.join("bad.sh");
        std::fs::write(&bad, "printf half > \"$KSTAR_INSTALL_DIR/bin/kstar\"\nexit 1\n").unwrap();
        (good, bad)
    };

    let failed = update(&kstar, &dir, "v99.0.0", &["--yes"], Some(&bad));
    assert!(!failed.status.success());
    assert!(stderr(&failed).contains("left as it was"), "{}", stderr(&failed));
    assert_eq!(std::fs::read_to_string(&lsp).unwrap(), "old lsp");
    assert!(Command::new(&kstar).arg("version").output().unwrap().status.success(), "kstar was not put back");

    let done = update(&kstar, &dir, "v99.0.0", &["-y"], Some(&good));
    assert!(done.status.success(), "{}{}", stdout(&done), stderr(&done));
    assert!(stdout(&done).contains("Updated kstar to v99.0.0"), "{}", stdout(&done));
    assert_eq!(std::fs::read_to_string(&kstar).unwrap().trim(), "new v99.0.0");
    assert_eq!(std::fs::read_to_string(&lsp).unwrap().trim(), "new lsp");
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
