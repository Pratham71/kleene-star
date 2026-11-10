//! The TOC language: from source text to a running program.
//!
//! ```text
//! text --lexer--> tokens --parser--> syntax tree --checker--> (ok) --interpreter--> output
//! ```

pub mod ast;
pub mod builtins;
pub mod check;
pub mod error;
pub mod interp;
pub mod lexer;
pub mod machine;
pub mod parser;
pub mod token;
pub mod value;
