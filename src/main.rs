//! The `kstar` command-line tool.

mod repl;
mod update;

use kstar::lang::codes;
use kstar::lang::error::Paint;
use kstar::Session;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

const USAGE: &str = "\
Kleene Star: a small language for Theory of Computation

Usage: kstar <command> [options]

Commands:
  run <files>...        run programs (files or folders of .kstar files)
  check <files>...      look for errors and warnings without running
  fmt <files>...        rewrite files in the standard style
  new <name>            start a new program from a template
  trace <file> <M> <w>  run a file quietly, then trace machine M on word w
  export <file> <M>     write machine M as a standalone Rust function
  explain <code>        explain an error code, such as E0200
  repl                  try things out interactively
  version               show the version, commit, build date and target
  update [--check]      see whether a newer release exists
  help [command]        show this message, or the help for a command

`kstar file.kstar` is short for `kstar run file.kstar`.
Run `kstar help <command>` for the options of a command.";

const RUN_HELP: &str = "\
Usage: kstar run [options] <files>...

Runs each program. A folder means every .kstar file inside it.

Options:
  --out <dir>        where `draw` writes its files (default: the current folder)
  --watch            run again whenever a file changes; stop with Ctrl-C
  --trace-calls      print every function call and return
  --eager-imports    run each imported module at its import, not at first use
  --color <when>     auto (default), always or never

Imported files are checked before anything runs. A module's top level runs
the first time one of its names is used, without its print, test, draw and
trace statements.

The exit code is 0 when every program ran without errors and no test failed.";

const CHECK_HELP: &str = "\
Usage: kstar check [options] <files>...

Reports every error and warning in each program, without running it. A
folder means every .kstar file inside it.

Options:
  --color <when>     auto (default), always or never

The exit code is 0 when no file has errors (warnings do not count).";

const FMT_HELP: &str = "\
Usage: kstar fmt [--check] <files>...

Rewrites each file in the standard style. A folder means every .kstar file
inside it, and `-` reads a program from standard input and writes the
formatted program to standard output.

Options:
  --check            change nothing; list the files that need formatting
                     and exit with code 1 if there are any";

const NEW_HELP: &str = "\
Usage: kstar new <name>

Writes <name>.kstar, a small program to start from. An existing file is
never overwritten.";

const TRACE_HELP: &str = "\
Usage: kstar trace [--color <when>] <file> <machine> <word>

Runs the file without showing its output, then shows every step of
<machine> on <word>, as `trace <machine> on \"<word>\"` would at the end of
the file. <machine> may be any expression, such as `to_dfa(r)`. Use \"\" for
the empty word.";

const EXPORT_HELP: &str = "\
Usage: kstar export [options] <file> <machine>

Runs the file without showing its output, then writes <machine> as Rust: one
dependency-free function `pub fn name(input: &str) -> bool` that accepts
exactly the machine's words, with a `#[cfg(test)]` module of sample words.
<machine> may be any regular language (Dfa, Nfa, Regex or Set[Str]), such as
`EvenZeros` or `to_dfa(r)`; it is turned into a DFA first.

Options:
  --fn <name>        the function's name (default: <machine> in snake_case)
  -o, --out <file>   write to <file> instead of standard output
  --minimize         minimize the DFA first, for the smallest table
  --color <when>     auto (default), always or never";

const UPDATE_HELP: &str = "\
Usage: kstar update [--check]

Asks GitHub for the latest stable release and says whether it is newer than
this kstar. Without --check it also prints the command that installs it; kstar
does not replace itself while it is running. Pre-releases (such as 0.2.5-beta)
are not offered: install one by name with KSTAR_VERSION (see docs/install.md).

Options:
  --check   only say whether a newer release exists";

const VERSION_HELP: &str = "\
Usage: kstar version

Shows this kstar's version, the commit it was built from, the build date and
the platform it was built for. `kstar --version` and `kstar -V` do the same.";

const EXPLAIN_HELP: &str = "\
Usage: kstar explain <code>

Explains an error or warning code, such as E0200 or W0100, with an example.
Every error message shows its code in brackets: NameError[E0200].
`kstar explain --list` lists every code.";

