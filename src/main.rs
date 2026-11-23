//! The `kstar` command-line tool.

mod repl;
mod selfupdate;
mod sha256;
mod update;

use kstar::lang::codes;
use kstar::lang::error::Paint;
use kstar::{Captured, Session};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    update::remove_old_programs();
    let Some((&command, rest)) = args.split_first() else {
        return help(None);
    };
    if rest.iter().any(|a| matches!(*a, "--help" | "-h")) {
        return help(Some(command));
    }
    // These take only names, so any option they do not list is a mistake.
    if matches!(command, "new" | "explain" | "version") {
        if let Some(bad) = rest.iter().find(|a| a.starts_with('-') && !kstar::help::accepts(command, a)) {
            return usage_error(&format!("unknown option `{bad}`"), command);
        }
    }
    match command {
        "help" | "--help" | "-h" => help(rest.first().copied()),
        "version" | "--version" | "-V" => match rest {
            [] => {
                println!("{}", update::version());
                ExitCode::SUCCESS
            }
            [arg, ..] => usage_error(&format!("`kstar version` takes no arguments, not `{arg}`"), "version"),
        },
        "update" => update::update(rest),
        "run" => run_command(rest, false),
        "check" => run_command(rest, true),
        "fmt" => {
            let rest: Vec<String> = rest.iter().map(|s| s.to_string()).collect();
            kstar::with_big_stack(move || fmt_command(&rest.iter().map(String::as_str).collect::<Vec<_>>()))
        }
        "new" => match rest {
            [name] => new_file(name),
            _ => usage_error("`kstar new` needs one name", "new"),
        },
        "trace" => trace_command(rest),
        "tui" => tui_command(rest),
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
            _ => usage_error("`kstar explain` needs one code, such as E0200", "explain"),
        },
        "repl" => {
            let options = match Options::parse(rest, "repl") {
                Ok(options) if options.files.is_empty() => options,
                Ok(_) => return usage_error("`kstar repl` takes no files; use `:load <file>` inside it", "repl"),
                Err(message) => return usage_error(&message, "repl"),
            };
            kstar::with_big_stack(move || repl::repl(options.color, options.trace_calls, options.theme))
        }
        file if file.ends_with(".kstar") => run_command(&args, false),
        other => {
            let mut message = format!("unknown command `{other}`");
            if let Some(close) = kstar::lang::error::did_you_mean(other, kstar::help::command_names()) {
                message.push_str(&format!("; did you mean `{close}`?"));
            }
            usage_error(&message, "")
        }
    }
}

