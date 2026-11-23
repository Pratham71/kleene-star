//! The man pages and the info manual, made from the help table in
//! `help.rs`, so they always say what `kstar help` says.
//!
//! The generated files are kept in `man/` in the repository and go into the
//! Linux and macOS release archives; `tests/manual.rs` checks that they are
//! up to date (`UPDATE_MAN=1 cargo test --test manual` writes them again).
//!
//! * `kstar.1`: every command, grouped as in `kstar help`, the options for
//!   every command, the environment and the exit status;
//! * `kstar-<command>.1`, one per command, and `kstar-lsp.1`;
//! * `kstar.info`: the same pages as one info manual, with a node per
//!   command and per standard-library module.

use crate::help::{self, Command, COMMANDS, GLOBAL_FLAGS, GROUPS};
use crate::lang::error::Paint;
use crate::lang::stdlib;

/// Text for roff: backslashes and leading dashes escaped, `code` in bold,
/// and a line that would start with `.` or `'` made safe.
fn roff(text: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    let mut previous = ' ';
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let next = chars.get(i + 1).copied().unwrap_or(' ');
        match c {
            '\\' => out.push_str("\\e"),
            '`' => {
                out.push_str(if in_code { "\\fR" } else { "\\fB" });
                in_code = !in_code;
            }
            // A dash that starts a word is an option's: a minus sign, so
            // it can be searched for and copied.
            '-' if matches!(previous, ' ' | '-' | '[' | '(' | '`' | '\n')
                && (next.is_alphanumeric() || matches!(next, '-' | '`') || next.is_whitespace()) =>
            {
                out.push_str("\\-")
            }
            _ => out.push(c),
        }
        previous = c;
    }
    out.lines()
        .map(|line| if line.starts_with(['.', '\'']) { format!("\\&{line}") } else { line.to_string() })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Paragraphs separated by blank lines; one whose lines are all indented
/// is kept as it is (an example of output).
fn paragraphs(text: &str, out: &mut String) {
    for paragraph in text.split("\n\n") {
        if paragraph.lines().all(|l| l.starts_with("    ")) {
            out.push_str(".PP\n.RS 4\n.nf\n");
            for line in paragraph.lines() {
                out.push_str(&roff(line.trim_start()));
                out.push('\n');
            }
            out.push_str(".fi\n.RE\n");
        } else {
            out.push_str(".PP\n");
            out.push_str(&roff(paragraph));
            out.push('\n');
        }
    }
}

