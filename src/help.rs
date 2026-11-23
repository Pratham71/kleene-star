//! The help for the `kstar` command, written down once.
//!
//! Every command is one `Command` entry: its usage, what it does, every
//! option it takes and a few examples. `kstar help`, `kstar <command>
//! --help` and the man pages are all made from this table, and the option
//! parsers in `main.rs` ask `accepts` whether a command takes an option, so
//! an option cannot be added without its help.
//!
//! `kstar help std` and `kstar help std.<module>` come from the standard
//! library's own table (`lang::stdlib::MODULES`).

use crate::lang::error::{did_you_mean, Paint};
use crate::lang::stdlib::{self, Module};

/// One option of a command.
pub struct Flag {
    /// How it is written, with its value: `-o, --out <path>`.
    pub names: &'static str,
    /// What it does, in one paragraph (wrapped when shown).
    pub help: &'static str,
}

impl Flag {
    /// The option's spellings, without the value: `["-o", "--out"]`.
    pub fn spellings(&self) -> Vec<&'static str> {
        self.names.split(", ").map(|n| n.split(' ').next().unwrap_or(n)).collect()
    }

    /// Whether it is followed by a value, such as `--out <dir>`.
    pub fn takes_value(&self) -> bool {
        self.names.contains('<')
    }
}

/// One command, such as `kstar run`.
pub struct Command {
    /// The name after `kstar`; for the language server, `kstar-lsp`.
    pub name: &'static str,
    /// The heading it is listed under in `kstar help`.
    pub group: &'static str,
    /// How it is listed in `kstar help`: `run <files>...`.
    pub synopsis: &'static str,
    /// One line about what it does.
    pub summary: &'static str,
    /// The usage lines, each a whole command line.
    pub usage: &'static [&'static str],
    /// What it does, in paragraphs separated by blank lines, already
    /// wrapped to fit 80 columns.
    pub description: &'static str,
    pub flags: &'static [Flag],
    /// Command lines, each with what it does.
    pub examples: &'static [(&'static str, &'static str)],
}

/// The headings of `kstar help`, in order.
pub const GROUPS: &[&str] = &["Run and check", "Explore and learn", "Export", "Editor", "Install and update"];

const COLOR: Flag = Flag {
    names: "--color <when>",
    help: "auto (the default), always or never. With auto, output is coloured only on a terminal and never when the NO_COLOR environment variable is set.",
};

/// The options for every command.
pub const GLOBAL_FLAGS: &[Flag] = &[
    Flag { names: "-h, --help", help: "show a command's help, as `kstar help <command>` does" },
    Flag { names: "-V, --version", help: "show the version, as `kstar version` does" },
];

