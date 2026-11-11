//! The editor sees the same errors as the command line: for every golden
//! program that is rejected before it runs, the language server's first
//! diagnostic must be exactly the first error `kstar` prints, at the same line.

mod common;
use common::{files_in, project_dir};
use kstar::ide::{position_of, Analysis};

#[test]
fn diagnostics_match_the_command_line() {
    let mut checked = 0;
    for program in files_in(&project_dir().join("tests/golden"), "kstar") {
        let src = std::fs::read_to_string(&program).unwrap().replace("\r\n", "\n");
        let expected = std::fs::read_to_string(program.with_extension("out")).unwrap().replace("\r\n", "\n");
        // Only errors found before running: they start with "Error in NAME, line N:".
        let Some(first) = expected.lines().next().filter(|l| l.starts_with("Error in ")) else { continue };
        let line: u32 = first.rsplit("line ").next().unwrap().trim_end_matches(':').parse().unwrap();
        let message = expected.lines().find(|l| l.contains("Error[")).unwrap();
        let analysis = Analysis::new(&src);
        let problem = analysis.errors().next().unwrap_or_else(|| panic!("{program:?}: no problem reported"));
        assert_eq!(problem.message, message, "{program:?}");
        assert_eq!(position_of(&src, problem.span.start).0 + 1, line, "{program:?}");
        checked += 1;
    }
    assert!(checked > 15, "only {checked} golden errors were checked");
}

#[test]
fn every_example_analyses_cleanly() {
    for example in files_in(&project_dir().join("examples"), "kstar") {
        let src = std::fs::read_to_string(&example).unwrap();
        let analysis = Analysis::new(&src);
        assert_eq!(analysis.problems, vec![], "{example:?}");
        assert!(!analysis.index.defs.is_empty());
    }
}