const REPL_HELP: &str = "\
Usage: kstar repl [--trace-calls] [--color <when>]

An interactive prompt: type statements or expressions and see the results.
Type :help inside it for its commands.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let Some((&command, rest)) = args.split_first() else {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    };
    if rest.iter().any(|a| matches!(*a, "--help" | "-h")) {
        return help(Some(command));
    }
    match command {
        "help" | "--help" | "-h" => help(rest.first().copied()),
        "version" | "--version" | "-V" => {
            println!("{}", update::version());
            ExitCode::SUCCESS
        }
        "update" => update::update(rest),
        "run" => run_command(rest, false),
        "check" => run_command(rest, true),
        "fmt" => {
            let rest: Vec<String> = rest.iter().map(|s| s.to_string()).collect();
            kstar::with_big_stack(move || fmt_command(&rest.iter().map(String::as_str).collect::<Vec<_>>()))
        }
        "new" => match rest {
            [name] => new_file(name),
            _ => usage_error("`kstar new` needs one name", NEW_HELP),
        },
        "trace" => trace_command(rest),
        "export" => export_command(rest),
        "explain" => match rest {
            ["--list"] => {
                for code in codes::ALL {
                    let title = codes::explain(code).and_then(|t| t.lines().next().map(str::to_string));
                    println!("{}", title.unwrap_or_else(|| code.to_string()));
                }
                ExitCode::SUCCESS
            }
            [code] => explain(code),
            _ => usage_error("`kstar explain` needs one code, such as E0200", EXPLAIN_HELP),
        },
        "repl" => {
            let options = match Options::parse(rest, false) {
                Ok(options) if options.files.is_empty() => options,
                Ok(_) => return usage_error("`kstar repl` takes no files; use `:load <file>` inside it", REPL_HELP),
                Err(message) => return usage_error(&message, REPL_HELP),
            };
            kstar::with_big_stack(move || repl::repl(options.color, options.trace_calls))
        }
        file if file.ends_with(".kstar") => run_command(&args, false),
        other => {
            let mut message = format!("unknown command `{other}`");
            let commands =
                ["run", "check", "fmt", "new", "trace", "export", "explain", "repl", "version", "update", "help"];
            if let Some(close) = kstar::lang::error::did_you_mean(other, commands.into_iter()) {
                message.push_str(&format!("; did you mean `{close}`?"));
            }
            usage_error(&message, USAGE)
        }
    }
}

fn help(command: Option<&str>) -> ExitCode {
    let text = match command {
        None => USAGE,
        Some("run") => RUN_HELP,
        Some("check") => CHECK_HELP,
        Some("fmt") => FMT_HELP,
        Some("new") => NEW_HELP,
        Some("explain") => EXPLAIN_HELP,
        Some("repl") => REPL_HELP,
        Some("trace") => TRACE_HELP,
        Some("export") => EXPORT_HELP,
        Some("update") => UPDATE_HELP,
        Some("version") => VERSION_HELP,
        Some(_) => USAGE,
    };
    println!("{text}");
    ExitCode::SUCCESS
}

fn usage_error(message: &str, usage: &str) -> ExitCode {
    eprintln!("kstar: {message}\n\n{usage}");
    ExitCode::from(2)
}

/// The options shared by `run`, `check` and `repl`.
struct Options {
    files: Vec<String>,
    out_dir: String,
    watch: bool,
    trace_calls: bool,
    eager_imports: bool,
    color: bool,
}