pub const COMMANDS: &[Command] = &[
    Command {
        name: "run",
        group: "Run and check",
        synopsis: "run <files>...",
        summary: "run programs (files or folders of .kstar files)",
        usage: &["kstar run [options] <files>...", "kstar <file.kstar>"],
        description: "\
Runs each program. A folder means every .kstar file inside it, and
`kstar file.kstar` is short for `kstar run file.kstar`. With several files,
each file's output starts with its name, and a count of the files that
passed and failed comes at the end.

Imported files are checked before anything runs. A module's top level runs
the first time one of its names is used, without its print, test, draw and
trace statements.

The exit code is 0 when every program ran without errors and no test
failed, 1 otherwise, and 2 when the command itself was wrong.",
        flags: &[
            Flag { names: "--out <dir>", help: "where `draw` writes its files (default: the current folder)" },
            Flag {
                names: "--allow-path <dir>",
                help: "let std.io also read and write files in <dir>; give it more than once for more folders. Without it a program may only use files in its own folder.",
            },
            Flag {
                names: "--fixed-clock",
                help: "std.benchmark reports every time as 0, so the output is the same on every run",
            },
            Flag { names: "--watch", help: "run again whenever a file changes; stop with Ctrl-C" },
            Flag { names: "--trace-calls", help: "print every function call with its arguments, and every return with its result" },
            Flag { names: "--eager-imports", help: "run each imported module at its import line, not when one of its names is first used" },
            Flag {
                names: "--theme <name>",
                help: "the colours of drawings and of the page `draw M on \"w\"` writes: auto (the default: the page follows the system, drawings are light), light, dark or high-contrast. Without it, the KSTAR_THEME environment variable or `theme = \"<name>\"` in the config file (~/.config/kstar/config.toml, or %APPDATA%\\kstar\\config.toml on Windows) decides.",
            },
            COLOR,
        ],
        examples: &[
            ("kstar run examples/tour.kstar", "run one program"),
            ("kstar run labs/ --out drawings", "run every program in labs/, writing drawings into drawings/"),
            ("kstar run --theme dark lab1.kstar", "run lab1.kstar, drawing in the dark theme"),
        ],
    },
    Command {
        name: "check",
        group: "Run and check",
        synopsis: "check <files>...",
        summary: "look for errors and warnings without running",
        usage: &["kstar check [options] <files>..."],
        description: "\
Reports every error and warning in each program, without running it. A
folder means every .kstar file inside it.

The exit code is 0 when no file has errors (warnings do not count), 1 when
one has, and 2 when the command itself was wrong.",
        flags: &[COLOR],
        examples: &[
            ("kstar check lab1.kstar", "check one program"),
            ("kstar check labs/", "check every program in labs/"),
        ],
    },
    Command {
        name: "fmt",
        group: "Run and check",
        synopsis: "fmt <files>...",
        summary: "rewrite files in the standard style",
        usage: &["kstar fmt [--check] <files>...", "kstar fmt -"],
        description: "\
Rewrites each file in the standard style. A folder means every .kstar file
inside it, and `-` reads a program from standard input and writes the
formatted program to standard output, for editors and scripts.",
        flags: &[Flag {
            names: "--check",
            help: "change nothing; list the files that need formatting and exit with code 1 if there are any",
        }],
        examples: &[
            ("kstar fmt lab1.kstar", "format one file in place"),
            ("kstar fmt --check .", "list the files in this folder that are not formatted"),
            ("kstar fmt - < lab1.kstar", "print lab1.kstar formatted, changing nothing"),
        ],
    },
    Command {
        name: "new",
        group: "Run and check",
        synopsis: "new <name>",
        summary: "start a new program from a template",
        usage: &["kstar new <name>"],
        description: "\
Writes <name>.kstar, a small working program to start from. An existing
file is never overwritten.",
        flags: &[],
        examples: &[("kstar new lab1", "write lab1.kstar"), ("kstar run lab1.kstar", "then run it")],
    },
    Command {
        name: "repl",
        group: "Explore and learn",
        synopsis: "repl",
        summary: "try things out interactively",
        usage: &["kstar repl [options]"],
        description: "\
An interactive prompt: type statements or expressions and see the results.
A bare expression is printed, and names can be defined again. Inside it,
`:type <expr>` shows a type, `:trace M <word>` traces a machine,
`:load <file>` runs a file, `:reset` forgets everything, and `:help` and
`:quit` do what they say. Tab completes names.",
        flags: &[
            Flag { names: "--trace-calls", help: "print every function call with its arguments, and every return with its result" },
            Flag {
                names: "--theme <name>",
                help: "the colours of drawings and of the page `draw M on \"w\"` writes: auto (the default: the page follows the system, drawings are light), light, dark or high-contrast. Without it, the KSTAR_THEME environment variable or `theme = \"<name>\"` in the config file (~/.config/kstar/config.toml, or %APPDATA%\\kstar\\config.toml on Windows) decides.",
            },
            COLOR,
        ],
        examples: &[("kstar repl", "start the prompt"), ("kstar repl --trace-calls", "start it, showing every call")],
    },
    Command {
        name: "trace",
        group: "Explore and learn",
        synopsis: "trace <file> <M> <w>",
        summary: "run a file quietly, then trace machine M on word w",
        usage: &["kstar trace [options] <file> <machine> <word>"],
        description: "\
Runs the file without showing its output, then shows every step of
<machine> on <word>, as `trace <machine> on \"<word>\"` would at the end of
the file. <machine> may be any expression, such as `to_dfa(r)`. Use \"\" for
the empty word.",
        flags: &[COLOR],
        examples: &[
            ("kstar trace examples/01_dfa.kstar HasDouble1 0110", "trace a DFA on 0110"),
            ("kstar trace lab.kstar \"to_dfa(r)\" \"\"", "trace an expression on the empty word"),
        ],
    },
    Command {
        name: "explain",
        group: "Explore and learn",
        synopsis: "explain <code>",
        summary: "explain an error code, such as E0200",
        usage: &["kstar explain <code>", "kstar explain --list"],
        description: "\
Explains an error or warning code, such as E0200 or W0100, with an example.
Every error message shows its code in brackets: NameError[E0200].",
        flags: &[Flag { names: "--list", help: "list every code with its title" }],
        examples: &[("kstar explain E0200", "explain one code"), ("kstar explain --list", "list them all")],
    },
    Command {
        name: "tui",
        group: "Explore and learn",
        synopsis: "tui [folder]",
        summary: "a full-screen menu to run, trace, draw or export",
        usage: &["kstar tui [--theme <name>] [<folder>]"],
        description: "\
A full-screen menu in the terminal. Pick a .kstar file in <folder> (the
current one by default, three folders deep), then run the whole file, or
pick a machine in it and trace it on a word step by step (Left and Right
step through the run), draw it, draw a run of it, or export it as Rust,
Python or JSON. Esc goes back, q leaves.

To list a file's machines it runs the file quietly, as `kstar trace`
does. Everything here is also a command: `kstar run`, `kstar trace`,
`draw` in a program, and `kstar export`. It needs a terminal; it does not
start when output is redirected, or with NO_COLOR or TERM=dumb.",
        flags: &[
            Flag {
                names: "--theme <name>",
                help: "the colours of the screen: auto (the default: the terminal's own colours), light, dark or high-contrast. KSTAR_THEME and the config file work as for `kstar run`.",
            },
        ],
        examples: &[("kstar tui", "pick a program in this folder"), ("kstar tui examples", "pick one of the examples")],
    },
    Command {
        name: "help",
        group: "Explore and learn",
        synopsis: "help [topic]",
        summary: "help for a command, or for the standard library",
        usage: &["kstar help [<command>]", "kstar help std", "kstar help std.<module>[.<function>]"],
        description: "\
Shows the overview of every command, or the full help for one command (the
same as `kstar <command> --help`). `kstar help std` lists the modules of
the standard library, `kstar help std.<module>` lists a module's functions
with their types, and `kstar help std.<module>.<function>` shows one.",
        flags: &[],
        examples: &[
            ("kstar help export", "every option of kstar export"),
            ("kstar help std", "the modules of the standard library"),
            ("kstar help std.automata", "the functions of std.automata"),
        ],
    },
    Command {
        name: "export",
        group: "Export",
        synopsis: "export <file> <M>",
        summary: "write machine M as Rust, Python or JSON",
        usage: &["kstar export [options] <file> <machine>"],
        description: "\
Runs the file without showing its output, then writes <machine> to a file
next to <file>, named after the machine (HasDouble1.rs, HasDouble1.py or
HasDouble1.json), and says where:

    Wrote C:\\Users\\me\\HasDouble1.rs (Rust, 42 lines)

It never replaces a file that is already there unless given --force.

As Rust (the default), the file is one dependency-free function
`pub fn name(input: &str) -> bool` that accepts exactly the machine's
words, with a `#[cfg(test)]` module of sample words. As Python, it is one
dependency-free file with `def name(word: str) -> bool` that checks itself
on sample words when run as a script. For both, <machine> may be any
regular language (Dfa, Nfa, Regex or Set[Str]), such as `EvenZeros` or
`to_dfa(r)`; it is turned into a DFA first.

As JSON it writes <machine> in Kleene Star's JSON format, which `from_json`
reads back: any Dfa, Nfa, Regex, Grammar, Pda or Tm, kept as it is, with
its state names.",
        flags: &[
            Flag { names: "--format <format>", help: "rust (the default), python or json" },
            Flag { names: "--fn <name>", help: "the function's name (default: <machine> in snake_case); not with --format json" },
            Flag {
                names: "-o, --out <path>",
                help: "write to this file, or into this folder; `-o -` prints to standard output instead of writing a file",
            },
            Flag { names: "--force", help: "replace the file if it is already there" },
            Flag { names: "--minimize", help: "minimize the DFA first, for the smallest table" },
            COLOR,
        ],
        examples: &[
            ("kstar export examples/01_dfa.kstar HasDouble1", "write HasDouble1.rs next to the file"),
            ("kstar export --format python lab.kstar M -o -", "print M as a Python function"),
            ("kstar export --format json lab.kstar G -o out/ --force", "write out/G.json, replacing it"),
        ],
    },
    Command {
        name: "kstar-lsp",
        group: "Editor",
        synopsis: "kstar-lsp",
        summary: "the language server editors start (a separate program)",
        usage: &["kstar-lsp"],
        description: "\
The Kleene Star language server. An editor starts it and talks to it over
standard input and output with the Language Server Protocol; it is not
meant to be run by hand. It reports errors and warnings as you type, and
gives hover, completion, signature help, go to definition, references,
rename, outline and formatting.

The VS Code extension starts it for you (`kstar update` keeps the two
matched). For another editor, configure kstar-lsp as the language server
for .kstar files.",
        flags: &[
            Flag { names: "-h, --help", help: "show this help and exit" },
            Flag { names: "-V, --version", help: "show the version and exit" },
        ],
        examples: &[("kstar-lsp --version", "check which version an editor would start")],
    },
    Command {
        name: "update",
        group: "Install and update",
        synopsis: "update",
        summary: "update kstar and its VS Code extension",
        usage: &["kstar update [options]"],
        description: "\
On a terminal, when there is a newer release, shows a full-screen view:
this kstar's version, the latest stable and beta releases, what is new in
the chosen one, a question, then the install step by step with its output.
Left and Right choose stable or beta, Enter updates, q leaves.

Otherwise (with --no-tui, --check or --yes, when standard input or output
is not a terminal, or with NO_COLOR or TERM=dumb) it works in plain text:
it asks GitHub for the latest stable release (with GITHUB_TOKEN, if set). If
it is newer than this kstar, it shows both versions and what the release is
about, and asks \"Update to vX.Y.Z? [Y/n]\". On yes it downloads the
release's archive for this system, checks it against both its .sha256 file
and SHA256SUMS, and swaps the new kstar and kstar-lsp into the folder kstar
is in, putting the old ones back if anything fails; then it updates the VS
Code extension. It does all of this itself, without PowerShell or a shell.
On no it changes nothing. Betas (pre-releases such as 0.2.5-beta) are
offered only with --beta, or on the beta channel of the full-screen view.

When standard input is not a terminal (a script or CI), it does not ask and
does not update: pass --yes.

If VS Code is installed (the `code` command) and kstar is up to date, it
also makes sure the Kleene Star extension matches this kstar: when it is
missing or from another release, it downloads kleene-star-<version>.vsix
from this kstar's release, checks its SHA-256 checksum and installs it.",
        flags: &[
            Flag {
                names: "--check",
                help: "only say whether a newer release exists, and whether the VS Code extension matches, without installing anything",
            },
            Flag { names: "--beta", help: "also offer betas: update to the latest pre-release when it is newer than the latest stable release" },
            Flag { names: "-y, --yes", help: "update without asking" },
            Flag { names: "--no-vscode", help: "leave VS Code alone (so does KSTAR_NO_VSCODE=1)" },
            Flag { names: "--no-tui", help: "plain text and a [Y/n] question instead of the full-screen view" },
            Flag {
                names: "--theme <name>",
                help: "the colours of the screen: auto (the default: the terminal's own colours), light, dark or high-contrast. KSTAR_THEME and the config file work as for `kstar run`.",
            },
        ],
        examples: &[
            ("kstar update --check", "see whether there is a newer release"),
            ("kstar update", "update, after asking"),
            ("kstar update --beta --yes", "update to the newest release, betas included, without asking"),
        ],
    },
    Command {
        name: "version",
        group: "Install and update",
        synopsis: "version",
        summary: "show the version, commit, build date and platform",
        usage: &["kstar version", "kstar --version"],
        description: "\
Shows this kstar's version, the commit it was built from, the build date
and the platform it was built for. `kstar --version` and `kstar -V` do the
same.",
        flags: &[],
        examples: &[("kstar version", "show the version")],
    },
];

