//! The standard library: modules that come with Kleene Star, reached as
//! `std.<module>.<name>` without an import, or imported like a file with
//! `import std.<module>` and `from std.<module> import <name>`.
//!
//! Each module is a table of `Module`s. The loader asks this file for a
//! module's `Exports`, exactly as it would read them off a checked file, so
//! the checker treats `std.math` and `"utils.kstar"` the same way.

use super::module::Exports;

/// One standard-library module.
pub struct Module {
    /// The name after `std.`.
    pub name: &'static str,
    /// One line about what the module is for.
    pub doc: &'static str,
}

/// Every standard-library module, in alphabetical order.
pub const MODULES: &[Module] = &[];

/// The module called `name`, if there is one.
pub fn module(name: &str) -> Option<&'static Module> {
    MODULES.iter().find(|m| m.name == name)
}

/// What module `name` offers, for the loader and the checker.
pub fn exports(name: &str) -> Option<Exports> {
    module(name).map(|_| Exports::default())
}

/// A hint listing the modules: "its modules are `std.math`, `std.string`".
pub fn modules_hint() -> String {
    if MODULES.is_empty() {
        return "the standard library has no modules yet".into();
    }
    let names: Vec<String> = MODULES.iter().map(|m| format!("`std.{}`", m.name)).collect();
    format!("its modules are {}", names.join(", "))
}