/// `kstar help [topic]` and `kstar <command> --help`, coloured only on a
/// terminal.
fn help(topic: Option<&str>) -> ExitCode {
    let paint = Paint(color_wanted());
    match topic {
        None => {
            println!("{}", kstar::help::overview(paint));
            ExitCode::SUCCESS
        }
        Some(topic) => match kstar::help::topic(topic, paint) {
            Ok(text) => {
                println!("{text}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("kstar: {message}");
                ExitCode::from(2)
            }
        },
    }
}

/// A mistake in the command line: the message, then the command's usage
/// (or, for "", where to find the commands). The exit code is 2.
fn usage_error(message: &str, command: &str) -> ExitCode {
    match kstar::help::command(command) {
        Some(c) => {
            let usage: Vec<String> = c.usage.iter().map(|u| format!("  {u}")).collect();
            eprintln!(
                "kstar: {message}\n\nUsage:\n{}\n\nRun `kstar help {}` for its options and examples.",
                usage.join("\n"),
                c.name
            );
        }
        None => eprintln!("kstar: {message}\n\nRun `kstar help` for the list of commands."),
    }
    ExitCode::from(2)
}

/// The options shared by `run`, `check` and `repl`.
struct Options {
    files: Vec<String>,
    out_dir: String,
    /// Folders `std.io` may use besides the program's own.
    allow_paths: Vec<String>,
    fixed_clock: bool,
    watch: bool,
    trace_calls: bool,
    eager_imports: bool,
    color: bool,
    /// The colours of drawings and players (`--theme`, `KSTAR_THEME` or
    /// the config file).
    theme: kstar::theme::Choice,
}

impl Options {
    /// Reads the options of `command`; an option its help does not list
    /// is an error, so every option the parser takes is documented.
    fn parse(args: &[&str], command: &str) -> Result<Options, String> {
        let mut options = Options {
            files: Vec::new(),
            out_dir: ".".into(),
            allow_paths: Vec::new(),
            fixed_clock: false,
            watch: false,
            trace_calls: false,
            eager_imports: false,
            color: false,
            theme: kstar::theme::Choice::Auto,
        };
        let mut color = "auto";
        let mut theme = None;
        // `--name=value` is the same as `--name value`.
        let mut split = Vec::new();
        for &arg in args {
            match arg.split_once('=') {
                Some((name, value)) if name.starts_with("--") => {
                    let takes_value = kstar::help::command(command)
                        .and_then(|c| c.flags.iter().find(|f| f.spellings().contains(&name)))
                        .is_some_and(|f| f.takes_value());
                    if kstar::help::accepts(command, name) && !takes_value {
                        return Err(format!("`{name}` takes no value"));
                    }
                    split.extend([name, value]);
                }
                _ => split.push(arg),
            }
        }
        let mut args = split.iter();
        while let Some(&arg) = args.next() {
            let is_option = arg.starts_with('-') && arg != "-";
            if is_option && !kstar::help::accepts(command, arg) {
                return Err(format!("unknown option `{arg}`"));
            }
            match arg {
                "--out" => options.out_dir = args.next().ok_or("`--out` needs a folder")?.to_string(),
                "--allow-path" => {
                    options.allow_paths.push(args.next().ok_or("`--allow-path` needs a folder")?.to_string());
                }
                "--watch" => options.watch = true,
                "--fixed-clock" => options.fixed_clock = true,
                "--eager-imports" => options.eager_imports = true,
                "--trace-calls" => options.trace_calls = true,
                "--color" => color = args.next().ok_or("`--color` needs auto, always or never")?,
                "--theme" => theme = Some(*args.next().ok_or("`--theme` needs auto, light, dark or high-contrast")?),
                _ if is_option => unreachable!("`{arg}` is in the help of `kstar {command}` but not handled"),
                file => options.files.push(file.to_string()),
            }
        }
        // Only commands that draw read the theme, so a mistake in
        // KSTAR_THEME or the config file does not stop `kstar check`.
        if kstar::help::accepts(command, "--theme") {
            options.theme = kstar::theme::resolve(theme)?;
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
    let usage = if check_only { "check" } else { "run" };
    let options = match Options::parse(args, usage) {
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
    let allow_paths = options.allow_paths.clone();
    let fixed_clock = options.fixed_clock;
    let theme = options.theme;
    kstar::with_big_stack(move || {
        let paint = Paint(color);
        let mut session = Session::new(&name, Box::new(std::io::stdout()), out_dir, false);
        session.theme(theme);
        session.eager_imports(eager);
        session.render_svg(true);
        session.ascii_only(console_needs_ascii());
        session.color(color);
        session.trace_calls(trace_calls);
        session.fixed_clock(fixed_clock);
        for dir in &allow_paths {
            if let Err(message) = session.allow_path(Path::new(dir)) {
                eprintln!("kstar: {message}");
                return false;
            }
        }
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
    let options = match Options::parse(args, "trace") {
        Ok(options) => options,
        Err(message) => return usage_error(&message, "trace"),
    };
    let [file, machine, word] = &options.files[..] else {
        return usage_error("`kstar trace` needs a file, a machine and a word", "trace");
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

/// What `kstar export` writes.
#[derive(Clone, Copy, PartialEq)]
enum ExportFormat {
    Rust,
    Python,
    Json,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            ExportFormat::Rust => "rs",
            ExportFormat::Python => "py",
            ExportFormat::Json => "json",
        }
    }

    fn label(self) -> &'static str {
        match self {
            ExportFormat::Rust => "Rust",
            ExportFormat::Python => "Python",
            ExportFormat::Json => "JSON",
        }
    }
}

/// Where `kstar export` puts what it makes.
#[derive(Debug, PartialEq)]
enum ExportTarget {
    Stdout,
    File(PathBuf),
}

/// The file to write for `-o` (or none given): `-` is standard output; a
/// folder (one that exists, or a path ending in a slash) gets
/// `<base>.<extension>` inside it; anything else is the file itself.
/// Without `-o` the file goes next to the source file.
fn export_target(out: Option<&str>, source: &Path, base: &str, extension: &str) -> ExportTarget {
    let file_name = format!("{base}.{extension}");
    match out {
        Some("-") => ExportTarget::Stdout,
        Some(out) if out.ends_with('/') || out.ends_with('\\') || Path::new(out).is_dir() => {
            ExportTarget::File(Path::new(out).join(file_name))
        }
        Some(out) => ExportTarget::File(PathBuf::from(out)),
        None => ExportTarget::File(source.parent().unwrap_or(Path::new("")).join(file_name)),
    }
}

/// `kstar export`: runs a file quietly, then writes one machine as Rust,
/// Python or JSON to a file, by evaluating `to_rust` or `to_json` at the
/// end of the file.
fn export_command(args: &[&str]) -> ExitCode {
    let mut fn_name = None;
    let mut out = None;
    let mut minimize = false;
    let mut force = false;
    let mut format = ExportFormat::Rust;
    let mut rest = Vec::new();
    let mut iter = args.iter();
    while let Some(&arg) = iter.next() {
        match arg {
            "--fn" => match iter.next() {
                Some(name) => fn_name = Some(name.to_string()),
                None => return usage_error("`--fn` needs a function name", "export"),
            },
            "-o" | "--out" => match iter.next() {
                Some(file) => out = Some(file.to_string()),
                None => return usage_error("`-o` needs a file or folder, or `-` for standard output", "export"),
            },
            "--minimize" => minimize = true,
            "--force" => force = true,
            "--format" => match iter.next() {
                Some(&"rust") => format = ExportFormat::Rust,
                Some(&"python") => format = ExportFormat::Python,
                Some(&"json") => format = ExportFormat::Json,
                Some(other) => return usage_error(&format!("unknown format `{other}`: use `rust`, `python` or `json`"), "export"),
                None => return usage_error("`--format` needs `rust`, `python` or `json`", "export"),
            },
            other => rest.push(other),
        }
    }
    let options = match Options::parse(&rest, "export") {
        Ok(options) => options,
        Err(message) => return usage_error(&message, "export"),
    };
    let [file, machine] = &options.files[..] else {
        return usage_error("`kstar export` needs a file and a machine", "export");
    };
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text.replace("\r\n", "\n"),
        Err(e) => {
            eprintln!("kstar: cannot read {file}: {e}");
            return ExitCode::from(2);
        }
    };
    // The machine's own name, when <machine> is a name: `lib.EvenZeros` -> `EvenZeros`.
    let is_name = machine.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
    let own_name = if is_name { machine.rsplit('.').next().unwrap_or(machine).to_string() } else { String::new() };
    // The default function name is the machine's, in snake_case: EvenZeros -> even_zeros.
    let fn_name_given = fn_name.is_some();
    let fn_name = fn_name.unwrap_or_else(|| {
        kstar::automata::codegen::snake_case(if own_name.is_empty() { "accepts" } else { &own_name })
    });
    if format == ExportFormat::Json && fn_name_given {
        return usage_error("`--fn` names a function; it does not go with `--format json`", "export");
    }
    // (For Rust, `to_rust` checks the name and reports it like any error.)
    if format == ExportFormat::Python {
        if let Err(message) = kstar::automata::codegen::check_fn_name(&fn_name) {
            return usage_error(&message, "export");
        }
    }
    let base = if own_name.is_empty() { fn_name.clone() } else { own_name.clone() };
    let target = export_target(out.as_deref(), Path::new(file), &base, format.extension());
    if let ExportTarget::File(path) = &target {
        if path.exists() && !force {
            eprintln!("kstar: {} is already there; pass --force to replace it, or -o to write elsewhere", shown(path));
            return ExitCode::FAILURE;
        }
    }
    let machine_expr = if minimize { format!("minimize({machine})") } else { machine.clone() };
    let quote = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
    let entry = match format {
        ExportFormat::Json if own_name.is_empty() => format!("print to_json({machine_expr})"),
        ExportFormat::Json => format!("print to_json({machine_expr}, \"{}\")", quote(&own_name)),
        ExportFormat::Rust => format!("print to_rust({machine_expr}, \"{}\")", quote(&fn_name)),
        // The DFA as JSON, which is turned into Python below.
        ExportFormat::Python => format!("print to_json(to_dfa({machine_expr}))"),
    };
    let name = shown(Path::new(file));
    let color = options.color;
    let title = format!("`{machine}`");
    let machine = machine.clone();
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
        match format {
            // Rust code already ends in a newline; drop the one `print` adds.
            // JSON does not, so it keeps it, like any text file.
            ExportFormat::Rust => {
                code.pop();
            }
            ExportFormat::Json => {}
            ExportFormat::Python => match kstar::automata::json::from_json(&code) {
                Ok((kstar::automata::json::Machine::Dfa(dfa), _)) => {
                    let fn_name = kstar::automata::codegen::python_name(&fn_name);
                    code = kstar::automata::codegen::dfa_to_python(&dfa, &fn_name, &title);
                }
                _ => {
                    eprintln!("kstar: could not turn {machine} into a DFA for Python");
                    return ExitCode::FAILURE;
                }
            },
        }
        let path = match target {
            ExportTarget::Stdout => {
                print!("{code}");
                return ExitCode::SUCCESS;
            }
            ExportTarget::File(path) => path,
        };
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("kstar: cannot make the folder {}: {e}", parent.display());
                return ExitCode::FAILURE;
            }
        }
        // `create_new` unless --force, so a file that appeared meanwhile is not replaced either.
        let mut options = std::fs::OpenOptions::new();
        options.write(true);
        if force {
            options.create(true).truncate(true);
        } else {
            options.create_new(true);
        }
        let written = options.open(&path).and_then(|mut f| std::io::Write::write_all(&mut f, code.as_bytes()));
        match written {
            Ok(()) => {
                let full = std::path::absolute(&path).unwrap_or(path);
                println!("Wrote {} ({}, {} lines)", full.display(), format.label(), code.lines().count());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("kstar: cannot write {}: {e}", shown(&path));
                ExitCode::FAILURE
            }
        }
    })
}