impl Options {
    fn parse(args: &[&str], allow_out: bool) -> Result<Options, String> {
        let mut options = Options {
            files: Vec::new(),
            out_dir: ".".into(),
            watch: false,
            trace_calls: false,
            eager_imports: false,
            color: false,
        };
        let mut color = "auto";
        let mut args = args.iter();
        while let Some(&arg) = args.next() {
            match arg {
                "--out" if allow_out => {
                    options.out_dir = args.next().ok_or("`--out` needs a folder")?.to_string();
                }
                "--watch" if allow_out => options.watch = true,
                "--eager-imports" if allow_out => options.eager_imports = true,
                "--trace-calls" => options.trace_calls = true,
                "--color" => color = args.next().ok_or("`--color` needs auto, always or never")?,
                _ if arg.starts_with("--color=") => color = &arg["--color=".len()..],
                _ if arg.starts_with('-') && arg != "-" => return Err(format!("unknown option `{arg}`")),
                file => options.files.push(file.to_string()),
            }
        }
        options.color = match color {
            "auto" => color_wanted(),
            "always" => {
                enable_ansi();
                true
            }
            "never" => false,
            other => return Err(format!("`--color` must be auto, always or never, not `{other}`")),
        };
        Ok(options)
    }
}

/// Colour by default only for a terminal, and never when `NO_COLOR` is set
/// (see no-color.org).
fn color_wanted() -> bool {
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    !no_color && std::io::stdout().is_terminal() && std::io::stderr().is_terminal() && enable_ansi()
}

/// Turns on ANSI colour codes in a Windows console, which older consoles
/// show as junk unless asked. Returns false if that is not possible.
fn enable_ansi() -> bool {
    #[cfg(windows)]
    {
        type Handle = *mut std::ffi::c_void;
        extern "system" {
            fn GetStdHandle(which: u32) -> Handle;
            fn GetConsoleMode(console: Handle, mode: *mut u32) -> i32;
            fn SetConsoleMode(console: Handle, mode: u32) -> i32;
        }
        const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
        const STD_ERROR_HANDLE: u32 = -12i32 as u32;
        const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
        let mut ok = true;
        for which in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // SAFETY: plain Win32 calls on the process's own standard
            // handles; `mode` is a valid place to write a u32.
            unsafe {
                let handle = GetStdHandle(which);
                let mut mode = 0;
                ok &= GetConsoleMode(handle, &mut mode) != 0
                    && SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0;
            }
        }
        ok
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// The .kstar files named by `args`: files as given, folders searched
/// (recursively, sorted). Unreadable paths are reported and skipped.
fn expand(args: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for arg in args {
        let path = Path::new(arg);
        if path.is_dir() {
            let before = files.len();
            collect_dir(path, &mut files).map_err(|e| format!("cannot read {arg}: {e}"))?;
            if files.len() == before {
                return Err(format!("{arg} contains no .kstar files"));
            }
        } else if path.exists() {
            files.push(path.to_path_buf());
        } else {
            return Err(format!("{arg} does not exist"));
        }
    }
    Ok(files)
}

fn collect_dir(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?.map(|e| e.map(|e| e.path())).collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_dir(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "kstar") {
            files.push(path);
        }
    }
    Ok(())
}