/// The entry for a command; `lsp` is short for `kstar-lsp`.
pub fn command(name: &str) -> Option<&'static Command> {
    let name = if name == "lsp" { "kstar-lsp" } else { name };
    COMMANDS.iter().find(|c| c.name == name)
}

/// Whether `command` takes the option `flag` (written without a value,
/// `--color=always` counting as `--color`).
pub fn accepts(command: &str, flag: &str) -> bool {
    let flag = flag.split('=').next().unwrap_or(flag);
    self::command(command).is_some_and(|c| c.flags.iter().any(|f| f.spellings().contains(&flag)))
}

/// The names `did you mean` chooses from for a mistyped command.
pub fn command_names() -> impl Iterator<Item = &'static str> {
    COMMANDS.iter().map(|c| c.name).filter(|n| *n != "kstar-lsp")
}

/// Wraps one paragraph to `width` columns, starting each line after the
/// first with `indent` spaces. The first line has `first` columns used.
fn wrap(text: &str, width: usize, indent: usize, first: usize) -> String {
    let mut out = String::new();
    let mut column = first;
    // Whether this line already has a word on it.
    let mut on_line = false;
    for word in text.split_whitespace() {
        let len = word.chars().count();
        if on_line && column + 1 + len > width {
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            column = indent;
            on_line = false;
        }
        if on_line {
            out.push(' ');
            column += 1;
        }
        out.push_str(word);
        column += len;
        on_line = true;
    }
    out
}