/// `kstar tui`: the full-screen menu (`kstar::tui::home`).
fn tui_command(args: &[&str]) -> ExitCode {
    let options = match Options::parse(args, "tui") {
        Ok(options) => options,
        Err(message) => return usage_error(&message, "tui"),
    };
    let root = match &options.files[..] {
        [] => PathBuf::from("."),
        [folder] if Path::new(folder).is_dir() => PathBuf::from(folder),
        [folder] => return usage_error(&format!("{folder} is not a folder"), "tui"),
        _ => return usage_error("`kstar tui` takes one folder at most", "tui"),
    };
    if !kstar::tui::wanted(false) {
        eprintln!(
            "kstar: `kstar tui` needs a terminal (and no NO_COLOR or TERM=dumb); use `kstar run`, `kstar trace` and `kstar export` instead"
        );
        return ExitCode::from(2);
    }
    let program = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("kstar"));
    let backend = kstar::tui::home::Local { program, root, theme: options.theme };
    let mut screen = kstar::tui::home::HomeScreen::new(Box::new(backend));
    match kstar::tui::run(&mut screen, &kstar::tui::Palette::of(options.theme)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kstar: the terminal UI failed: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `kstar fmt`: formats each file in place, or with `--check` only reports
/// the ones that are not formatted yet. Exit code 1 means a file could not
/// be formatted, or (with `--check`) at least one file would change.
fn fmt_command(args: &[&str]) -> ExitCode {
    let check = args.contains(&"--check");
    let paths: Vec<String> = args.iter().filter(|a| **a != "--check").map(|a| a.to_string()).collect();
    if let Some(bad) = paths.iter().find(|a| a.starts_with('-') && *a != "-") {
        return usage_error(&format!("unknown option `{bad}`"), "fmt");
    }
    if paths.is_empty() {
        return usage_error("`kstar fmt` needs at least one file or folder, or `-` for standard input", "fmt");
    }
    if paths.iter().any(|p| p == "-") {
        if paths.len() > 1 {
            return usage_error("`-` cannot be combined with files", "fmt");
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
