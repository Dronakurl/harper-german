//! German for Harper, built as a WebAssembly plugin (see `PLAN.md`).

pub mod clock;
pub mod dialects;
pub mod grammar;
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
