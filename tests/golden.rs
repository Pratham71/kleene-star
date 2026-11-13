//! Golden tests: every `tests/golden/NAME.kstar` is run and its complete output
//! (everything printed, then the error if there was one) must equal
//! `tests/golden/NAME.out`.
//!
//! A golden program may import files; those live in `tests/golden/modules/`
//! and are not golden tests themselves.
//!
//! After an intended change in behaviour, regenerate the `.out` files with
//! `UPDATE_GOLDEN=1 cargo test --test golden` and review the diff.

mod common;
use common::{files_in, project_dir, run_at};

#[test]
fn golden_files() {
    let programs = files_in(&project_dir().join("tests/golden"), "kstar");
    assert!(!programs.is_empty(), "no golden tests found");
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();

    let mut failures = Vec::new();
    for program in programs {
        let name = program.file_stem().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&program).unwrap();
        let actual = run_at(&program, &format!("{name}.kstar"), &text).output;
        let expected_path = program.with_extension("out");
        if update {
            std::fs::write(&expected_path, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("{name}.out is missing; run with UPDATE_GOLDEN=1"))
            .replace("\r\n", "\n");
        if actual != expected {
            failures.push(format!("---- {name} ----\nexpected:\n{expected}\nactual:\n{actual}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
