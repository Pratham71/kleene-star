//! `std.io`: reading and writing text files next to the program, such as a
//! word list to test a machine on, or a machine saved with `to_json`.
//!
//! A program may only touch files in its own folder (and the folders under
//! it), plus folders the person running it allows with
//! `kstar run --allow-path DIR`. Everything else is refused, so running a
//! program someone sent you cannot read or change the rest of your files.
//! The rules, checked on every call:
//!
//! * A path is relative to the program's folder (an absolute path is taken
//!   as it is). It is resolved on the disk, following `..` and symbolic
//!   links, and the file it names must be inside an allowed folder.
//! * No part of the path below that folder may be hidden (start with `.`),
//!   so `.git` and `.env` are out of reach, nor contain `:` (an alternate
//!   data stream on Windows), nor be a Windows device name such as `CON`.
//! * Writing creates or replaces only text files whose extension is one of
//!   `WRITABLE`, so a program cannot replace another program or a script.
//!   Folders on the way are created.
//! * Files are UTF-8 text. A file read may be at most `MAX_READ` bytes, a
//!   write at most `MAX_WRITE`; one run may write at most `MAX_WRITTEN`
//!   bytes in all, in at most `MAX_WRITES` calls.
//!
//! The interpreter owns the allowed folders (a `Files`) and lends them to
//! this module for the length of each call (`with_files`), since a library
//! function only gets its arguments. Without them (a program that is not a
//! file, such as a code block in the docs) every function fails with
//! E0618.

use super::{str_arg, Failure, Function, Module, Ty};
use crate::lang::codes;
use crate::lang::value::Value;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

/// The most bytes a file may have to be read.
pub const MAX_READ: u64 = 1 << 20;
/// The most bytes one call may write.
pub const MAX_WRITE: usize = 1 << 20;
/// The most bytes one run may write in all.
pub const MAX_WRITTEN: u64 = 16 << 20;
/// The most writes one run may make.
pub const MAX_WRITES: usize = 1000;
/// The most files `files` lists.
pub const MAX_LISTED: usize = 10_000;
/// The extensions of the files a program may write.
pub const WRITABLE: &[&str] = &["csv", "dot", "html", "json", "log", "md", "svg", "tsv", "txt"];

const STR_SET: Ty = Ty::Set(&Ty::Str);

pub const MODULE: Module = Module {
    name: "io",
    doc: "Reading and writing text files in the program's folder, such as word lists and machines saved as JSON.",
    functions: &[
        Function { name: "append_text", params: &[("path", Ty::Str), ("text", Ty::Str)], ret: Ty::Int, doc: "Adds `text` to the end of the file at `path`, creating it if needed, and gives the number of bytes written.", run: append_text },
        Function { name: "exists", params: &[("path", Ty::Str)], ret: Ty::Bool, doc: "Whether there is a file at `path`; a path outside the allowed folders is an error, not `false`.", run: exists },
        Function { name: "files", params: &[("folder", Ty::Str)], ret: STR_SET, doc: "The paths of the files directly in `folder` (`\".\"` for the program's own), leaving out hidden ones.", run: files },
        Function { name: "read_lines", params: &[("path", Ty::Str)], ret: STR_SET, doc: "The lines of the file at `path` that are not empty, as a set.", run: read_lines },
        Function { name: "read_text", params: &[("path", Ty::Str)], ret: Ty::Str, doc: "The whole file at `path`, which must be UTF-8 text of at most 1 MiB; line ends become `\\n`.", run: read_text },
        Function { name: "read_words", params: &[("path", Ty::Str)], ret: STR_SET, doc: "The words of the file at `path`, separated by spaces or new lines, as a set; a `#` starts a comment to the end of its line.", run: read_words },
        Function { name: "write_lines", params: &[("path", Ty::Str), ("lines", STR_SET)], ret: Ty::Int, doc: "Writes the members of `lines` to the file at `path`, one per line in the set's order, and gives the number of bytes written.", run: write_lines },
        Function { name: "write_text", params: &[("path", Ty::Str), ("text", Ty::Str)], ret: Ty::Int, doc: "Creates or replaces the file at `path` with `text`, and gives the number of bytes written.", run: write_text },
    ],
    explain: None,
};

