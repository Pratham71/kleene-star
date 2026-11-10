//! Shared by the integration tests: run a TOC program and capture what a
//! user would see.
#![allow(dead_code)] // each test file uses a different part of this module

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use toc::Session;

/// A writer that keeps what is written, so the test can read it back.
#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct Outcome {
    /// Everything printed, then the test summary, then the error if any.
    pub output: String,
    /// True if the program ran to the end and no `test` failed.
    pub ok: bool,
}

pub fn run_program(shown_name: &str, text: &str) -> Outcome {
    let (shown_name, text) = (shown_name.to_string(), text.replace("\r\n", "\n"));
    toc::with_big_stack(move || {
        let capture = Capture::default();
        let mut session = Session::new(&shown_name, Box::new(capture.clone()), scratch_dir(), false);
        let result = session.run(&text);
        let mut output = String::from_utf8(capture.0.borrow().clone()).unwrap();
        let (passed, failed) = session.tests();
        if passed + failed > 0 {
            output.push_str(&format!("tests: {passed} passed, {failed} failed\n"));
        }
        let ok = result.is_ok() && failed == 0;
        if let Err(message) = result {
            output.push_str(&message);
            output.push('\n');
        }
        Outcome { output, ok }
    })
}

/// Where `draw` statements in test programs put their files.
fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("toc-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// All files in `dir` with the given extension, sorted.
pub fn files_in(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect();
    files.sort();
    files
}

pub fn project_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
