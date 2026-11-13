//! The source map: the name and text of every file a program is made of.
//!
//! A `Span` says *where* something is with byte offsets and a `FileId`. To
//! show an error, those offsets have to be turned back into a line of text
//! and a file name, and that is what this table is for. Every stage that
//! reports errors (the lexer, parser, checker and interpreter) only records
//! spans; the source map is consulted once, at the end, when an error is
//! rendered.
//!
//! The file being run is always `FileId::MAIN`. In the REPL, every entry is
//! appended to the main file's text, so an error can quote a line typed
//! several entries ago.

use super::token::FileId;

/// One file: what to call it in messages, and its text.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// The name shown in messages, like `examples/01_dfa.kstar` or `<repl>`.
    pub name: String,
    /// The text, with Windows line endings already turned into `\n`.
    pub text: String,
}

/// Every file of one program, numbered by `FileId`.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    /// A map whose main file (`FileId::MAIN`) is called `name` and holds `text`.
    pub fn new(name: &str, text: &str) -> SourceMap {
        let mut map = SourceMap::default();
        map.add(name, text);
        map
    }

    /// Adds a file and returns its id. Ids are handed out in order, so the
    /// first file added is `FileId::MAIN`.
    pub fn add(&mut self, name: &str, text: &str) -> FileId {
        self.files.push(SourceFile { name: name.to_string(), text: text.to_string() });
        FileId((self.files.len() - 1) as u32)
    }

    /// The file with this id. A span from somewhere unexpected (a test, or
    /// a default span) may name a file the map does not have; it is shown
    /// as an empty file rather than causing a crash.
    fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    /// The name of a file, as shown in messages.
    pub fn name(&self, id: FileId) -> &str {
        self.file(id).map_or("<unknown>", |f| f.name.as_str())
    }

    /// The text of a file.
    pub fn text(&self, id: FileId) -> &str {
        self.file(id).map_or("", |f| f.text.as_str())
    }

    /// Adds text to the end of a file, as the REPL does with each entry.
    /// Returns the byte offset where the new text starts and its first line,
    /// which is where the lexer should start reading.
    pub fn append(&mut self, id: FileId, more: &str) -> (usize, u32) {
        let file = &mut self.files[id.0 as usize];
        let from = file.text.len();
        let first_line = file.text.matches('\n').count() as u32 + 1;
        file.text.push_str(more);
        (from, first_line)
    }

    /// How many files there are.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// True if no file has been added.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_numbered_in_order() {
        let mut map = SourceMap::new("main.kstar", "print 1\n");
        let other = map.add("lib.kstar", "let x = 2\n");
        assert_eq!(other, FileId(1));
        assert_eq!(map.name(FileId::MAIN), "main.kstar");
        assert_eq!(map.text(other), "let x = 2\n");
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn appending_reports_where_the_new_text_starts() {
        let mut map = SourceMap::new("<repl>", "");
        assert_eq!(map.append(FileId::MAIN, "let a = 1\n"), (0, 1));
        assert_eq!(map.append(FileId::MAIN, "print a\n"), (10, 2));
        assert_eq!(map.text(FileId::MAIN), "let a = 1\nprint a\n");
    }

    #[test]
    fn an_unknown_file_is_empty() {
        let map = SourceMap::new("main.kstar", "");
        assert_eq!(map.name(FileId(7)), "<unknown>");
        assert_eq!(map.text(FileId(7)), "");
    }
}
