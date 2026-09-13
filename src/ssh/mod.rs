#[path = "impls/docker_completion.rs"]
mod docker_completion;
#[path = "impls/known_hosts.rs"]
pub(crate) mod known_hosts;
#[path = "impls/ppk.rs"]
pub(crate) mod ppk;
#[path = "impls/proxy.rs"]
pub(crate) mod proxy;
#[path = "impls/ssh.rs"]
#[expect(
    clippy::module_inception,
    reason = "keep the SSH implementation path stable"
)]
mod ssh;
#[path = "impls/ssh_config.rs"]
pub(crate) mod ssh_config;
#[path = "struct/mod.rs"]
mod structs;

pub(crate) use ssh::*;
pub(crate) use structs::*;