/// A list of `term  explanation` lines, the explanations lined up at
/// column `at` and wrapped.
fn table(rows: &[(String, &str)], at: usize, paint: Paint, out: &mut String) {
    for (term, text) in rows {
        let shown = term.chars().count() + 2;
        out.push_str("  ");
        out.push_str(&paint.green(term));
        if shown + 2 > at {
            out.push('\n');
            out.push_str(&" ".repeat(at));
        } else {
            out.push_str(&" ".repeat(at - shown));
        }
        out.push_str(&wrap(text, 80, at, at));
        out.push('\n');
    }
}

/// `kstar help`: every command, one line each, grouped by task.
pub fn overview(paint: Paint) -> String {
    let mut out = format!(
        "{}: a small language for Theory of Computation, version {}\n\n",
        paint.bold("Kleene Star"),
        env!("CARGO_PKG_VERSION")
    );
    out.push_str(&format!("{} kstar <command> [options]\n", paint.bold("Usage:")));
    out.push_str("       kstar <file.kstar>   (short for kstar run <file.kstar>)\n");
    for group in GROUPS {
        out.push_str(&format!("\n{}\n", paint.bold(&format!("{group}:"))));
        let rows: Vec<(String, &str)> =
            COMMANDS.iter().filter(|c| c.group == *group).map(|c| (c.synopsis.to_string(), c.summary)).collect();
        table(&rows, 25, paint, &mut out);
    }
    out.push_str(&format!("\n{}\n", paint.bold("Options for every command:")));
    let rows: Vec<(String, &str)> = GLOBAL_FLAGS.iter().map(|f| (f.names.to_string(), f.help)).collect();
    table(&rows, 25, paint, &mut out);
    out.push_str(
        "\nRun `kstar help <command>` for a command's options and examples,\n\
         `kstar help std` for the standard library, and `kstar explain <code>`\n\
         for an error code.",
    );
    out
}

