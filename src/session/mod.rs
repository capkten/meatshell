#[path = "struct/prompts.rs"]
mod prompts;
#[path = "impls/session.rs"]
#[expect(
    clippy::module_inception,
    reason = "keep the session implementation path stable"
)]
mod session;

pub(crate) use prompts::{ConnectCtx, PendingCred, PendingHostKey, PendingMfa};