/// The folders a run may use, and what it has written so far.
#[derive(Debug)]
pub struct Files {
    /// Resolved folders; the first is the program's own, which relative
    /// paths start from.
    roots: Vec<PathBuf>,
    /// The same folders as they were given (made absolute, but not
    /// resolved on the disk), so an absolute path written the same way
    /// passes the check `within_by_text` makes before touching the disk.
    given: Vec<PathBuf>,
    written: Cell<u64>,
    writes: Cell<usize>,
}

impl Files {
    /// The files of a program in `folder`. `None` if the folder does not
    /// exist.
    pub fn new(folder: &Path) -> Option<Files> {
        let folder = if folder.as_os_str().is_empty() { Path::new(".") } else { folder };
        let root = folder.canonicalize().ok().filter(|p| p.is_dir())?;
        Some(Files { roots: vec![root], given: vec![absolute(folder)], written: Cell::new(0), writes: Cell::new(0) })
    }

    /// Also allows the folder `dir` (`kstar run --allow-path`), relative to
    /// the current folder.
    pub fn allow(&mut self, dir: &Path) -> Result<(), String> {
        let root = dir.canonicalize().map_err(|e| format!("cannot allow {}: {e}", dir.display()))?;
        if !root.is_dir() {
            return Err(format!("cannot allow {}: it is not a folder", dir.display()));
        }
        self.roots.push(root);
        self.given.push(absolute(dir));
        Ok(())
    }
}

thread_local! {
    /// The `Files` of the run that is calling a `std.io` function.
    static CURRENT: RefCell<Option<Rc<Files>>> = const { RefCell::new(None) };
}

/// Runs `f`, a call of a `std.io` function, with `files` as the folders it
/// may use.
pub fn with_files<T>(files: Option<Rc<Files>>, f: impl FnOnce() -> T) -> T {
    let before = CURRENT.with(|c| c.replace(files));
    let result = f();
    CURRENT.with(|c| c.replace(before));
    result
}

fn current() -> Result<Rc<Files>, Failure> {
    CURRENT.with(|c| c.borrow().clone()).ok_or_else(|| Failure {
        code: codes::PATH_NOT_ALLOWED,
        arg: None,
        message: "this program cannot use files: it is not running from a file".into(),
        hint: "`std.io` works in programs run from a file, such as `kstar run program.kstar`, and in the REPL".into(),
    })
}

fn not_allowed(path: &str, why: String, hint: String) -> Failure {
    Failure { code: codes::PATH_NOT_ALLOWED, arg: Some(0), message: format!("`{path}` {why}"), hint }
}

fn io_failure(path: &str, what: &str, error: std::io::Error) -> Failure {
    Failure { code: codes::IO, arg: Some(0), message: format!("cannot {what} `{path}`: {error}"), hint: String::new() }
}

/// `path` made absolute against the current folder, by its text alone.
fn absolute(path: &Path) -> PathBuf {
    lexical(&std::env::current_dir().unwrap_or_default().join(path))
}

/// `path` with `.` and `..` worked out from its text alone.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Windows reserves these names (with any extension) for devices.
fn is_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit())
}

/// Whether `path`, with `.` and `..` already worked out, is inside one of
/// the folders by its text alone. `resolve` asks this before it touches the
/// disk, since on Windows even looking at a network path such as
/// `\\host\share` connects to that computer and sends it the user's
/// credentials. The folders are resolved paths, which on Windows begin
/// `\\?\C:\`; a path typed as `C:\...` is compared without that.
fn within_by_text(files: &Files, path: &Path) -> bool {
    files.given.iter().any(|given| path.starts_with(given))
        || files.roots.iter().any(|root| {
        if path.starts_with(root) {
            return true;
        }
        let text = root.to_string_lossy();
        match text.strip_prefix(r"\\?\") {
            Some(plain) if !plain.starts_with("UNC") => path.starts_with(plain),
            _ => false,
        }
    })
}

