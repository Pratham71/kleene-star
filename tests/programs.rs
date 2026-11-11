//! Keeps the examples and the documentation honest.
//!
//! * Every program in `examples/` must run without errors or failed tests.
//! * Every code block in the docs marked ```kstar must do the same, every block
//!   marked ```kstar-error must be rejected, and the output shown underneath
//!   must be what the program really prints. So if the language changes,
//!   out-of-date documentation fails the build. After a deliberate change,
//!   `UPDATE_DOCS=1 cargo test --test programs` rewrites those output blocks.

mod common;
use common::{files_in, project_dir, run_program};

#[test]
fn examples_run_cleanly() {
    let examples = files_in(&project_dir().join("examples"), "kstar");
    assert!(!examples.is_empty(), "no examples found");
    for example in examples {
        let name = example.file_name().unwrap().to_string_lossy().to_string();
        let outcome = run_program(&name, &std::fs::read_to_string(&example).unwrap());
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
        for (i, block) in blocks.iter().enumerate() {
            let should_pass = match block.tag.as_str() {
                "kstar" => true,
                "kstar-error" => false,
                _ => continue,
            };
            checked += 1;
            let outcome = run_program("demo.kstar", &block.text);
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

/// Each section of docs/errors.md explains one code, and its example must
/// really produce that code. A few codes have no example: the formatter's
/// safety net, a file that cannot be written, and the catch-all machine error.
#[test]
fn every_error_example_shows_its_code() {
    let page = std::fs::read_to_string(project_dir().join("docs/errors.md")).unwrap().replace("\r\n", "\n");
    let without_example = ["E0112", "E0400", "E0607"];
    let mut checked = 0;
    for section in page.split("\n## ").skip(1) {
        let code = &section[..5];
        let example = code_blocks(section).into_iter().find(|b| b.tag == "kstar" || b.tag == "kstar-error");
        let Some(example) = example else {
            assert!(without_example.contains(&code), "{code} has no example in docs/errors.md");
            continue;
        };
        let output = run_program("demo.kstar", &example.text).output;
        assert!(output.contains(&format!("[{code}]")), "the example for {code} does not show it:\n{output}");
        checked += 1;
    }
    assert!(checked > 40, "only {checked} examples were checked");
}