/// `kstar help <command>` and `kstar <command> --help`.
pub fn command_page(command: &Command, paint: Paint) -> String {
    let mut out = String::new();
    for (i, line) in command.usage.iter().enumerate() {
        let label = if i == 0 { "Usage:" } else { "      " };
        out.push_str(&format!("{} {line}\n", paint.bold(label)));
    }
    out.push('\n');
    out.push_str(command.description);
    out.push('\n');
    if !command.flags.is_empty() {
        out.push_str(&format!("\n{}\n", paint.bold("Options:")));
        let rows: Vec<(String, &str)> = command.flags.iter().map(|f| (f.names.to_string(), f.help)).collect();
        table(&rows, 22, paint, &mut out);
    }
    out.push_str(&format!("\n{}\n", paint.bold("Examples:")));
    for (line, what) in command.examples {
        out.push_str(&format!("  {}\n      {}\n", paint.green(line), wrap(what, 80, 6, 6)));
    }
    out.pop();
    out
}

/// `kstar help std`: the modules of the standard library.
pub fn std_overview(paint: Paint) -> String {
    let mut out = format!("{}\n\n", paint.bold("The standard library"));
    out.push_str(&wrap(
        "Every module is reached as std.<module>.<name> without an import, or imported with `import std.<module>` or `from std.<module> import <name>`.",
        80,
        0,
        0,
    ));
    out.push_str(&format!("\n\n{}\n", paint.bold("Modules:")));
    let rows: Vec<(String, &str)> = stdlib::MODULES.iter().map(|m| (format!("std.{}", m.name), m.doc)).collect();
    table(&rows, 20, paint, &mut out);
    out.push_str("\nRun `kstar help std.<module>` for a module's functions.");
    out
}