/// Where `path` (argument 0 of a call) really is, after checking that the
/// program may use it. The file need not exist.
fn resolve(files: &Files, path: &str) -> Result<PathBuf, Failure> {
    if path.is_empty() || path.chars().any(|c| c.is_control()) {
        return Err(not_allowed(path, "is not a file name".into(), "give a path such as \"words.txt\" or \"data/words.txt\"".into()));
    }
    for part in Path::new(path).components() {
        let Component::Normal(name) = part else { continue };
        let name = name.to_string_lossy();
        if name.contains(':') || is_device_name(&name) {
            return Err(not_allowed(path, format!("has `{name}` in it, which is not an ordinary file name"), "use plain names such as \"words.txt\"".into()));
        }
    }
    let wanted = lexical(&files.roots[0].join(path));
    if !within_by_text(files, &wanted) {
        return Err(outside(path));
    }
    // The longest part of the path that exists, resolved on the disk (which
    // follows symbolic links), and the names after it.
    let mut base = wanted.clone();
    let mut rest = Vec::new();
    while std::fs::symlink_metadata(&base).is_err() {
        match base.file_name() {
            Some(name) => rest.push(name.to_os_string()),
            None => break,
        }
        base.pop();
    }
    let mut real = base.canonicalize().map_err(|e| io_failure(path, "use", e))?;
    real.extend(rest.iter().rev());
    let Some(root) = files.roots.iter().find(|root| real.starts_with(root)) else {
        return Err(outside(path));
    };
    for part in real.strip_prefix(root).unwrap_or(Path::new("")).components() {
        let name = part.as_os_str().to_string_lossy();
        if name.starts_with('.') {
            return Err(not_allowed(path, format!("goes through `{name}`, which is hidden"), "hidden files and folders, such as `.git`, are never read or written".into()));
        }
    }
    Ok(real)
}

fn outside(path: &str) -> Failure {
    not_allowed(
        path,
        "is outside the program's folder".into(),
        "a program may only use files in its own folder; to allow another, run it with `kstar run --allow-path FOLDER`".into(),
    )
}

/// A failure for `path`, which is there but is not an ordinary file (a
/// folder, a pipe or a device, which could also wait for ever).
fn not_a_file(path: &str) -> Failure {
    Failure { code: codes::IO, arg: Some(0), message: format!("`{path}` is not a file"), hint: "to list a folder, use `std.io.files`".into() }
}

/// The file at argument 0, resolved, for reading: it must exist.
fn readable(files: &Files, path: &str) -> Result<std::fs::File, Failure> {
    let real = resolve(files, path)?;
    if !real.exists() {
        return Err(Failure {
            code: codes::IO,
            arg: Some(0),
            message: format!("there is no file `{path}`"),
            hint: "paths are relative to the program's folder; `std.io.exists` says whether a file is there".into(),
        });
    }
    // Looked at before opening: opening a pipe waits for a writer.
    if !real.is_file() {
        return Err(not_a_file(path));
    }
    let file = std::fs::File::open(&real).map_err(|e| io_failure(path, "read", e))?;
    let meta = file.metadata().map_err(|e| io_failure(path, "read", e))?;
    if !meta.is_file() {
        return Err(not_a_file(path));
    }
    if meta.len() > MAX_READ {
        return Err(too_large(format!("`{path}` has {} bytes, more than the 1 MiB a program may read at once", meta.len())));
    }
    Ok(file)
}

fn too_large(message: String) -> Failure {
    Failure { code: codes::FILE_TOO_LARGE, arg: None, message, hint: "split the data into smaller files".into() }
}

/// The text of the file at `path`, with `\r\n` turned into `\n`.
fn read(path: &str) -> Result<String, Failure> {
    let files = current()?;
    let mut bytes = Vec::new();
    readable(&files, path)?.take(MAX_READ + 1).read_to_end(&mut bytes).map_err(|e| io_failure(path, "read", e))?;
    if bytes.len() as u64 > MAX_READ {
        return Err(too_large(format!("`{path}` has more than the 1 MiB a program may read at once")));
    }
    let text = String::from_utf8(bytes).map_err(|_| Failure {
        code: codes::IO,
        arg: Some(0),
        message: format!("`{path}` is not UTF-8 text"),
        hint: "save the file as UTF-8".into(),
    })?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    Ok(text.replace("\r\n", "\n"))
}

