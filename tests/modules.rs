//! Imports between files, run through `Session` with files that exist only
//! in the test. (The golden tests in `tests/golden/import_*.kstar` cover the
//! error messages; these cover behaviour.)

mod common;
use common::{run_with, run_with_files, Files, Run};

const LIB: &str = "\
alphabet B = {0, 1}
let Size = 2
fn size() -> Int {
    return Size
}
fn checked(n: Int) -> Int {
    test n > 0
    print \"checked\", n
    return n
}
";

#[test]
fn each_file_keeps_its_own_globals() {
    // Both files define `Size`; a function sees the one in its own file.
    let main = "import \"lib.kstar\"\nlet Size = 100\nprint Size, lib.Size, lib.size()\n";
    let outcome = run_with_files("main.kstar", main, &[("lib.kstar", LIB)]);
    assert_eq!(outcome.output, "100 2 2\n");
    assert!(outcome.ok);
}

#[test]
fn a_modules_functions_behave_normally() {
    // Its top-level `test` and `print` are skipped, but a function that
    // prints and tests does so when called, and its tests count.
    let main = "from \"lib.kstar\" import checked as check\nprint check(3)\nprint check(-1)\n";
    let outcome = run_with_files("main.kstar", main, &[("lib.kstar", LIB)]);
    assert_eq!(outcome.output, "checked 3\n3\ntest failed: lib.kstar, line 7\n    test n > 0\nchecked -1\n-1\ntests: 1 passed, 1 failed\n");
}

#[test]
fn eager_imports_run_at_the_import_line() {
    let top = "let Zero = 0\nlet Bad = 1 % Zero\n";
    let main = "print \"one\"\nimport \"top.kstar\"\nprint \"two\"\n";
    let files = Files::Virtual(vec![("top.kstar".into(), top.into())]);
    let lazy = run_with("main.kstar", main, Run { files: files.clone(), eager_imports: false });
    assert!(lazy.ok, "{}", lazy.output);
    let eager = run_with("main.kstar", main, Run { files, eager_imports: true });
    assert!(!eager.ok);
    // (The module is never used, which is warned about first.)
    assert!(eager.output.contains("W0202") && eager.output.contains("\n\none\nTraceback"), "{}", eager.output);
    assert!(eager.output.contains("main.kstar, line 2, in <main>\n    import \"top.kstar\"\n"), "{}", eager.output);
    assert!(eager.output.contains("top.kstar, line 2, in <module>"), "{}", eager.output);
}

#[test]
fn modules_in_folders_import_relative_to_themselves() {
    let files = [
        ("machines/parity.kstar", "import \"../shared/alpha.kstar\"\ndfa Even over alpha.B { start e  accept e  e --0,1--> o  o --0,1--> e }\n"),
        ("shared/alpha.kstar", "alphabet B = {0, 1}\n"),
    ];
    let main = "import \"machines/parity.kstar\"\nprint parity.Even accepts \"0101\"\n";
    let outcome = run_with_files("main.kstar", main, &files);
    assert_eq!(outcome.output, "true\n");
}

#[test]
fn the_same_file_by_two_routes_is_one_module() {
    let files = [
        ("lib/a.kstar", "import \"b.kstar\"\nfn get() -> Int {\n    return b.Count\n}\n"),
        ("lib/b.kstar", "fn tick() -> Int {\n    print \"b runs\"\n    return 1\n}\nlet Count = tick()\n"),
    ];
    let main = "import \"lib/a.kstar\"\nimport \"lib/./b.kstar\" as b\nprint a.get(), b.Count\n";
    let outcome = run_with_files("main.kstar", main, &files);
    assert_eq!(outcome.output, "b runs\n1 1\n");
}

#[test]
fn the_repl_can_import() {
    let out = kstar::with_big_stack(|| {
        let mut session = kstar::Session::new("<repl>", Box::new(std::io::sink()), std::env::temp_dir(), true);
        session.set_reader(Box::new(|path| {
            assert_eq!(path.to_string_lossy().replace('\\', "/"), "lib.kstar");
            Ok(LIB.to_string())
        }));
        vec![
            session.run("import \"lib.kstar\"").is_ok(),
            session.run("let s = lib.size()").is_ok(),
            session.run("from \"lib.kstar\" import Size").is_ok(),
            // An entry with a mistake is rejected and leaves no trace.
            session.run("print lib.nope").is_ok(),
            session.type_of("lib.Size + s").as_deref() == Ok("Int"),
            session.names().contains(&"lib".to_string()),
        ]
    });
    assert_eq!(out, [true, true, true, false, true, true]);
}

#[test]
fn std_needs_no_import_and_cannot_be_redefined() {
    let outcome = run_with_files("main.kstar", "let std = 1\n", &[]);
    assert!(outcome.output.contains("E0209"), "{}", outcome.output);
    let outcome = run_with_files("main.kstar", "print std\n", &[]);
    assert!(outcome.output.contains("E0211"), "{}", outcome.output);
}
