//! Keeps the examples and the documentation honest.
//!
//! * Every program in `examples/` must run without errors or failed tests.
//! * Every code block in the docs marked ```toc must do the same, every block
//!   marked ```toc-error must be rejected, and the output shown underneath
//!   must be what the program really prints. So if the language changes,
//!   out-of-date documentation fails the build.

mod common;
use common::{files_in, project_dir, run_program};

#[test]
fn examples_run_cleanly() {
    let examples = files_in(&project_dir().join("examples"), "toc");
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
}

fn code_blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut current: Option<Block> = None;
    let mut prose_since_last_block = true;
    for line in markdown.lines() {
        let fence = line.trim_start().strip_prefix("```");
        match (&mut current, fence) {
            (Some(_), Some(_)) => {
                blocks.push(current.take().unwrap());
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
                });
            }
            (None, None) => prose_since_last_block |= !line.trim().is_empty(),
        }
    }
    blocks
}

/// Every ```toc block must run cleanly and every ```toc-error block must
/// fail. When a ```text block comes straight after either, it must be
/// exactly what the program prints.
#[test]
fn documentation_code_runs() {
    let root = project_dir();
    let mut pages = vec![root.join("README.md")];
    pages.extend(files_in(&root.join("docs"), "md"));
    pages.extend(files_in(&root.join("docs/tutorial"), "md"));

    let mut problems = Vec::new();
    let mut checked = 0;
    for page in pages {
        let name = page.strip_prefix(root).unwrap().display().to_string();
        let blocks = code_blocks(&std::fs::read_to_string(&page).unwrap());
        for (i, block) in blocks.iter().enumerate() {
            let should_pass = match block.tag.as_str() {
                "toc" => true,
                "toc-error" => false,
                _ => continue,
            };
            checked += 1;
            let outcome = run_program("demo.toc", &block.text);
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
                    problems.push(format!(
                        "{name}: the output shown after this block is out of date:\n{}\nshown:\n{}\nactual:\n{}",
                        block.text, shown.text, outcome.output
                    ));
                }
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n--------\n"));
    assert!(checked > 20, "only {checked} documentation blocks were found");
}
