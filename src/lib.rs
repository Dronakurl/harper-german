//! German for Harper, built as a WebAssembly plugin (see https://github.com/Dronakurl/harper-german/issues/1).

pub mod clock;
pub mod dialects;
pub mod grammar;
pub mod language_detection;
pub mod lexing;
pub mod linting;
pub mod module;
pub mod parsers;
pub mod plugin;
pub mod spell;
#[cfg(test)]
pub mod testing;

#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
mod wasm_support;
