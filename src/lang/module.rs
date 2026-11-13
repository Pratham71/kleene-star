//! Modules: finding, reading and checking the files a program imports.
//!
//! An `import` statement only names a file. Before the importing file is
//! checked, the `Loader` goes through its import statements and, for each
//! one, works out which file is meant, reads it, parses it, loads *its*
//! imports in turn, and type-checks it. The result is written into the
//! statement's `target`, so the checker and the interpreter never deal with
//! paths at all: they see `ModuleRef::File(id)`.
//!
//! Four ideas keep this simple and safe:
//!
//! * **Every file is loaded once.** Files are remembered by their path, so
//!   a module imported by two files is read, checked (and later run) once.
//! * **Cycles are found while loading.** The loader keeps the chain of files
//!   it is in the middle of loading. Meeting one of them again means the
//!   imports go round in a circle, and the chain is the error message.
//! * **Checking happens up front.** A module is fully checked when it is
//!   loaded, so its mistakes are reported before anything runs, even if
//!   none of its names is ever used. Running it is left to the interpreter,
//!   which does it lazily (see `interp.rs`).
//! * **A module's interface is a table.** What a module offers is its
//!   `Exports`: the type of each top-level name, read off its checker. The
//!   importing file's checker needs nothing else.

use super::ast::{Lib, LibKind, ModuleRef, Stmt, StmtKind, Type};
use super::codes;
use super::error::{did_you_mean, Diag, ErrorKind};
use super::index::Def;
use super::source::SourceMap;
use super::token::{FileId, Span};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

/// How the loader reads a file. On the command line this is
/// `std::fs::read_to_string`; tests and the language server supply their own
/// (an editor knows the text of files that are open but not saved).
pub type Reader = Box<dyn Fn(&Path) -> std::io::Result<String>>;

/// A top-level value of a module: an alphabet, machine, grammar or `let`.
#[derive(Debug, Clone)]
pub struct ExportedValue {
    pub ty: Type,
    /// For an alphabet, its symbols, so machines `over` it can be checked.
    pub alphabet: Option<Vec<char>>,
    /// Its definition, for editors: where it is and how to describe it.
    pub def: Def,
}

/// A function of a module.
#[derive(Debug, Clone)]
pub struct ExportedFn {
    pub params: Vec<Type>,
    pub ret: Type,
    pub def: Def,
}

/// What a module offers to the files that import it. Everything defined at
/// its top level is public; nothing it imported itself is passed on.
#[derive(Debug, Clone, Default)]
pub struct Exports {
    pub values: HashMap<String, ExportedValue>,
    pub functions: HashMap<String, ExportedFn>,
    /// The names this module imported, each with the statement that would
    /// import the same thing directly. They are not exported (there is no
    /// re-exporting), but knowing them gives a better error message.
    pub imported: HashMap<String, String>,
    /// Names whose definition had an error. Using one is not reported
    /// again: the mistake is reported in the module.
    pub broken: HashSet<String>,
}

impl Exports {
    /// Every exported name, for "did you mean" and completion.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.values.keys().chain(self.functions.keys()).map(String::as_str)
    }
}

/// A file that has been loaded.
pub struct Module {
    /// Where it is, after joining it to the importing file's folder.
    pub path: PathBuf,
    pub program: Rc<Vec<Stmt>>,
    /// Its own errors: syntax, types, and imports that failed.
    pub errors: Vec<Diag>,
    /// The files it imports.
    pub deps: Vec<FileId>,
}

pub struct Loader {
    read: Reader,
    /// The main file's path, if it has one (the REPL's input does not).
    main_path: Option<PathBuf>,
    /// The folder the main file's imports are relative to.
    main_dir: PathBuf,
    modules: HashMap<FileId, Module>,
    by_path: HashMap<PathBuf, FileId>,
    /// Where each file is, known from the moment it starts loading (before
    /// it is in `modules`), since its own imports are relative to it.
    paths: HashMap<FileId, PathBuf>,
    /// The files being loaded right now, outermost first: the chain that a
    /// circular import is reported with. Each is a path and a shown name.
    loading: Vec<(PathBuf, String)>,
    /// The interface of every module loaded so far, files and `std` alike.
    pub exports: HashMap<ModuleRef, Rc<Exports>>,
}

impl Loader {
    /// A loader for a program whose main file is at `main_path` (if it has
    /// one). Imports in the main file are relative to its folder, or to the
    /// current folder when there is no path.
    pub fn new(main_path: Option<PathBuf>, read: Reader) -> Loader {
        let main_dir = main_path.as_deref().and_then(Path::parent).map_or_else(PathBuf::new, Path::to_path_buf);
        Loader {
            read,
            main_path: main_path.map(|p| normalize(&p)),
            main_dir,
            modules: HashMap::new(),
            by_path: HashMap::new(),
            paths: HashMap::new(),
            loading: Vec::new(),
            exports: HashMap::new(),
        }
    }

