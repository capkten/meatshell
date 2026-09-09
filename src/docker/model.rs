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
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub not_found: bool,
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
    pub kind: DockerErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerContainerSummary {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: ContainerState,
    pub status: String,
    pub created: String,
    pub ports: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerImageSummary {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
    pub created: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerContainerDetail {
    pub id: String,
    pub image: String,
    pub command: String,
    pub created: String,
    pub ports: String,
    pub mounts: String,
    pub networks: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerImageDetail {
    pub id: String,
    pub repository_tags: String,
    pub size: String,
    pub created: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DockerSnapshot {
    pub containers: Vec<DockerContainerSummary>,
    pub images: Vec<DockerImageSummary>,
    pub fetched_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockerStatus {
    Loading,
    Ready,
    Empty,
    NotInstalled,
    Error(DockerError),
}
