//! Command-line interface feature.
//!
//! Human-facing commands and script-friendly output belong in this module.

#[path = "impls/cli.rs"]
#[expect(
    clippy::module_inception,
    reason = "keep the CLI implementation path stable"
)]
mod cli;
#[path = "struct/mod.rs"]
mod structs;

pub(crate) use cli::run;