/// `--out <dir>` as `\fB\-\-out\fR \fI<dir>\fR`.
fn option_names(names: &str) -> String {
    names
        .split(", ")
        .map(|name| match name.split_once(' ') {
            Some((option, value)) => format!("\\fB{}\\fR \\fI{}\\fR", roff(option), roff(value)),
            None => format!("\\fB{}\\fR", roff(name)),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A page's name as roff writes it in NAME and SEE ALSO: `kstar\-run`.
fn roff_name(name: &str) -> String {
    name.replace('-', "\\-")
}

fn page_name(command: &Command) -> String {
    if command.name == "kstar-lsp" {
        "kstar-lsp".into()
    } else {
        format!("kstar-{}", command.name)
    }
}

fn header(title: &str, out: &mut String) {
    out.push_str(&format!(
        ".\\\" Made from the kstar help (src/help.rs); do not edit. See src/manual.rs.\n\
         .TH {} 1 \"\" \"Kleene Star\" \"Kleene Star Manual\"\n",
        roff(&title.to_uppercase())
    ));
}

fn see_also(pages: &[String], out: &mut String) {
    out.push_str(".SH SEE ALSO\n");
    let refs: Vec<String> = pages.iter().map(|p| format!("\\fB{}\\fR(1)", roff_name(p))).collect();
    out.push_str(&refs.join(",\n"));
    out.push_str("\n.PP\nThe reference and the tutorial: https://github.com/Pratham71/kleene\\-star\n");
}

/// The man page of one command.
pub fn command_page(command: &Command) -> String {
    let name = page_name(command);
    let mut out = String::new();
    header(&name, &mut out);
    out.push_str(&format!(".SH NAME\n{} \\- {}\n", roff_name(&name), roff(command.summary)));
    out.push_str(".SH SYNOPSIS\n.nf\n");
    for line in command.usage {
        // The command's words in bold, its arguments as written.
        let words: Vec<&str> = line.split(' ').collect();
        let program = words.iter().take_while(|w| !w.starts_with(['<', '[', '-'])).count();
        let rest: String = words[program..].iter().map(|w| format!(" {}", roff(w))).collect();
        out.push_str(&format!("\\fB{}\\fR{rest}\n", roff(&words[..program].join(" "))));
    }
    out.push_str(".fi\n.SH DESCRIPTION\n");
    paragraphs(command.description, &mut out);
    if !command.flags.is_empty() {
        out.push_str(".SH OPTIONS\n");
        for flag in command.flags {
            out.push_str(&format!(".TP\n{}\n{}\n", option_names(flag.names), roff(flag.help)));
        }
    }
    out.push_str(".SH EXAMPLES\n");
    for (line, what) in command.examples {
        out.push_str(&format!(".TP\n\\fB{}\\fR\n{}\n", roff(line), roff(what)));
    }
    let mut also = vec!["kstar".to_string()];
    if command.name != "help" && command.name != "kstar-lsp" {
        also.push("kstar-help".into());
    }
    see_also(&also, &mut out);
    out
}

/// `kstar.1`: the overview.
pub fn main_page() -> String {
    let mut out = String::new();
    header("kstar", &mut out);
    out.push_str(".SH NAME\nkstar \\- a small language for Theory of Computation\n");
    out.push_str(".SH SYNOPSIS\n.nf\n\\fBkstar\\fR \\fI<command>\\fR [options]\n\\fBkstar\\fR \\fI<file.kstar>\\fR\n.fi\n");
    out.push_str(&format!(
        ".SH DESCRIPTION\n{}\n",
        roff("Kleene Star is a small language for Theory of Computation: DFAs, NFAs, regular expressions, grammars, pushdown automata and Turing machines, written the way a textbook writes them, with tests, traces and drawings. `kstar` runs, checks and formats programs, traces machines, and exports them as Rust, Python or JSON. `kstar file.kstar` is short for `kstar run file.kstar`.")
    ));
    out.push_str(&format!(
        ".PP\n{}\n",
        roff("Each command has its own page, such as kstar-run(1). `kstar help <command>` shows the same text in the terminal, and `kstar help std` describes the standard library.")
    ));
    out.push_str(".SH COMMANDS\n");
    for group in GROUPS {
        out.push_str(&format!(".SS {}\n", roff(group)));
        for c in COMMANDS.iter().filter(|c| c.group == *group) {
            let (program, rest) = match c.synopsis.split_once(' ') {
                Some((p, r)) => (p, format!(" {}", roff(r))),
                None => (c.synopsis, String::new()),
            };
            out.push_str(&format!(
                ".TP\n\\fB{}\\fR{rest}\n{}; see \\fB{}\\fR(1).\n",
                roff(program),
                roff(c.summary),
                roff_name(&page_name(c))
            ));
        }
    }
    out.push_str(".SH OPTIONS\nFor every command:\n");
    for flag in GLOBAL_FLAGS {
        out.push_str(&format!(".TP\n{}\n{}\n", option_names(flag.names), roff(flag.help)));
    }
    out.push_str(".SH ENVIRONMENT\n");
    for (name, what) in [
        ("NO_COLOR", "when set to anything but the empty string, nothing is coloured unless asked for with `--color always`"),
        ("KSTAR_NO_VSCODE", "when 1, `kstar update` leaves VS Code alone"),
        ("GITHUB_TOKEN", "used by `kstar update` to ask GitHub for the latest release when the `gh` command is not installed"),
    ] {
        out.push_str(&format!(".TP\n\\fB{name}\\fR\n{}\n", roff(what)));
    }
    out.push_str(&format!(
        ".SH EXIT STATUS\n{}\n",
        roff("0 on success; 1 when a program had an error or a failed test (or, for `kstar fmt --check`, a file would change); 2 when the command line itself was wrong. Warnings do not change the exit status.")
    ));
    let mut pages: Vec<String> = COMMANDS.iter().map(page_name).collect();
    pages.sort();
    see_also(&pages, &mut out);
    out
}

/// Every man page, as (file name, text).
pub fn man_pages() -> Vec<(String, String)> {
    let mut pages = vec![("kstar.1".to_string(), main_page())];
    pages.extend(COMMANDS.iter().map(|c| (format!("{}.1", page_name(c)), command_page(c))));
    pages
}

/// One node of the info manual.
struct Node {
    name: String,
    title: String,
    text: String,
    /// The node's menu: (node, description).
    menu: Vec<(String, String)>,
}

fn module_node(module: &stdlib::Module) -> String {
    format!("std {}", module.name)
}

/// `kstar.info`: the overview, a node per command, and a node for the
/// standard library with one per module.
pub fn info_manual() -> String {
    let plain = Paint(false);
    let mut nodes = vec![Node {
        name: "Top".into(),
        title: "Kleene Star".into(),
        text: help::overview(plain),
        menu: COMMANDS
            .iter()
            .map(|c| (c.name.to_string(), c.summary.to_string()))
            .chain([("Standard library".to_string(), "the modules under std".to_string())])
            .collect(),
    }];
    for c in COMMANDS {
        let title = if c.name == "kstar-lsp" { "kstar-lsp".to_string() } else { format!("kstar {}", c.name) };
        nodes.push(Node { name: c.name.into(), title, text: help::command_page(c, plain), menu: Vec::new() });
    }
    nodes.push(Node {
        name: "Standard library".into(),
        title: "The standard library".into(),
        text: help::std_overview(plain),
        menu: stdlib::MODULES.iter().map(|m| (module_node(m), m.doc.to_string())).collect(),
    });
    for m in stdlib::MODULES {
        let text = help::topic(&format!("std.{}", m.name), plain).expect("every module has a page");
        // The page starts with the module's name, which is the title here.
        let text = text.split_once("\n\n").map_or(text.clone(), |(_, rest)| rest.to_string());
        nodes.push(Node { name: module_node(m), title: format!("std.{}", m.name), text, menu: Vec::new() });
    }

    // Each node's Up: commands and the library under Top, modules under
    // the library.
    let up = |name: &str| -> &str {
        if name == "Top" {
            "(dir)"
        } else if name.starts_with("std ") {
            "Standard library"
        } else {
            "Top"
        }
    };
    let mut out = String::from(
        "This is kstar.info, made from the kstar help (src/help.rs) by src/manual.rs.\n\n\
         INFO-DIR-SECTION Programming\n\
         START-INFO-DIR-ENTRY\n\
         * kstar: (kstar).               Kleene Star, a language for Theory of Computation.\n\
         END-INFO-DIR-ENTRY\n\n",
    );
    let mut tags = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        tags.push((node.name.clone(), out.len()));
        let mut line = format!("File: kstar.info,  Node: {}", node.name);
        // Next and Prev among the nodes with the same Up.
        let siblings: Vec<&Node> = nodes.iter().filter(|n| up(&n.name) == up(&node.name)).collect();
        let at = siblings.iter().position(|n| n.name == node.name).unwrap_or(0);
        if let Some(next) = siblings.get(at + 1) {
            line.push_str(&format!(",  Next: {}", next.name));
        } else if i == 0 {
            line.push_str(&format!(",  Next: {}", nodes[1].name));
        }
        if at > 0 {
            line.push_str(&format!(",  Prev: {}", siblings[at - 1].name));
        }
        line.push_str(&format!(",  Up: {}", up(&node.name)));
        out.push_str(&format!("\x1f\n{line}\n\n{}\n{}\n\n", node.title, "*".repeat(node.title.chars().count())));
        out.push_str(node.text.trim_end());
        out.push('\n');
        if !node.menu.is_empty() {
            out.push_str("\n* Menu:\n\n");
            for (target, what) in &node.menu {
                let entry = format!("* {target}::");
                let pad = 24usize.saturating_sub(entry.chars().count()).max(1);
                out.push_str(&format!("{entry}{}{what}\n", " ".repeat(pad)));
            }
        }
        out.push('\n');
    }
    out.push_str("\x1f\nTag Table:\n");
    for (name, offset) in tags {
        out.push_str(&format!("Node: {name}\x7f{offset}\n"));
    }
    out.push_str("\x1f\nEnd Tag Table\n\n\x1f\nLocal Variables:\ncoding: utf-8\nEnd:\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roff_escapes() {
        assert_eq!(roff("use --watch, not -w"), "use \\-\\-watch, not \\-w");
        assert_eq!(roff("dependency-free"), "dependency-free");
        assert_eq!(roff("a `kstar run` b"), "a \\fBkstar run\\fR b");
        assert_eq!(roff("C:\\Users"), "C:\\eUsers");
        assert_eq!(roff("x\n.y\n'z"), "x\n\\&.y\n\\&'z");
        assert_eq!(option_names("-o, --out <path>"), "\\fB\\-o\\fR, \\fB\\-\\-out\\fR \\fI<path>\\fR");
    }

    #[test]
    fn a_page_per_command_with_every_option() {
        let pages = man_pages();
        assert_eq!(pages.len(), COMMANDS.len() + 1);
        for c in COMMANDS {
            let (_, page) = pages.iter().find(|(n, _)| *n == format!("{}.1", page_name(c))).unwrap();
            assert!(page.contains(&format!(".SH NAME\n{} \\- ", roff_name(&page_name(c)))));
            for f in c.flags {
                assert!(page.contains(&option_names(f.names)), "{}: {}", c.name, f.names);
            }
            for (line, _) in c.examples {
                assert!(page.contains(&roff(line)), "{}: {line}", c.name);
            }
        }
        let main = main_page();
        for c in COMMANDS {
            assert!(main.contains(&format!("\\fB{}\\fR(1)", roff_name(&page_name(c)))), "{}", c.name);
        }
    }

    #[test]
    fn the_info_manual_has_a_node_for_each_command_and_module() {
        let info = info_manual();
        for c in COMMANDS {
            assert!(info.contains(&format!("Node: {},", c.name)), "{}", c.name);
            assert!(info.contains(&format!("* {}::", c.name)), "{}", c.name);
        }
        for m in stdlib::MODULES {
            assert!(info.contains(&format!("Node: std {},", m.name)), "{}", m.name);
        }
        // Each tag points at the node's separator.
        let table = &info[info.find("Tag Table:\n").unwrap()..];
        for line in table.lines().filter(|l| l.starts_with("Node: ")) {
            let (name, offset) = line["Node: ".len()..].split_once('\x7f').unwrap();
            let offset: usize = offset.parse().unwrap();
            assert!(info[offset..].starts_with(&format!("\x1f\nFile: kstar.info,  Node: {name},")), "{name}");
        }
    }
}
