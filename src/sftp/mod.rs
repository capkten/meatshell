#[path = "impls/sftp.rs"]
#[expect(
    clippy::module_inception,
    reason = "keep the SFTP implementation path stable"
)]
mod sftp;
#[path = "struct/transfer.rs"]
mod transfer;

pub(crate) use sftp::*;
pub(crate) use transfer::{DownloadConflict, SftpCommand, SftpHandles, SftpLastCwd};
