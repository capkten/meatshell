use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use slint::{ModelRc, VecModel};
use tokio::runtime::Runtime;

use crate::docker::command::run_local;
use crate::docker::parse::{
    classify_failure, filter_containers, filter_images, parse_container_detail,
    parse_container_rows, parse_image_detail, parse_image_rows,
};
use crate::docker::{
    ContainerFilter, ContainerState, DockerContainerDetail, DockerError, DockerErrorKind,
    DockerExecResult, DockerImageDetail, DockerRequest, DockerSnapshot, DockerStatus, DockerTab,
    DockerTarget,
};
use crate::ssh::SessionHandle;
use crate::ui::{AppWindow, DockerContainerRow, DockerDetailRow, DockerImageRow, DockerWindow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DockerSummary {
    pub target: String,
    pub status: String,
    pub error: String,
    pub visible: bool,
    pub container_count: i32,
    pub running_count: i32,
    pub image_count: i32,
}

#[derive(Debug, Clone)]
pub(super) struct DockerUiState {
    pub target: DockerTarget,
    pub status: DockerStatus,
    pub snapshot: Option<DockerSnapshot>,
    pub container_error: Option<DockerError>,
    pub image_error: Option<DockerError>,
    pub query: String,
    pub active_tab: DockerTab,
    pub filter: ContainerFilter,
    pub selected_id: Option<String>,
    pub details: Vec<DockerDetailRow>,
    pub generation: u64,
    pub in_flight: bool,
}

impl Default for DockerUiState {
    fn default() -> Self {
        Self::ready_for(DockerTarget::Local)
    }
}

impl DockerUiState {
    pub(super) fn ready_for(target: DockerTarget) -> Self {
        Self {
            target,
            status: DockerStatus::Loading,
            snapshot: None,
            container_error: None,
            image_error: None,
            query: String::new(),
            active_tab: DockerTab::Containers,
            filter: ContainerFilter::All,
            selected_id: None,
            details: Vec::new(),
            generation: 0,
            in_flight: false,
        }
    }

    pub(super) fn begin_target(&mut self, target: DockerTarget) {
        if self.target == target {
            return;
        }
        self.target = target;
        self.generation = self.generation.wrapping_add(1);
        self.status = DockerStatus::Loading;
        self.snapshot = None;
        self.container_error = None;
        self.image_error = None;
        self.query.clear();
        self.active_tab = DockerTab::Containers;
        self.filter = ContainerFilter::All;
        self.selected_id = None;
        self.details.clear();
        self.in_flight = false;
    }
}

pub(super) fn docker_rows(state: &DockerUiState) -> Vec<DockerContainerRow> {
    state
        .snapshot
        .as_ref()
        .map(|s| filter_containers(&s.containers, &state.query, state.filter))
        .unwrap_or_default()
        .into_iter()
        .map(|r| DockerContainerRow {
            id: r.id.into(),
            name: r.name.into(),
            image: r.image.into(),
            status: r.status.into(),
            created: r.created.into(),
        })
        .collect()
}

pub(super) fn docker_image_rows(state: &DockerUiState) -> Vec<DockerImageRow> {
    state
        .snapshot
        .as_ref()
        .map(|s| filter_images(&s.images, &state.query))
        .unwrap_or_default()
        .into_iter()
        .map(|r| DockerImageRow {
            id: r.id.into(),
            repository: r.repository.into(),
            tag: r.tag.into(),
            size: r.size.into(),
            created: r.created.into(),
        })
        .collect()
}

pub(super) fn docker_summary(state: &DockerUiState) -> DockerSummary {
    let snapshot = state.snapshot.as_ref();
    let container_count = snapshot.map_or(0, |s| s.containers.len()) as i32;
    let running_count = snapshot.map_or(0, |s| {
        s.containers
            .iter()
            .filter(|c| c.state == ContainerState::Running)
            .count()
    }) as i32;
    let image_count = snapshot.map_or(0, |s| s.images.len()) as i32;
    DockerSummary {
        target: target_label(&state.target),
        status: status_text(&state.status),
        error: state
            .container_error
            .as_ref()
            .or(state.image_error.as_ref())
            .map(|e| e.message.clone())
            .unwrap_or_default(),
        visible: !matches!(state.status, DockerStatus::NotInstalled),
        container_count,
        running_count,
        image_count,
    }
}

fn target_label(target: &DockerTarget) -> String {
    match target {
        DockerTarget::Local => crate::i18n::t("本机", "Local").to_string(),
        DockerTarget::Remote { label, .. } => label.clone(),
    }
}
fn status_text(status: &DockerStatus) -> String {
    match status {
        DockerStatus::Loading => crate::i18n::t("正在加载 Docker…", "Loading Docker…").to_string(),
        DockerStatus::Ready => crate::i18n::t("Docker 已就绪", "Docker ready").to_string(),
        DockerStatus::Empty => crate::i18n::t("没有 Docker 数据", "No Docker data").to_string(),
        DockerStatus::NotInstalled => {
            crate::i18n::t("未找到 Docker", "Docker is not installed").to_string()
        }
        DockerStatus::Error(e) => e.message.clone(),
    }
}

pub(super) fn target_for_tab(
    active: &str,
    statuses: &crate::resource::TabStatuses,
) -> DockerTarget {
    if active == "welcome" {
        return DockerTarget::Local;
    }
    statuses
        .lock()
        .unwrap()
        .get(active)
        .filter(|s| s.is_ssh)
        .map(|s| DockerTarget::Remote {
            tab_id: active.into(),
            label: s.host.clone(),
        })
        .unwrap_or(DockerTarget::Local)
}

pub(crate) struct DockerController {
    state: Arc<Mutex<DockerUiState>>,
    runtime: Arc<Runtime>,
    handles: Rc<RefCell<HashMap<String, SessionHandle>>>,
    main: slint::Weak<AppWindow>,
    window: slint::Weak<DockerWindow>,
}

impl DockerController {
    pub(super) fn new(
        runtime: Arc<Runtime>,
        handles: Rc<RefCell<HashMap<String, SessionHandle>>>,
        main: slint::Weak<AppWindow>,
        window: slint::Weak<DockerWindow>,
    ) -> Rc<Self> {
        Rc::new(Self {
            state: Arc::new(Mutex::new(DockerUiState::default())),
            runtime,
            handles,
            main,
            window,
        })
    }
    pub(super) fn refresh_target(&self, target: DockerTarget) {
        let refresh = {
            let mut s = self.state.lock().unwrap();
            let changed = s.target != target;
            s.begin_target(target);
            changed || s.snapshot.is_none()
        };
        if refresh {
            self.refresh_now();
        } else {
            self.render();
        }
    }

    pub(super) fn refresh_now(&self) {
        let (generation, target) = {
            let mut s = self.state.lock().unwrap();
            s.in_flight = true;
            (s.generation, s.target.clone())
        };
        let state = self.state.clone();
        let main = self.main.clone();
        let window = self.window.clone();
        let runtime = self.runtime.clone();
        let requests = [
            DockerRequest::Version,
            DockerRequest::Containers,
            DockerRequest::Images,
        ];
        let receivers = match &target {
            DockerTarget::Local => None,
            DockerTarget::Remote { tab_id, .. } => {
                let handles = self.handles.borrow();
                let Some(handle) = handles.get(tab_id) else {
                    self.apply_error(generation, target, "SSH session unavailable");
                    return;
                };
                Some(
                    requests
                        .iter()
                        .cloned()
                        .map(|r| handle.docker_exec(r))
                        .collect::<Vec<_>>(),
                )
            }
        };
        runtime.spawn(async move {
            let result = match receivers {
                None => {
                    let (v, c, i) = tokio::join!(
                        run_local(DockerRequest::Version),
                        run_local(DockerRequest::Containers),
                        run_local(DockerRequest::Images)
                    );
                    SnapshotResult::from_results(v, c, i)
                }
                Some(mut r) => {
                    let v = r.remove(0).await.unwrap_or_else(|_| channel_closed());
                    let c = r.remove(0).await.unwrap_or_else(|_| channel_closed());
                    let i = r.remove(0).await.unwrap_or_else(|_| channel_closed());
                    SnapshotResult::from_results(v, c, i)
                }
            };
            let _ = slint::invoke_from_event_loop(move || {
                apply_snapshot(&state, &main, &window, generation, &target, result)
            });
        });
    }

    pub(super) fn set_query(&self, query: String) {
        self.state.lock().unwrap().query = query;
        self.render();
    }
    pub(super) fn set_tab(&self, tab: DockerTab) {
        self.state.lock().unwrap().active_tab = tab;
        self.render();
    }
    pub(super) fn set_filter(&self, filter: ContainerFilter) {
        self.state.lock().unwrap().filter = filter;
        self.render();
    }

    pub(super) fn select_item(&self, id: String) {
        let (generation, target, request, tab) = {
            let mut s = self.state.lock().unwrap();
            let Some(snapshot) = s.snapshot.as_ref() else {
                return;
            };
            let tab = s.active_tab;
            let request = match tab {
                DockerTab::Containers if snapshot.containers.iter().any(|r| r.id == id) => {
                    DockerRequest::InspectContainer(id.clone())
                }
                DockerTab::Images if snapshot.images.iter().any(|r| r.id == id) => {
                    DockerRequest::InspectImage(id.clone())
                }
                _ => return,
            };
            s.selected_id = Some(id.clone());
            s.details.clear();
            (s.generation, s.target.clone(), request, tab)
        };
        let state = self.state.clone();
        let main = self.main.clone();
        let window = self.window.clone();
        match target.clone() {
            DockerTarget::Local => {
                let runtime = self.runtime.clone();
                runtime.spawn(async move {
                    let result = run_local(request).await;
                    let _ = slint::invoke_from_event_loop(move || {
                        apply_detail(
                            &state, &main, &window, generation, &target, &id, tab, result,
                        )
                    });
                });
            }
            DockerTarget::Remote { tab_id, .. } => {
                let handles = self.handles.borrow();
                let Some(handle) = handles.get(&tab_id) else {
                    return;
                };
                let receiver = handle.docker_exec(request);
                self.runtime.spawn(async move {
                    let result = receiver.await.unwrap_or_else(|_| channel_closed());
                    let _ = slint::invoke_from_event_loop(move || {
                        apply_detail(
                            &state, &main, &window, generation, &target, &id, tab, result,
                        )
                    });
                });
            }
        }
    }

    fn apply_error(&self, generation: u64, target: DockerTarget, message: &str) {
        apply_snapshot(
            &self.state,
            &self.main,
            &self.window,
            generation,
            &target,
            SnapshotResult::fatal(message.into()),
        );
    }
    pub(super) fn render(&self) {
        render_state(&self.state, &self.main, &self.window);
    }
}

struct SnapshotResult {
    snapshot: Option<DockerSnapshot>,
    container_error: Option<DockerError>,
    image_error: Option<DockerError>,
    status: DockerStatus,
}
impl SnapshotResult {
    fn fatal(message: String) -> Self {
        let e = DockerError {
            kind: DockerErrorKind::CommandFailed,
            message,
        };
        Self {
            snapshot: None,
            container_error: Some(e.clone()),
            image_error: None,
            status: DockerStatus::Error(e),
        }
    }
    fn from_results(
        version: DockerExecResult,
        containers: DockerExecResult,
        images: DockerExecResult,
    ) -> Self {
        if !successful(&version) {
            let e = error_from_result(&version);
            return Self {
                snapshot: None,
                container_error: Some(e.clone()),
                image_error: None,
                status: if e.kind == DockerErrorKind::NotInstalled {
                    DockerStatus::NotInstalled
                } else {
                    DockerStatus::Error(e)
                },
            };
        }
        let (containers, container_error) = parse_containers(&containers);
        let (images, image_error) = parse_images(&images);
        let snapshot = Some(DockerSnapshot {
            containers,
            images,
            fetched_at: None,
        });
        let status = if container_error.is_none() && image_error.is_none() {
            if snapshot
                .as_ref()
                .is_some_and(|s| s.containers.is_empty() && s.images.is_empty())
            {
                DockerStatus::Empty
            } else {
                DockerStatus::Ready
            }
        } else if container_error
            .as_ref()
            .is_some_and(|e| e.kind == DockerErrorKind::NotInstalled)
            || image_error
                .as_ref()
                .is_some_and(|e| e.kind == DockerErrorKind::NotInstalled)
        {
            DockerStatus::NotInstalled
        } else {
            DockerStatus::Error(container_error.clone().or(image_error.clone()).unwrap())
        };
        Self {
            snapshot,
            container_error,
            image_error,
            status,
        }
    }
}
fn parse_containers(
    r: &DockerExecResult,
) -> (
    Vec<crate::docker::DockerContainerSummary>,
    Option<DockerError>,
) {
    if !successful(r) {
        return (Vec::new(), Some(error_from_result(r)));
    }
    match parse_container_rows(&r.stdout) {
        Ok(v) => (v, None),
        Err(e) => (Vec::new(), Some(e)),
    }
}
fn parse_images(
    r: &DockerExecResult,
) -> (Vec<crate::docker::DockerImageSummary>, Option<DockerError>) {
    if !successful(r) {
        return (Vec::new(), Some(error_from_result(r)));
    }
    match parse_image_rows(&r.stdout) {
        Ok(v) => (v, None),
        Err(e) => (Vec::new(), Some(e)),
    }
}
fn successful(r: &DockerExecResult) -> bool {
    !r.timed_out && !r.not_found && r.exit_code == Some(0)
}
fn error_from_result(r: &DockerExecResult) -> DockerError {
    DockerError {
        kind: if r.timed_out {
            DockerErrorKind::CommandFailed
        } else {
            classify_failure(r)
        },
        message: if r.timed_out {
            "Docker command timed out".into()
        } else if r.stderr.is_empty() {
            r.stdout.clone()
        } else {
            r.stderr.clone()
        },
    }
}
fn channel_closed() -> DockerExecResult {
    DockerExecResult {
        stderr: "Docker session channel closed".into(),
        ..Default::default()
    }
}

fn apply_snapshot(
    state: &Arc<Mutex<DockerUiState>>,
    main: &slint::Weak<AppWindow>,
    window: &slint::Weak<DockerWindow>,
    generation: u64,
    target: &DockerTarget,
    result: SnapshotResult,
) {
    let mut s = state.lock().unwrap();
    if s.generation != generation || s.target != *target {
        return;
    }
    s.snapshot = result.snapshot;
    s.container_error = result.container_error;
    s.image_error = result.image_error;
    s.status = result.status;
    s.in_flight = false;
    drop(s);
    render_state(state, main, window);
}
#[allow(clippy::too_many_arguments)]
fn apply_detail(
    state: &Arc<Mutex<DockerUiState>>,
    main: &slint::Weak<AppWindow>,
    window: &slint::Weak<DockerWindow>,
    generation: u64,
    target: &DockerTarget,
    selected_id: &str,
    tab: DockerTab,
    result: DockerExecResult,
) {
    let mut s = state.lock().unwrap();
    if s.generation != generation
        || s.target != *target
        || s.selected_id.as_deref() != Some(selected_id)
        || s.active_tab != tab
        || !successful(&result)
    {
        return;
    }
    s.details = match tab {
        DockerTab::Containers => parse_container_detail(&result.stdout)
            .map(container_detail_rows)
            .unwrap_or_default(),
        DockerTab::Images => parse_image_detail(&result.stdout)
            .map(image_detail_rows)
            .unwrap_or_default(),
    };
    drop(s);
    render_state(state, main, window);
}
fn container_detail_rows(d: DockerContainerDetail) -> Vec<DockerDetailRow> {
    [
        ("ID", d.id),
        ("Image", d.image),
        ("Command", d.command),
        ("Created", d.created),
        ("Ports", d.ports),
        ("Mounts", d.mounts),
        ("Networks", d.networks),
    ]
    .into_iter()
    .filter(|(_, v)| !v.is_empty())
    .map(|(l, v)| DockerDetailRow {
        label: l.into(),
        value: v.into(),
    })
    .collect()
}
fn image_detail_rows(d: DockerImageDetail) -> Vec<DockerDetailRow> {
    [
        ("ID", d.id),
        ("Repository tags", d.repository_tags),
        ("Size", d.size),
        ("Created", d.created),
    ]
    .into_iter()
    .filter(|(_, v)| !v.is_empty())
    .map(|(l, v)| DockerDetailRow {
        label: l.into(),
        value: v.into(),
    })
    .collect()
}

fn render_state(
    state: &Arc<Mutex<DockerUiState>>,
    main: &slint::Weak<AppWindow>,
    window: &slint::Weak<DockerWindow>,
) {
    let s = state.lock().unwrap().clone();
    let summary = docker_summary(&s);
    if let Some(w) = window.upgrade() {
        w.set_target(summary.target.clone().into());
        w.set_status(summary.status.clone().into());
        w.set_error(summary.error.clone().into());
        w.set_loading(s.in_flight || matches!(s.status, DockerStatus::Loading));
        w.set_query(s.query.clone().into());
        w.set_active_tab(if s.active_tab == DockerTab::Containers {
            0
        } else {
            1
        });
        w.set_container_filter(match s.filter {
            ContainerFilter::All => 0,
            ContainerFilter::Running => 1,
            ContainerFilter::Stopped => 2,
        });
        w.set_containers(ModelRc::from(Rc::new(VecModel::from(docker_rows(&s)))));
        w.set_images(ModelRc::from(Rc::new(VecModel::from(docker_image_rows(
            &s,
        )))));
        w.set_details(ModelRc::from(Rc::new(VecModel::from(s.details))));
    }
    if let Some(w) = main.upgrade() {
        w.set_docker_visible(summary.visible);
        w.set_docker_target(summary.target.into());
        w.set_docker_status(summary.status.into());
        w.set_docker_error(summary.error.into());
        w.set_docker_container_count(summary.container_count);
        w.set_docker_running_count(summary.running_count);
        w.set_docker_image_count(summary.image_count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn view_state_filters_rows_without_changing_raw_snapshot() {
        let snapshot = snapshot_with_running_and_stopped_containers();
        let state = DockerUiState {
            snapshot: Some(snapshot.clone()),
            query: "nginx".into(),
            filter: ContainerFilter::Running,
            ..Default::default()
        };
        let rows = docker_rows(&state);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "web-nginx");
        assert_eq!(state.snapshot.as_ref().unwrap().containers.len(), 2);
    }
    #[test]
    fn target_change_increments_generation_and_clears_old_data() {
        let mut state = DockerUiState::ready_for(DockerTarget::Local);
        state.snapshot = Some(snapshot_with_running_and_stopped_containers());
        let generation = state.generation;
        state.begin_target(DockerTarget::Remote {
            tab_id: "term-2".into(),
            label: "server".into(),
        });
        assert_eq!(state.generation, generation + 1);
        assert!(state.snapshot.is_none());
    }

    #[test]
    fn target_routing_keeps_non_ssh_tabs_local() {
        let statuses = crate::resource::TabStatuses::default();
        statuses.lock().unwrap().insert(
            "ssh".into(),
            crate::resource::TabStatus {
                host: "server".into(),
                is_ssh: true,
                ..Default::default()
            },
        );
        statuses.lock().unwrap().insert(
            "telnet".into(),
            crate::resource::TabStatus {
                host: "telnet-host".into(),
                ..Default::default()
            },
        );
        statuses.lock().unwrap().insert(
            "serial".into(),
            crate::resource::TabStatus {
                host: "COM3".into(),
                ..Default::default()
            },
        );

        assert_eq!(target_for_tab("welcome", &statuses), DockerTarget::Local);
        assert_eq!(target_for_tab("telnet", &statuses), DockerTarget::Local);
        assert_eq!(target_for_tab("serial", &statuses), DockerTarget::Local);
        assert_eq!(
            target_for_tab("ssh", &statuses),
            DockerTarget::Remote {
                tab_id: "ssh".into(),
                label: "server".into()
            }
        );
    }

    #[test]
    fn summary_counts_raw_snapshot_and_hides_only_not_installed() {
        let mut state = DockerUiState {
            snapshot: Some(snapshot_with_running_and_stopped_containers()),
            status: DockerStatus::Ready,
            query: "nginx".into(),
            filter: ContainerFilter::Running,
            ..Default::default()
        };
        let summary = docker_summary(&state);
        assert_eq!(summary.container_count, 2);
        assert_eq!(summary.running_count, 1);
        assert!(summary.visible);

        state.status = DockerStatus::NotInstalled;
        assert!(!docker_summary(&state).visible);
    }

    #[test]
    fn image_rows_map_filtered_snapshot_values() {
        let state = DockerUiState {
            snapshot: Some(DockerSnapshot {
                images: vec![crate::docker::DockerImageSummary {
                    id: "sha256:abc".into(),
                    repository: "nginx".into(),
                    tag: "latest".into(),
                    size: "42MB".into(),
                    created: "today".into(),
                }],
                ..DockerSnapshot::default()
            }),
            query: "NGINX".into(),
            ..Default::default()
        };
        let rows = docker_image_rows(&state);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].repository, "nginx");
        assert_eq!(rows[0].tag, "latest");
    }

    #[test]
    fn stale_snapshot_result_does_not_replace_new_target_state() {
        let state = Arc::new(Mutex::new(DockerUiState::ready_for(DockerTarget::Local)));
        let generation = state.lock().unwrap().generation;
        state.lock().unwrap().begin_target(DockerTarget::Remote {
            tab_id: "ssh".into(),
            label: "server".into(),
        });
        state.lock().unwrap().in_flight = true;
        apply_snapshot(
            &state,
            &slint::Weak::default(),
            &slint::Weak::default(),
            generation,
            &DockerTarget::Local,
            SnapshotResult::from_results(success_result(), success_result(), success_result()),
        );
        let current = state.lock().unwrap();
        assert!(current.snapshot.is_none());
        assert!(current.in_flight);
    }

    #[test]
    fn stale_detail_result_does_not_replace_new_selection() {
        let state = Arc::new(Mutex::new(DockerUiState::ready_for(DockerTarget::Local)));
        let target = DockerTarget::Local;
        {
            let mut s = state.lock().unwrap();
            s.snapshot = Some(snapshot_with_running_and_stopped_containers());
            s.selected_id = Some("stopped-id".into());
        }
        apply_detail(
            &state,
            &slint::Weak::default(),
            &slint::Weak::default(),
            0,
            &target,
            "running-id",
            DockerTab::Containers,
            success_detail_result(),
        );
        assert!(state.lock().unwrap().details.is_empty());
    }

    #[test]
    fn detail_rows_never_include_config_environment() {
        let detail = parse_container_detail(
            r#"[{"Id":"container-id","Config":{"Image":"nginx","Env":["SECRET=x"]}}]"#,
        )
        .unwrap();
        let rows = container_detail_rows(detail);
        assert!(rows.iter().all(|row| !row.value.contains("SECRET")));
    }

    #[test]
    fn partial_list_failure_keeps_successful_page_and_reports_error() {
        let result = SnapshotResult::from_results(
            success_result(),
            success_result_with_container(),
            failed_result("permission denied"),
        );
        assert_eq!(result.snapshot.unwrap().containers.len(), 1);
        assert_eq!(
            result.image_error.unwrap().kind,
            DockerErrorKind::PermissionDenied
        );
    }

    fn success_result() -> DockerExecResult {
        DockerExecResult {
            exit_code: Some(0),
            ..Default::default()
        }
    }

    fn success_result_with_container() -> DockerExecResult {
        DockerExecResult {
            stdout: r#"{"ID":"id","Names":"web","Image":"nginx","State":"running"}"#.into(),
            exit_code: Some(0),
            ..Default::default()
        }
    }

    fn failed_result(stderr: &str) -> DockerExecResult {
        DockerExecResult {
            stderr: stderr.into(),
            exit_code: Some(1),
            ..Default::default()
        }
    }

    fn success_detail_result() -> DockerExecResult {
        DockerExecResult {
            stdout: r#"[{"Id":"running-id","Config":{"Image":"nginx"}}]"#.into(),
            exit_code: Some(0),
            ..Default::default()
        }
    }
    fn snapshot_with_running_and_stopped_containers() -> DockerSnapshot {
        DockerSnapshot {
            containers: vec![
                crate::docker::DockerContainerSummary {
                    id: "running-id".into(),
                    name: "web-nginx".into(),
                    image: "nginx:latest".into(),
                    state: ContainerState::Running,
                    status: "Up".into(),
                    created: "today".into(),
                    ports: "80/tcp".into(),
                },
                crate::docker::DockerContainerSummary {
                    id: "stopped-id".into(),
                    name: "worker".into(),
                    image: "busybox".into(),
                    state: ContainerState::Stopped,
                    status: "Exited".into(),
                    created: "yesterday".into(),
                    ports: String::new(),
                },
            ],
            images: Vec::new(),
            fetched_at: None,
        }
    }
}
