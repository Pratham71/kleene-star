//! Shared by the integration tests: run a Kleene Star program and capture what a
//! user would see.
#![allow(dead_code)] // each test file uses a different part of this module

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use kstar::Session;

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
    /// The warnings, everything printed, then the test summary, then the
    /// errors if any.
    pub output: String,
    /// True if the program ran to the end and no `test` failed.
    pub ok: bool,
}

/// Where a program's imports come from.
#[derive(Clone)]
pub enum Files {
    /// No other files: an import fails as "not found".
    None,
    /// Files that exist only in the test, by path (`lib/utils.kstar`),
    /// relative to the program.
    Virtual(Vec<(String, String)>),
    /// The disk: the program is at this path, and imports are relative to it.
    Disk(PathBuf),
}

/// Options for `run_with`.
#[derive(Clone)]
pub struct Run {
    pub files: Files,
    pub eager_imports: bool,
}

pub fn run_program(shown_name: &str, text: &str) -> Outcome {
    run_with(shown_name, text, Run { files: Files::None, eager_imports: false })
}

/// Runs `text` as the file `shown_name`, which lives on disk at `path`.
pub fn run_at(path: &Path, shown_name: &str, text: &str) -> Outcome {
    run_with(shown_name, text, Run { files: Files::Disk(path.to_path_buf()), eager_imports: false })
}

/// Runs `text` as the file `shown_name`, with other files it can import.
pub fn run_with_files(shown_name: &str, text: &str, files: &[(&str, &str)]) -> Outcome {
    let files = files.iter().map(|(name, text)| (name.to_string(), text.to_string())).collect();
    run_with(shown_name, text, Run { files: Files::Virtual(files), eager_imports: false })
}

pub fn run_with(shown_name: &str, text: &str, options: Run) -> Outcome {
    let (shown_name, text) = (shown_name.to_string(), text.replace("\r\n", "\n"));
    kstar::with_big_stack(move || {
        let capture = Capture::default();
        let mut session = Session::new(&shown_name, Box::new(capture.clone()), scratch_dir(), false);
        session.eager_imports(options.eager_imports);
        match options.files {
            Files::None => session.set_reader(Box::new(|path| {
                Err(std::io::Error::new(std::io::ErrorKind::NotFound, path.display().to_string()))
            })),
            Files::Virtual(files) => session.set_reader(Box::new(move |path| {
                let wanted = path.to_string_lossy().replace('\\', "/");
                files
                    .iter()
                    .find(|(name, _)| *name == wanted)
                    .map(|(_, text)| text.clone())
                    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, wanted))
            })),
            Files::Disk(path) => session.set_path(&path),
        }
        let result = session.run(&text);
        // Warnings come first, as on the command line, where they are shown
        // before the program runs.
        let mut output: String = session.take_warnings().iter().map(|w| format!("{w}\n\n")).collect();
        output.push_str(&String::from_utf8(capture.0.borrow().clone()).unwrap());
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
    let dir = std::env::temp_dir().join("kstar-tests");
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

/// A small, seeded random number generator (xorshift64*), so that tests
/// with random inputs are repeatable: the same seed gives the same inputs.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n` (`n` must not be 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// True with probability `percent` in 100.
    pub fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// The seed for random tests: `KSTAR_SEED` if set, so a failure can be
/// reproduced, otherwise a fixed one.
pub fn seed() -> u64 {
    std::env::var("KSTAR_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(20_261_111)
}
