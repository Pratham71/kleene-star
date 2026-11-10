//! Tests for `toc fmt`.
//!
//! * Golden files: every `tests/format/NAME.toc` is formatted and the result
//!   (or the error) must equal `tests/format/NAME.out`. Regenerate them with
//!   `UPDATE_GOLDEN=1 cargo test --test format` and review the diff.
//! * Every example is already formatted, and formatting never changes what a
//!   program prints.
//! * Formatting is idempotent: formatting formatted text changes nothing.

mod common;
use common::{files_in, project_dir, run_program};

fn format(name: &str, text: &str) -> Result<String, String> {
    toc::format_program(name, &text.replace("\r\n", "\n"))
}

#[test]
fn golden_format_files() {
    let inputs = files_in(&project_dir().join("tests/format"), "toc");
    assert!(!inputs.is_empty(), "no format tests found");
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = Vec::new();
    for input in inputs {
        let name = input.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&input).unwrap();
        let actual = format(&name, &text).unwrap_or_else(|error| error + "\n");
        let expected_path = input.with_extension("out");
        if update {
            std::fs::write(&expected_path, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("{name}: .out is missing; run with UPDATE_GOLDEN=1"))
            .replace("\r\n", "\n");
        if actual != expected {
            failures.push(format!("---- {name} ----\nexpected:\n{expected}\nactual:\n{actual}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// Every example is already in the standard style, and formatting it (again)
/// leaves exactly the same program: same output, same test results.
#[test]
fn examples_are_formatted_and_keep_their_meaning() {
    for example in files_in(&project_dir().join("examples"), "toc") {
        let name = example.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&example).unwrap().replace("\r\n", "\n");
        let formatted = format(&name, &text).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(formatted, text, "examples/{name} is not formatted; run `toc fmt examples/{name}`");
        assert_eq!(format(&name, &formatted).unwrap(), formatted, "{name}: not idempotent");
        let before = run_program(&name, &text);
        let after = run_program(&name, &formatted);
        assert_eq!(before.output, after.output, "{name}: formatting changed the output");
    }
}

/// Unformatted inputs format the same way twice, and still run the same.
#[test]
fn formatting_is_idempotent_and_keeps_output() {
    let root = project_dir();
    let mut programs = files_in(&root.join("tests/format"), "toc");
    programs.extend(files_in(&root.join("tests/golden"), "toc"));
    for program in programs {
        let name = program.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&program).unwrap().replace("\r\n", "\n");
        let Ok(once) = format(&name, &text) else { continue }; // syntax errors
        assert_eq!(format(&name, &once).unwrap(), once, "{name}: not idempotent");
        // Line numbers may move, so only compare programs that run cleanly.
        let before = run_program(&name, &text);
        if before.ok {
            assert_eq!(before.output, run_program(&name, &once).output, "{name}: output changed");
        }
    }
}

#[test]
fn windows_line_endings_are_kept() {
    let formatted = toc::format_program("w.toc", "let x=1\r\nprint x\r\n").unwrap();
    assert_eq!(formatted, "let x = 1\r\nprint x\r\n");
}