/// How a file is named in messages: as given, with forward slashes.
fn shown(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// `kstar run` and `kstar check`.
fn run_command(args: &[&str], check_only: bool) -> ExitCode {
    let usage = if check_only { CHECK_HELP } else { RUN_HELP };
    let options = match Options::parse(args, !check_only) {
        Ok(options) => options,
        Err(message) => return usage_error(&message, usage),
    };
    if options.files.is_empty() {
        let what = if check_only { "check" } else { "run" };
        return usage_error(&format!("`kstar {what}` needs at least one file or folder"), usage);
    }
    if options.watch {
        return watch(&options);
    }
    match expand(&options.files) {
        Ok(files) => run_files(&files, &options, check_only),
        Err(message) => {
            eprintln!("kstar: {message}");
            ExitCode::from(2)
        }
    }
}

/// Runs (or checks) each file; with several, names each one first and
/// ends with a count.
fn run_files(files: &[PathBuf], options: &Options, check_only: bool) -> ExitCode {
    let paint = Paint(options.color);
    let many = files.len() > 1;
    let mut failed = Vec::new();
    for (i, file) in files.iter().enumerate() {
        if many {
            if i > 0 {
                println!();
            }
            println!("{}", paint.bold(&format!("--- {}", shown(file))));
        }
        if !run_file(file, options, check_only) {
            failed.push(shown(file));
        }
    }
    if many {
        let ok = files.len() - failed.len();
        let summary = format!("{} files: {ok} ok, {} failed", files.len(), failed.len());
        println!("\n{}", if failed.is_empty() { paint.green(&summary) } else { paint.red(&summary) });
        for file in &failed {
            println!("  failed: {file}");
        }
    }
    if failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Runs (or only checks) one file. True when it had no errors and no
/// failed tests.
fn run_file(file: &Path, options: &Options, check_only: bool) -> bool {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text.replace("\r\n", "\n"),
        Err(e) => {
            eprintln!("kstar: cannot read {}: {e}", shown(file));
            return false;
        }
    };
    let name = shown(file);
    let out_dir = PathBuf::from(&options.out_dir);
    let (color, trace_calls, eager) = (options.color, options.trace_calls, options.eager_imports);
    kstar::with_big_stack(move || {
        let paint = Paint(color);
        let mut session = Session::new(&name, Box::new(std::io::stdout()), out_dir, false);
        session.eager_imports(eager);
        session.render_svg(true);
        session.ascii_only(console_needs_ascii());
        session.color(color);
        session.trace_calls(trace_calls);
        let result = if check_only { session.check(&text).map(drop) } else { session.run(&text) };
        let warnings = session.take_warnings();
        for warning in &warnings {
            eprintln!("{warning}\n");
        }
        if check_only && result.is_ok() {
            let found = match warnings.len() {
                0 => String::new(),
                1 => " (1 warning)".into(),
                n => format!(" ({n} warnings)"),
            };
            println!("{name}: {}{found}", paint.green("no errors found"));
        }
        let (passed, failed) = session.tests();
        if passed + failed > 0 {
            let failed_text = format!("{failed} failed");
            let failed_text = if failed > 0 { paint.red(&failed_text) } else { failed_text };
            println!("tests: {} passed, {failed_text}", paint.green(&passed.to_string()));
        }
        match result {
            Ok(()) => failed == 0,
            Err(message) => {
                eprintln!("{message}");
                if let Some(code) = first_code(&message) {
                    eprintln!("{}", paint.bold(&format!("(for more about an error, run `kstar explain {code}`)")));
                }
                false
            }
        }
    })
}

/// The first error code in a rendered report, such as `E0200`.
fn first_code(report: &str) -> Option<&str> {
    let start = report.find("Error[")? + "Error[".len();
    report.get(start..start + 5)
}

/// `kstar run --watch`: runs the files, then again each time one of them
/// changes (or a .kstar file appears in a watched folder).
fn watch(options: &Options) -> ExitCode {
    let paint = Paint(options.color);
    let stamps = |files: &[PathBuf]| -> Vec<(PathBuf, Option<SystemTime>)> {
        files.iter().map(|f| (f.clone(), std::fs::metadata(f).and_then(|m| m.modified()).ok())).collect()
    };
    let mut last = None;
    loop {
        let files = match expand(&options.files) {
            Ok(files) => files,
            Err(message) => {
                eprintln!("kstar: {message}");
                return ExitCode::from(2);
            }
        };
        let now = stamps(&files);
        if last.as_ref() != Some(&now) {
            if last.is_some() {
                println!("\n{}", paint.bold("=== a file changed; running again ==="));
            }
            // Watching goes on whatever the result, so the exit code is unused.
            let _ = run_files(&files, options, false);
            println!("{}", paint.bold("(watching for changes; press Ctrl-C to stop)"));
            last = Some(now);
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// `kstar trace`: runs a file with its output thrown away, then traces one
/// machine on one word.
fn trace_command(args: &[&str]) -> ExitCode {
    let options = match Options::parse(args, false) {
        Ok(options) => options,
        Err(message) => return usage_error(&message, TRACE_HELP),
    };
    let [file, machine, word] = &options.files[..] else {
        return usage_error("`kstar trace` needs a file, a machine and a word", TRACE_HELP);
    };
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text.replace("\r\n", "\n"),
        Err(e) => {
            eprintln!("kstar: cannot read {file}: {e}");
            return ExitCode::from(2);
        }
    };
    let name = shown(Path::new(file));
    let word = word.replace('\\', "\\\\").replace('"', "\\\"");
    let entry = format!("trace {machine} on \"{word}\"");
    let color = options.color;
    kstar::with_big_stack(move || {
        let mut session = Session::new(&name, Box::new(std::io::sink()), PathBuf::from("."), true);
        session.set_path(Path::new(&name));
        session.ascii_only(console_needs_ascii());
        session.color(color);
        let result = session.run(&text).and_then(|()| {
            session.set_output(Box::new(std::io::stdout()));
            session.run(&entry)
        });
        match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                ExitCode::FAILURE
            }
        }
    })
}

/// What a program prints, kept so that it can be read back.
#[derive(Clone, Default)]
struct Captured(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// `kstar export`: runs a file quietly, then writes one machine as Rust,
/// by evaluating `to_rust(machine, "name")` at the end of the file.
fn export_command(args: &[&str]) -> ExitCode {
    let mut fn_name = None;
    let mut out_file = None;
    let mut minimize = false;
    let mut rest = Vec::new();
    let mut iter = args.iter();
    while let Some(&arg) = iter.next() {
        match arg {
            "--fn" => match iter.next() {
                Some(name) => fn_name = Some(name.to_string()),
                None => return usage_error("`--fn` needs a function name", EXPORT_HELP),
            },
            "-o" | "--out" => match iter.next() {
                Some(file) => out_file = Some(file.to_string()),
                None => return usage_error("`-o` needs a file name", EXPORT_HELP),
            },
            "--minimize" => minimize = true,
            other => rest.push(other),
        }
    }
    let options = match Options::parse(&rest, false) {
        Ok(options) => options,
        Err(message) => return usage_error(&message, EXPORT_HELP),
    };
    let [file, machine] = &options.files[..] else {
        return usage_error("`kstar export` needs a file and a machine", EXPORT_HELP);
    };
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text.replace("\r\n", "\n"),
        Err(e) => {
            eprintln!("kstar: cannot read {file}: {e}");
            return ExitCode::from(2);
        }
    };
    // The default name is the machine's, in Rust's style: EvenZeros -> even_zeros.
    let is_name = machine.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
    let fn_name = fn_name.unwrap_or_else(|| {
        let base = if is_name { machine.rsplit('.').next().unwrap_or(machine) } else { "accepts" };
        kstar::automata::codegen::snake_case(base)
    });
    let target = if minimize { format!("minimize({machine})") } else { machine.clone() };
    let quoted = fn_name.replace('\\', "\\\\").replace('"', "\\\"");
    let entry = format!("print to_rust({target}, \"{quoted}\")");
    let name = shown(Path::new(file));
    let color = options.color;
    kstar::with_big_stack(move || {
        let mut session = Session::new(&name, Box::new(std::io::sink()), PathBuf::from("."), true);
        session.set_path(Path::new(&name));
        session.color(color);
        let code = Captured::default();
        let result = session.run(&text).and_then(|()| {
            session.set_output(Box::new(code.clone()));
            session.run(&entry)
        });
        if let Err(message) = result {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
        let mut code = String::from_utf8_lossy(&code.0.borrow()).to_string();
        code.pop(); // the newline `print` adds
        match out_file {
            None => {
                print!("{code}");
                ExitCode::SUCCESS
            }
            Some(out) => match std::fs::write(&out, code) {
                Ok(()) => {
                    eprintln!("wrote {out}: `pub fn {fn_name}(input: &str) -> bool`");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("kstar: cannot write {out}: {e}");
                    ExitCode::FAILURE
                }
            },
        }
    })
}

/// `kstar fmt`: formats each file in place, or with `--check` only reports
/// the ones that are not formatted yet. Exit code 1 means a file could not
/// be formatted, or (with `--check`) at least one file would change.
fn fmt_command(args: &[&str]) -> ExitCode {
    let check = args.contains(&"--check");
    let paths: Vec<String> = args.iter().filter(|a| **a != "--check").map(|a| a.to_string()).collect();
    if let Some(bad) = paths.iter().find(|a| a.starts_with('-') && *a != "-") {
        return usage_error(&format!("unknown option `{bad}`"), FMT_HELP);
    }
    if paths.is_empty() {
        return usage_error("`kstar fmt` needs at least one file or folder, or `-` for standard input", FMT_HELP);
    }
    if paths.iter().any(|p| p == "-") {
        if paths.len() > 1 {
            return usage_error("`-` cannot be combined with files", FMT_HELP);
        }
        return fmt_stdin(check);
    }
    let files = match expand(&paths) {
        Ok(files) => files,
        Err(message) => {
            eprintln!("kstar: {message}");
            return ExitCode::from(2);
        }
    };
    let mut status = ExitCode::SUCCESS;
    for file in &files {
        let name = shown(file);
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("kstar: cannot read {name}: {e}");
                return ExitCode::from(2);
            }
        };
        let formatted = match kstar::format_program(&name, &text) {
            Ok(formatted) => formatted,
            Err(message) => {
                eprintln!("{message}");
                status = ExitCode::FAILURE;
                continue;
            }
        };
        if formatted == text {
            continue;
        }
        if check {
            println!("{name}: would be reformatted");
            status = ExitCode::FAILURE;
        } else if let Err(e) = std::fs::write(file, &formatted) {
            eprintln!("kstar: cannot write {name}: {e}");
            status = ExitCode::FAILURE;
        } else {
            println!("formatted {name}");
        }
    }
    status
}