fn module_page(module: &Module, paint: Paint) -> String {
    let mut out = format!("{}\n\n{}\n", paint.bold(&format!("std.{}", module.name)), wrap(module.doc, 80, 0, 0));
    for f in module.functions {
        out.push_str(&format!("\n  {}\n      {}\n", paint.green(&f.signature()), wrap(f.doc, 80, 6, 6)));
    }
    let call = format!("Call them as std.{0}.<name>(...), or import them with `from std.{0} import <name>`.", module.name);
    out.push('\n');
    out.push_str(&wrap(&call, 80, 0, 0));
    out
}

/// The help for one topic: a command, `std`, `std.<module>` or
/// `std.<module>.<function>`. An unknown topic is an error message.
pub fn topic(name: &str, paint: Paint) -> Result<String, String> {
    if let Some(command) = command(name) {
        return Ok(command_page(command, paint));
    }
    if name == "std" {
        return Ok(std_overview(paint));
    }
    if let Some(path) = name.strip_prefix("std.") {
        let (module_name, function) = match path.split_once('.') {
            Some((m, f)) => (m, Some(f)),
            None => (path, None),
        };
        let Some(module) = stdlib::module(module_name) else {
            let mut message = format!("there is no module `std.{module_name}`");
            if let Some(close) = did_you_mean(module_name, stdlib::MODULES.iter().map(|m| m.name)) {
                message.push_str(&format!("; did you mean `std.{close}`?"));
            }
            return Err(message + "; `kstar help std` lists them");
        };
        let Some(function) = function else {
            return Ok(module_page(module, paint));
        };
        let Some(f) = module.functions.iter().find(|f| f.name == function) else {
            let mut message = format!("`std.{module_name}` has no function `{function}`");
            if let Some(close) = did_you_mean(function, module.functions.iter().map(|f| f.name)) {
                message.push_str(&format!("; did you mean `{close}`?"));
            }
            return Err(message + &format!("; `kstar help std.{module_name}` lists them"));
        };
        return Ok(format!("{}\n\n{}", paint.green(&f.signature()), wrap(f.doc, 80, 0, 0)));
    }
    let mut message = format!("there is no help for `{name}`");
    if let Some(close) = did_you_mean(name, command_names()) {
        message.push_str(&format!("; did you mean `{close}`?"));
    }
    Err(message + "; `kstar help` lists the commands")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Paint = Paint(false);

    #[test]
    fn every_command_is_in_a_group_and_has_examples() {
        for c in COMMANDS {
            assert!(GROUPS.contains(&c.group), "{}", c.name);
            assert!(!c.examples.is_empty(), "{} has no examples", c.name);
            assert!(c.examples.len() <= 3, "{} has more than 3 examples", c.name);
        }
        let overview = overview(PLAIN);
        for c in COMMANDS {
            assert!(overview.contains(c.synopsis), "{}", c.name);
            assert!(overview.contains(c.summary), "{}", c.name);
        }
    }

    #[test]
    fn every_line_fits_80_columns() {
        let mut pages = vec![overview(PLAIN), std_overview(PLAIN)];
        pages.extend(COMMANDS.iter().map(|c| command_page(c, PLAIN)));
        pages.extend(stdlib::MODULES.iter().map(|m| module_page(m, PLAIN)));
        for page in pages {
            for line in page.lines() {
                // A signature or a long example is a single word and may not fit.
                if line.trim().contains(' ') && !line.trim_start().starts_with("fn ") && !line.trim_start().starts_with("kstar ") {
                    assert!(line.chars().count() <= 80, "too long: {line}");
                }
            }
        }
    }

    #[test]
    fn a_command_page_shows_every_option_and_example() {
        let page = command_page(command("export").unwrap(), PLAIN);
        assert!(page.starts_with("Usage: kstar export [options] <file> <machine>\n"));
        for names in ["--format <format>", "-o, --out <path>", "--force", "--minimize", "--fn <name>"] {
            assert!(page.contains(names), "{names}");
        }
        assert!(page.contains("  kstar export --format python lab.kstar M -o -\n      print M as a Python function"));
    }

    #[test]
    fn options_are_accepted_by_their_commands_only() {
        assert!(accepts("run", "--watch"));
        assert!(accepts("run", "--color=never"));
        assert!(accepts("export", "-o"));
        assert!(accepts("update", "-y"));
        assert!(!accepts("check", "--watch"));
        assert!(!accepts("check", "--trace-calls"));
        assert!(!accepts("nope", "--watch"));
    }

    #[test]
    fn standard_library_topics() {
        let all = topic("std", PLAIN).unwrap();
        for m in stdlib::MODULES {
            assert!(all.contains(&format!("std.{}", m.name)));
        }
        let automata = topic("std.automata", PLAIN).unwrap();
        for f in stdlib::module("automata").unwrap().functions {
            assert!(automata.contains(&f.signature()), "{}", f.name);
        }
        let one = topic("std.collections.power_set", PLAIN).unwrap();
        assert!(one.starts_with("fn power_set(s: Set[T]) -> Set[Set[T]]\n\n"), "{one}");
        assert!(topic("std.automat", PLAIN).unwrap_err().contains("did you mean `std.automata`?"));
        assert!(topic("std.collections.powerset", PLAIN).unwrap_err().contains("did you mean `power_set`?"));
        assert!(topic("chek", PLAIN).unwrap_err().contains("did you mean `check`?"));
        assert_eq!(topic("lsp", PLAIN), topic("kstar-lsp", PLAIN));
    }

    #[test]
    fn wrapping() {
        assert_eq!(wrap("a b c", 3, 0, 0), "a b\nc");
        assert_eq!(wrap("one two three", 9, 2, 2), "one two\n  three");
        assert_eq!(wrap("", 10, 0, 0), "");
    }
}