    /// Reads files from the disk.
    pub fn from_disk(main_path: Option<PathBuf>) -> Loader {
        Loader::new(main_path, Box::new(|path| std::fs::read_to_string(path).map(|t| t.replace("\r\n", "\n"))))
    }

    /// Moves the main file, as `Session::set_path` does.
    pub fn set_main_path(&mut self, path: Option<PathBuf>) {
        self.main_dir = path.as_deref().and_then(Path::parent).map_or_else(PathBuf::new, Path::to_path_buf);
        self.main_path = path.map(|p| normalize(&p));
    }

    /// Reads files with `read` from now on.
    pub fn set_reader(&mut self, read: Reader) {
        self.read = read;
    }

    pub fn module(&self, file: FileId) -> Option<&Module> {
        self.modules.get(&file)
    }

    /// The statements of every module loaded so far.
    pub fn programs(&self) -> impl Iterator<Item = (FileId, Rc<Vec<Stmt>>)> + '_ {
        self.modules.iter().map(|(&file, module)| (file, module.program.clone()))
    }

    /// Where a loaded file is.
    pub fn path(&self, file: FileId) -> Option<&Path> {
        match file {
            FileId::MAIN => self.main_path.as_deref(),
            _ => self.paths.get(&file).map(PathBuf::as_path),
        }
    }

    /// Loads every module that the top-level import statements of `program`
    /// (the text of `file`) name, filling in each statement's `target`.
    /// Returns the errors in `file` itself: imports that could not be found
    /// or that go round in a circle. Errors *inside* the modules are kept
    /// with them; `errors_below` collects them.
    pub fn resolve(&mut self, program: &mut [Stmt], file: FileId, sources: &mut SourceMap) -> Vec<Diag> {
        let pushed = file == FileId::MAIN && self.loading.is_empty();
        if pushed {
            let main = self.main_path.clone().unwrap_or_default();
            self.loading.push((main, sources.name(FileId::MAIN).to_string()));
        }
        let mut errors = Vec::new();
        for stmt in program.iter_mut() {
            let (StmtKind::Import { lib, target, .. } | StmtKind::FromImport { lib, target, .. }) = &mut stmt.kind
            else {
                continue;
            };
            match self.find(lib, file, sources) {
                Ok(found) => *target = Some(found),
                Err(error) => errors.push(error),
            }
        }
        if pushed {
            self.loading.pop();
        }
        errors
    }

    /// The module that `lib` names, loading it if it is a file seen for the
    /// first time.
    fn find(&mut self, lib: &Lib, importer: FileId, sources: &mut SourceMap) -> Result<ModuleRef, Diag> {
        let rel = match &lib.kind {
            LibKind::Std(name) => {
                let target = ModuleRef::Std(name.clone());
                if !self.exports.contains_key(&target) {
                    let Some(exports) = super::stdlib::exports(name) else {
                        let mut err = Diag::new(
                            ErrorKind::Name,
                            lib.span,
                            format!("the standard library has no module called `{name}`"),
                        )
                        .code(codes::UNKNOWN_MEMBER);
                        if let Some(close) = did_you_mean(name, super::stdlib::MODULES.iter().map(|m| m.name)) {
                            err = err.hint(format!("did you mean `std.{close}`?"));
                        } else {
                            err = err.hint(super::stdlib::modules_hint());
                        }
                        return Err(err);
                    };
                    self.exports.insert(target.clone(), Rc::new(exports));
                }
                return Ok(target);
            }
            LibKind::File(rel) => rel.replace('\\', "/"),
        };
        // (The parser has checked that the path is relative and ends in
        // `.kstar`.)
        let dir = match importer {
            FileId::MAIN => self.main_dir.clone(),
            other => self.paths[&other].parent().map_or_else(PathBuf::new, Path::to_path_buf),
        };
        let path = normalize(&dir.join(&rel));
        let shown = join_shown(sources.name(importer), &rel);

        if let Some(at) = self.loading.iter().position(|(p, _)| *p == path) {
            let mut chain: Vec<&str> = self.loading[at..].iter().map(|(_, shown)| shown.as_str()).collect();
            chain.push(&self.loading[at].1);
            return Err(Diag::new(ErrorKind::Name, lib.span, format!("circular import: {}", chain.join(" -> ")))
                .code(codes::IMPORT_CYCLE)
                .hint("modules cannot import each other in a circle; move what both need into a third file that both import"));
        }
        if let Some(&id) = self.by_path.get(&path) {
            return Ok(ModuleRef::File(id));
        }

        let text = (self.read)(&path).map_err(|e| {
            let message = if e.kind() == std::io::ErrorKind::NotFound {
                format!("cannot find the file `{shown}`")
            } else {
                format!("cannot read `{shown}`: {e}")
            };
            Diag::new(ErrorKind::Name, lib.span, message).code(codes::IMPORT_NOT_FOUND).hint(
                "a path in an import is relative to the folder of the file that contains the import",
            )
        })?;
        let id = sources.add(&shown, &text);
        self.by_path.insert(path.clone(), id);
        self.paths.insert(id, path.clone());
        self.loading.push((path.clone(), shown));

        let (mut program, mut errors) = super::parse_source(id, sources.text(id), 0, 1, false);
        let syntax_ok = errors.is_empty();
        errors.extend(self.resolve(&mut program, id, sources));
        let deps = targets(&program);
        let mut checker = super::check::Checker::for_module(id, self.exports.clone());
        errors.extend(super::check_parsed(&program, syntax_ok, &mut checker));
        self.exports.insert(ModuleRef::File(id), Rc::new(checker.exports()));

        self.loading.pop();
        self.modules.insert(id, Module { path, program: Rc::new(program), errors, deps });
        Ok(ModuleRef::File(id))
    }

    /// The errors inside every module that `program` imports, directly or
    /// through other modules, each module's once, in the order they were
    /// loaded.
    pub fn errors_below(&self, program: &[Stmt]) -> Vec<Diag> {
        let mut seen = HashSet::new();
        let mut todo = targets(program);
        let mut found = Vec::new();
        while let Some(id) = todo.pop() {
            if !seen.insert(id) {
                continue;
            }
            if let Some(module) = self.modules.get(&id) {
                found.push(id);
                todo.extend(&module.deps);
            }
        }
        found.sort();
        found.iter().flat_map(|id| self.modules[id].errors.iter().cloned()).collect()
    }

    /// For each module that `program` (the main file) reaches, the place in
    /// the main file of the import that first leads to it. An editor shows
    /// a module's errors there.
    pub fn import_sites(&self, program: &[Stmt]) -> HashMap<FileId, Span> {
        let mut sites = HashMap::new();
        for stmt in program {
            let (StmtKind::Import { lib, target: Some(ModuleRef::File(id)), .. }
            | StmtKind::FromImport { lib, target: Some(ModuleRef::File(id)), .. }) = &stmt.kind
            else {
                continue;
            };
            let mut todo = vec![*id];
            while let Some(id) = todo.pop() {
                if sites.contains_key(&id) {
                    continue;
                }
                sites.insert(id, lib.span);
                if let Some(module) = self.modules.get(&id) {
                    todo.extend(&module.deps);
                }
            }
        }
        sites
    }
}

