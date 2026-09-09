#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockerRequest {
    Version,
    Containers,
    Images,
    InspectContainer(String),
    InspectImage(String),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DockerExecResult {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) not_found: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerState {
    Running,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerFilter {
    All,
    Running,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockerTarget {
    Local,
    Remote { tab_id: String, label: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockerTab {
    Containers,
    Images,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockerErrorKind {
    NotInstalled,
    PermissionDenied,
    DaemonUnavailable,
    CommandFailed,
    ParseFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerError {
    pub(crate) kind: DockerErrorKind,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerContainerSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) image: String,
    pub(crate) state: ContainerState,
    pub(crate) status: String,
    pub(crate) created: String,
    pub(crate) ports: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerImageSummary {
    pub(crate) id: String,
    pub(crate) repository: String,
    pub(crate) tag: String,
    pub(crate) size: String,
    pub(crate) created: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerContainerDetail {
    pub(crate) id: String,
    pub(crate) image: String,
    pub(crate) command: String,
    pub(crate) created: String,
    pub(crate) ports: String,
    pub(crate) mounts: String,
    pub(crate) networks: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerImageDetail {
    pub(crate) id: String,
    pub(crate) repository_tags: String,
    pub(crate) size: String,
    pub(crate) created: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DockerSnapshot {
    pub(crate) containers: Vec<DockerContainerSummary>,
    pub(crate) images: Vec<DockerImageSummary>,
    pub(crate) fetched_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockerStatus {
    Loading,
    Ready,
    Empty,
    NotInstalled,
    Error(DockerError),
}