/// `kstar fmt -`: formats standard input to standard output, as editors
/// and other tools expect.
fn fmt_stdin(check: bool) -> ExitCode {
    let mut text = String::new();
    if let Err(e) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text) {
        eprintln!("kstar: cannot read standard input: {e}");
        return ExitCode::from(2);
    }
    match kstar::format_program("<stdin>", &text) {
        Ok(formatted) if check => {
            if formatted == text {
                ExitCode::SUCCESS
            } else {
                println!("<stdin>: would be reformatted");
                ExitCode::FAILURE
            }
        }
        Ok(formatted) => {
            print!("{formatted}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// `kstar new`: writes a starter program.
fn new_file(name: &str) -> ExitCode {
    let path = if name.ends_with(".kstar") { PathBuf::from(name) } else { PathBuf::from(format!("{name}.kstar")) };
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if path.exists() {
        eprintln!("kstar: {} already exists; choose another name", shown(&path));
        return ExitCode::FAILURE;
    }
    let file = shown(&path);
    let text = kstar::starter_program(&stem, &file);
    let created = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()));
    match created {
        Ok(()) => {
            println!("wrote {file}; run it with:  kstar run {file}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kstar: cannot write {file}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `kstar explain`: the long explanation of a code, from docs/errors.md,
/// with code blocks indented instead of fenced.
fn explain(code: &str) -> ExitCode {
    let Some(text) = codes::explain(code) else {
        eprintln!("kstar: there is no code `{code}`; `kstar explain --list` lists them all");
        return ExitCode::FAILURE;
    };
    let mut in_block = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_block = !in_block;
            continue;
        }
        if in_block {
            println!("    {line}");
        } else {
            println!("{line}");
        }
    }
    ExitCode::SUCCESS
}

/// True when non-ASCII output, such as the em dash in a `trace` table, would
/// come out garbled.
///
/// Rust writes straight to a Windows console in UTF-16, so a console window
/// always shows it correctly. The risk is output piped into another program
/// (PowerShell, for example) that decodes it with the console's code page:
/// unless that code page is UTF-8 (65001), the em dash turns into junk. Other
/// systems use UTF-8 throughout.
pub fn console_needs_ascii() -> bool {
    #[cfg(windows)]
    {
        if std::io::stdout().is_terminal() {
            return false;
        }
        extern "system" {
            fn GetConsoleOutputCP() -> u32;
        }
        // SAFETY: GetConsoleOutputCP takes no arguments and only reads a
        // setting; it returns 0 when there is no console at all.
        let code_page = unsafe { GetConsoleOutputCP() };
        code_page != 0 && code_page != 65001
    }
    #[cfg(not(windows))]
    {
        false
    }
}