/// Writes `text` to `path`, replacing the file or adding to its end.
fn write(path: &str, text: &str, append: bool) -> Result<Value, Failure> {
    let files = current()?;
    let real = resolve(&files, path)?;
    let extension = real.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if !WRITABLE.contains(&extension.as_str()) {
        let list: Vec<String> = WRITABLE.iter().map(|e| format!(".{e}")).collect();
        return Err(not_allowed(path, "is not a text file a program may write".into(), format!("a program may write files ending in {}", list.join(", "))));
    }
    if text.len() > MAX_WRITE {
        return Err(too_large(format!("{} bytes is more than the 1 MiB one call may write", text.len())));
    }
    if files.writes.get() >= MAX_WRITES || files.written.get() + text.len() as u64 > MAX_WRITTEN {
        return Err(too_large(format!("a run may write at most 16 MiB, in at most {MAX_WRITES} calls")));
    }
    if real.exists() && !real.is_file() {
        return Err(not_a_file(path));
    }
    if let Some(parent) = real.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_failure(path, "write", e))?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true);
    if append {
        options.append(true);
    } else {
        options.write(true).truncate(true);
    }
    let mut file = options.open(&real).map_err(|e| io_failure(path, "write", e))?;
    file.write_all(text.as_bytes()).map_err(|e| io_failure(path, "write", e))?;
    files.writes.set(files.writes.get() + 1);
    files.written.set(files.written.get() + text.len() as u64);
    Ok(Value::Int(text.len() as i64))
}

fn str_set(items: impl IntoIterator<Item = String>) -> Value {
    Value::Set(Rc::new(items.into_iter().map(|s| Value::str(&s)).collect::<BTreeSet<_>>()))
}

fn read_text(args: &[Value]) -> Result<Value, Failure> {
    read(str_arg(args, 0)).map(|text| Value::str(&text))
}

fn read_lines(args: &[Value]) -> Result<Value, Failure> {
    let text = read(str_arg(args, 0))?;
    Ok(str_set(text.lines().filter(|l| !l.is_empty()).map(str::to_string)))
}

fn read_words(args: &[Value]) -> Result<Value, Failure> {
    let text = read(str_arg(args, 0))?;
    let words = text.lines().flat_map(|line| line.split('#').next().unwrap_or("").split_whitespace()).map(str::to_string);
    Ok(str_set(words))
}

fn write_text(args: &[Value]) -> Result<Value, Failure> {
    write(str_arg(args, 0), str_arg(args, 1), false)
}

fn append_text(args: &[Value]) -> Result<Value, Failure> {
    write(str_arg(args, 0), str_arg(args, 1), true)
}

fn write_lines(args: &[Value]) -> Result<Value, Failure> {
    let Value::Set(lines) = &args[1] else { unreachable!("the checker allowed {:?} as a Set", args[1]) };
    let mut text = String::new();
    for line in lines.iter() {
        let Value::Str(line) = line else { unreachable!("the checker allowed {line:?} as a Str") };
        text.push_str(line);
        text.push('\n');
    }
    write(str_arg(args, 0), &text, false)
}

fn exists(args: &[Value]) -> Result<Value, Failure> {
    let files = current()?;
    Ok(Value::Bool(resolve(&files, str_arg(args, 0))?.is_file()))
}