/// The files the import statements of `program` were resolved to.
fn targets(program: &[Stmt]) -> Vec<FileId> {
    program
        .iter()
        .filter_map(|stmt| match &stmt.kind {
            StmtKind::Import { target: Some(ModuleRef::File(id)), .. }
            | StmtKind::FromImport { target: Some(ModuleRef::File(id)), .. } => Some(*id),
            _ => None,
        })
        .collect()
}

/// `path` with `.` and `dir/..` taken out, without asking the file system.
/// Two imports of the same file by different routes (`lib/a.kstar` from
/// the main file, `a.kstar` from inside `lib/`) then give the same path, so
/// the file is loaded once.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// The name to show for `rel`, imported by the file shown as `importer`:
/// the importer's folder joined to `rel`, with forward slashes, the way
/// the command line shows the main file.
pub fn join_shown(importer: &str, rel: &str) -> String {
    let dir = importer.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p| *p != "..") => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_normalized_without_the_file_system() {
        assert_eq!(normalize(Path::new("a/./b/../c.kstar")), PathBuf::from("a/c.kstar"));
        assert_eq!(normalize(Path::new("../x/../y.kstar")), PathBuf::from("../y.kstar"));
        assert_eq!(normalize(Path::new("")), PathBuf::new());
    }

    #[test]
    fn shown_names_follow_the_importer() {
        assert_eq!(join_shown("main.kstar", "utils.kstar"), "utils.kstar");
        assert_eq!(join_shown("examples/main.kstar", "lib/parity.kstar"), "examples/lib/parity.kstar");
        assert_eq!(join_shown("examples/lib/a.kstar", "../b.kstar"), "examples/b.kstar");
        assert_eq!(join_shown("a.kstar", "../b.kstar"), "../b.kstar");
    }

    #[test]
    fn default_names_come_from_the_file_name() {
        let lib = |path: &str| Lib { kind: LibKind::File(path.into()), span: Span::default() };
        assert_eq!(lib("machines/parity.kstar").default_name(), "parity");
        assert_eq!(lib("utils.kstar").default_name(), "utils");
        assert_eq!(Lib { kind: LibKind::Std("math".into()), span: Span::default() }.default_name(), "math");
    }
}
