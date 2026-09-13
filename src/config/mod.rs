#[path = "impls/config.rs"]
#[expect(
    clippy::module_inception,
    reason = "keep the config implementation path stable"
)]
mod config;
#[path = "impls/finalshell.rs"]
mod finalshell;
#[path = "struct/mod.rs"]
mod structs;

pub(crate) use config::*;
pub(crate) use structs::*;