fn files(args: &[Value]) -> Result<Value, Failure> {
    let files = current()?;
    let folder = str_arg(args, 0);
    let real = resolve(&files, folder)?;
    if !real.is_dir() {
        return Err(Failure { code: codes::IO, arg: Some(0), message: format!("there is no folder `{folder}`"), hint: "paths are relative to the program's folder".into() });
    }
    let entries = std::fs::read_dir(&real).map_err(|e| io_failure(folder, "list", e))?;
    let prefix = folder.trim_end_matches(['/', '\\']);
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| io_failure(folder, "list", e))?;
        let Ok(name) = entry.file_name().into_string() else { continue };
        if name.starts_with('.') || !entry.path().is_file() {
            continue;
        }
        if found.len() == MAX_LISTED {
            return Err(too_large(format!("`{folder}` has more than {MAX_LISTED} files")));
        }
        found.push(if prefix.is_empty() || prefix == "." { name } else { format!("{prefix}/{name}") });
    }
    Ok(str_set(found))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A new, empty folder for one test.
    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kstar-io-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn call(files: &Files, name: &str, args: &[Value]) -> Result<Value, Failure> {
        let files = Rc::new(Files { roots: files.roots.clone(), given: files.given.clone(), written: Cell::new(0), writes: Cell::new(0) });
        with_files(Some(files), || (super::super::function("io", name).unwrap().run)(args))
    }

    fn s(text: &str) -> Value {
        Value::str(text)
    }

    #[test]
    fn writes_and_reads_back() {
        let dir = folder("round");
        let files = Files::new(&dir).unwrap();
        assert_eq!(call(&files, "write_text", &[s("out/a.txt"), s("ab\ncd")]), Ok(Value::Int(5)));
        assert_eq!(call(&files, "append_text", &[s("out/a.txt"), s("\nab")]), Ok(Value::Int(3)));
        assert_eq!(call(&files, "read_text", &[s("out/a.txt")]), Ok(s("ab\ncd\nab")));
        assert_eq!(call(&files, "read_lines", &[s("out/a.txt")]), Ok(str_set(["ab".into(), "cd".into()])));
        assert_eq!(call(&files, "exists", &[s("out/a.txt")]), Ok(Value::Bool(true)));
        assert_eq!(call(&files, "exists", &[s("out/b.txt")]), Ok(Value::Bool(false)));
        assert_eq!(call(&files, "files", &[s("out")]), Ok(str_set(["out/a.txt".into()])));
    }

    #[test]
    fn reads_words_and_line_ends() {
        let dir = folder("words");
        std::fs::write(dir.join("w.txt"), "\u{feff}# words\r\n01 10  # two\r\n\r\n0110\r\n").unwrap();
        let files = Files::new(&dir).unwrap();
        assert_eq!(call(&files, "read_words", &[s("w.txt")]), Ok(str_set(["01".into(), "10".into(), "0110".into()])));
        assert_eq!(call(&files, "read_text", &[s("./w.txt")]), Ok(s("# words\n01 10  # two\n\n0110\n")));
        std::fs::write(dir.join("bad.txt"), [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(call(&files, "read_text", &[s("bad.txt")]).unwrap_err().code, codes::IO);
    }

    #[test]
    fn stays_inside_the_folder() {
        let dir = folder("sandbox");
        let inner = dir.join("program");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(dir.join("secret.txt"), "no").unwrap();
        let files = Files::new(&inner).unwrap();
        let refused = |name: &str, args: &[Value]| call(&files, name, args).unwrap_err().code;
        for path in ["../secret.txt", "sub/../../secret.txt", dir.join("secret.txt").to_str().unwrap()] {
            assert_eq!(refused("read_text", &[s(path)]), codes::PATH_NOT_ALLOWED, "{path}");
            assert_eq!(refused("exists", &[s(path)]), codes::PATH_NOT_ALLOWED, "{path}");
            assert_eq!(refused("write_text", &[s(path), s("x")]), codes::PATH_NOT_ALLOWED, "{path}");
        }
        assert_eq!(refused("files", &[s("..")]), codes::PATH_NOT_ALLOWED);
        // Hidden names, odd names and files that are not text.
        for path in [".env", ".git/hooks/pre-commit.txt", "a/.hidden/b.txt", "CON", "nul.txt", "a.txt:stream", "", "a\nb.txt"] {
            assert_eq!(refused("write_text", &[s(path), s("x")]), codes::PATH_NOT_ALLOWED, "{path:?}");
        }
        for path in ["run.kstar", "run.sh", "run.bat", "Makefile", "lib.rs"] {
            assert_eq!(refused("write_text", &[s(path), s("x")]), codes::PATH_NOT_ALLOWED, "{path}");
        }
        assert!(!dir.join("program/.git").exists() && !dir.join("program/run.kstar").exists());
        // Once allowed, the other folder works too.
        let mut files = Files::new(&inner).unwrap();
        files.allow(&dir).unwrap();
        assert_eq!(call(&files, "read_text", &[s("../secret.txt")]), Ok(s("no")));
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_links_out() {
        let dir = folder("links");
        let inner = dir.join("program");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(dir.join("secret.txt"), "no").unwrap();
        std::os::unix::fs::symlink(dir.join("secret.txt"), inner.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(&dir, inner.join("up")).unwrap();
        let files = Files::new(&inner).unwrap();
        for path in ["link.txt", "up/secret.txt", "up/new.txt"] {
            assert_eq!(call(&files, "read_text", &[s(path)]).unwrap_err().code, codes::PATH_NOT_ALLOWED, "{path}");
            assert_eq!(call(&files, "write_text", &[s(path), s("x")]).unwrap_err().code, codes::PATH_NOT_ALLOWED, "{path}");
        }
        assert_eq!(std::fs::read_to_string(dir.join("secret.txt")).unwrap(), "no");
    }

    /// A path that is outside the folders by its text alone is refused
    /// before the disk is asked anything: on Windows, even looking at a
    /// network path connects to that computer and sends it the user's
    /// credentials.
    #[test]
    fn refuses_by_the_text_first() {
        let dir = folder("text");
        let files = Files::new(&dir).unwrap();
        assert!(within_by_text(&files, &dir.join("a.txt")));
        assert!(within_by_text(&files, &files.roots[0].join("sub").join("a.txt")));
        assert!(!within_by_text(&files, &lexical(&dir.join("..").join("x.txt"))));
        // Outside Windows a backslash is part of a file name, not a network path.
        let unc = if cfg!(windows) { r"\\attacker.invalid\share\x.txt" } else { "//attacker.invalid/share/x.txt" };
        assert!(!within_by_text(&files, Path::new(unc)));
        for path in [unc, "//attacker.invalid/share/x.txt"] {
            for name in ["read_text", "exists", "files"] {
                assert_eq!(call(&files, name, &[s(path)]).unwrap_err().code, codes::PATH_NOT_ALLOWED, "{name} {path}");
            }
            assert_eq!(call(&files, "write_text", &[s(path), s("x")]).unwrap_err().code, codes::PATH_NOT_ALLOWED, "{path}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn does_not_open_pipes() {
        let dir = folder("pipe");
        assert!(std::process::Command::new("mkfifo").arg(dir.join("p.txt")).status().unwrap().success());
        let files = Files::new(&dir).unwrap();
        assert_eq!(call(&files, "read_text", &[s("p.txt")]).unwrap_err().code, codes::IO);
        assert_eq!(call(&files, "write_text", &[s("p.txt"), s("x")]).unwrap_err().code, codes::IO);
    }

    #[test]
    fn limits_sizes() {
        let dir = folder("limits");
        std::fs::write(dir.join("big.txt"), vec![b'a'; MAX_READ as usize + 1]).unwrap();
        let files = Files::new(&dir).unwrap();
        assert_eq!(call(&files, "read_text", &[s("big.txt")]).unwrap_err().code, codes::FILE_TOO_LARGE);
        let big = "a".repeat(MAX_WRITE + 1);
        assert_eq!(call(&files, "write_text", &[s("out.txt"), s(&big)]).unwrap_err().code, codes::FILE_TOO_LARGE);
        let files = Rc::new(Files::new(&dir).unwrap());
        files.writes.set(MAX_WRITES);
        let run = with_files(Some(files), || write_text(&[s("out.txt"), s("x")]));
        assert_eq!(run.unwrap_err().code, codes::FILE_TOO_LARGE);
    }

    #[test]
    fn needs_a_folder() {
        assert_eq!(read_text(&[s("a.txt")]).unwrap_err().code, codes::PATH_NOT_ALLOWED);
        assert!(Files::new(Path::new("no such folder here")).is_none());
    }
}
