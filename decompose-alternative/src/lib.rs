//! `decompose-alternative` - A Component Model IR for graph traversal
//!
//! This crate parses WebAssembly Components into a structure that preserves
//! the 12 index spaces and allows easy traversal of references.

pub mod ir;
pub mod module;
pub mod parse;

pub use ir::Component;
pub use module::Module;
pub use parse::parse_component;

// Re-export wasmparser and wasm_encoder so consumers use the same versions
pub use wasm_encoder;
pub use wasmparser;
