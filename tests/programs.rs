//! Keeps the examples and the documentation honest.
//!
//! * Every program in `examples/` must run without errors or failed tests.
//! * Every code block in the docs marked ```kstar must do the same, every block
//!   marked ```kstar-error must be rejected, and the output shown underneath
//!   must be what the program really prints. So if the language changes,
//!   out-of-date documentation fails the build. After a deliberate change,
//!   `UPDATE_DOCS=1 cargo test --test programs` rewrites those output blocks.
//! * A block marked ```kstar file=utils.kstar is not run. It is a file that
//!   the later blocks on the same page (or, in docs/errors.md, the same
//!   section) can import, as if it were next to them on disk.

mod common;
use common::{files_in, project_dir, run_at, run_program, run_with_files};

#[test]
fn examples_run_cleanly() {
    let examples = files_in(&project_dir().join("examples"), "kstar");
    assert!(!examples.is_empty(), "no examples found");
    for example in examples {
        let name = example.file_name().unwrap().to_string_lossy().to_string();
        let outcome = run_at(&example, &name, &std::fs::read_to_string(&example).unwrap());
        assert!(outcome.ok, "examples/{name} failed:\n{}", outcome.output);
    }
}

/// A fenced code block: its tag (the word after the opening ```), its text,
/// and whether only blank lines separate it from the block before.
struct Block {
    tag: String,
    text: String,
    follows_directly: bool,
    /// The lines of the text, between the fences, counting from 0.
    lines: std::ops::Range<usize>,
}

fn code_blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut current: Option<Block> = None;
    let mut prose_since_last_block = true;
    for (number, line) in markdown.lines().enumerate() {
        let fence = line.trim_start().strip_prefix("```");
        match (&mut current, fence) {
            (Some(_), Some(_)) => {
                let mut block = current.take().unwrap();
                block.lines.end = number;
                blocks.push(block);
                prose_since_last_block = false;
            }
            (Some(block), None) => {
                block.text.push_str(line);
                block.text.push('\n');
            }
            (None, Some(tag)) => {
                current = Some(Block {
                    tag: tag.trim().to_string(),
                    text: String::new(),
                    follows_directly: !prose_since_last_block,
                    lines: number + 1..number + 1,
                });
            }
            (None, None) => prose_since_last_block |= !line.trim().is_empty(),
        }
    }
    blocks
}

/// Every ```kstar block must run cleanly and every ```kstar-error block must
/// fail. When a ```text block comes straight after either, it must be
/// exactly what the program prints.
#[test]
fn documentation_code_runs() {
    let root = project_dir();
    let mut pages = vec![root.join("README.md")];
    pages.extend(files_in(&root.join("docs"), "md"));
    pages.extend(files_in(&root.join("docs/tutorial"), "md"));

    let update = std::env::var_os("UPDATE_DOCS").is_some();
    let mut problems = Vec::new();
    let mut checked = 0;
    for page in pages {
        let name = page.strip_prefix(root).unwrap().display().to_string();
        let markdown = std::fs::read_to_string(&page).unwrap().replace("\r\n", "\n");
        let blocks = code_blocks(&markdown);
        // With UPDATE_DOCS, (line range, new text) for each stale output block.
        let mut updates = Vec::new();
        // The files defined so far on this page, for the blocks to import.
        let mut files: Vec<(String, String)> = Vec::new();
        for (i, block) in blocks.iter().enumerate() {
            if let Some(file) = file_name(block) {
                files.retain(|(name, _)| name != file);
                files.push((file.to_string(), block.text.clone()));
                continue;
            }
            let should_pass = match block.tag.as_str() {
                "kstar" => true,
                "kstar-error" => false,
                _ => continue,
            };
            checked += 1;
            let outcome = run_block(&block.text, &files);
            if outcome.ok != should_pass {
                problems.push(format!(
                    "{name}: this block should {}:\n{}\nit printed:\n{}",
                    if should_pass { "run cleanly" } else { "fail" },
                    block.text,
                    outcome.output
                ));
                continue;
            }
            if let Some(shown) = blocks.get(i + 1).filter(|b| b.tag == "text" && b.follows_directly) {
                if shown.text.trim_end() != outcome.output.trim_end() {
                    if update {
                        updates.push((shown.lines.clone(), outcome.output.trim_end().to_string()));
                        continue;
                    }
                    problems.push(format!(
                        "{name}: the output shown after this block is out of date:\n{}\nshown:\n{}\nactual:\n{}",
                        block.text, shown.text, outcome.output
                    ));
                }
            }
        }
        if !updates.is_empty() {
            let mut lines: Vec<String> = markdown.lines().map(str::to_string).collect();
            for (range, text) in updates.into_iter().rev() {
                lines.splice(range, text.lines().map(str::to_string));
            }
            std::fs::write(&page, lines.join("\n") + "\n").unwrap();
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n--------\n"));
    assert!(checked > 20, "only {checked} documentation blocks were found");
}

/// The file a ```kstar file=NAME block defines, if it is one.
fn file_name(block: &Block) -> Option<&str> {
    block.tag.strip_prefix("kstar file=").map(str::trim)
}

/// Runs a documentation block as `demo.kstar`, next to `files`.
fn run_block(text: &str, files: &[(String, String)]) -> common::Outcome {
    if files.is_empty() {
        return run_program("demo.kstar", text);
    }
    let files: Vec<(&str, &str)> = files.iter().map(|(name, text)| (name.as_str(), text.as_str())).collect();
    run_with_files("demo.kstar", text, &files)
}

/// Each section of docs/errors.md explains one code, and its example must
/// really produce that code. A few codes have no example: the formatter's
/// safety net, the nesting limit (an example would be hundreds of brackets),
/// a file that cannot be written, and the catch-all machine error.
#[test]
fn every_error_example_shows_its_code() {
    let page = std::fs::read_to_string(project_dir().join("docs/errors.md")).unwrap().replace("\r\n", "\n");
    let without_example = ["E0112", "E0113", "E0400", "E0607"];
    let mut checked = 0;
    for section in page.split("\n## ").skip(1) {
        let code = &section[..5];
        let blocks = code_blocks(section);
        let files: Vec<(String, String)> = blocks
            .iter()
            .filter_map(|b| file_name(b).map(|name| (name.to_string(), b.text.clone())))
            .collect();
        let example = blocks.into_iter().find(|b| b.tag == "kstar" || b.tag == "kstar-error");
        let Some(example) = example else {
            assert!(without_example.contains(&code), "{code} has no example in docs/errors.md");
            continue;
        };
        let output = run_block(&example.text, &files).output;
        assert!(output.contains(&format!("[{code}]")), "the example for {code} does not show it:\n{output}");
        checked += 1;
    }
    assert!(checked > 40, "only {checked} examples were checked");
}
