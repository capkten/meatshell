# Docker 容器与镜像监控实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不影响现有会话、系统资源和进程监控的前提下，为 MeatShell 增加跟随当前活动目标的只读 Docker 容器/镜像摘要和独立监控窗口。

**Architecture:** 新增纯 Rust Docker 领域模块，统一定义 Docker 请求、JSON 解析、错误分类、搜索和状态筛选；本机通过 `tokio::process::Command` 执行固定参数的 Docker CLI，远程通过当前 SSH worker 已认证的 russh handle 开启短生命周期 exec channel。应用层用一个 UI 线程专属的 Docker 控制器管理目标、快照、筛选、详情、刷新代次和 Slint 模型；左侧摘要和独立窗口只消费控制器的已解析状态。

**Tech Stack:** Rust 2021、Slint 1.8、tokio、serde/serde_json、russh 0.49、现有 `VecModel`/`Timer`/`slint::invoke_from_event_loop` 链路。

## Global Constraints

- 版本保持 Rust 1.75、Slint 1.8、russh 0.49；不得为了 Docker 功能升级 russh。
- Docker 第一版只读，只支持 Docker CLI，不支持 Podman、Compose、Kubernetes 或容器操作。
- 欢迎页、本地终端、Telnet、串口查询本机 Docker；SSH 会话复用当前已认证 SSH 会话查询远程 Docker。
- Docker 未安装时隐藏左侧面板；权限不足、守护进程未运行、命令失败和解析失败必须显示原因。
- 搜索在已加载快照上进行；容器按名称/完整 ID/镜像匹配，镜像按仓库/标签/ID 匹配，大小写不敏感。
- 面板或窗口可见时每 5 秒刷新，隐藏时暂停；旧目标或旧请求结果不得覆盖新目标。
- 不读取、不解析、不展示环境变量；不得把用户搜索词直接拼进远程 shell 命令。
- 所有 Slint 更新必须在 UI 线程完成；后台结果使用现有 UI 回调或 `slint::invoke_from_event_loop` 路径。
- 保留 `Session.notes`、本地更新地址、GPU 监控、SFTP 生命周期和其他 fork-specific contracts。
- 每个任务都要有独立测试和提交；实现完成后按 `cargo fmt` → `cargo clippy` → `cargo test` 顺序验证，并补跑 `cargo check`。

---

## 文件和责任地图

| 文件 | 责任 |
| --- | --- |
| `src/docker/mod.rs` | Docker 模块入口和类型/解析/命令 API 导出 |
| `src/docker/model.rs` | 请求、执行结果、容器/镜像摘要、详情和错误状态 |
| `src/docker/command.rs` | 固定 Docker CLI 参数、远程命令引用、运行本机命令 |
| `src/docker/parse.rs` | JSON 行解析、inspect 映射、错误分类、搜索和状态过滤 |
| `src/ssh/struct/command.rs` | 增加经当前 SSH worker 请求 Docker exec 的命令 |
| `src/ssh/struct/event.rs` | 增加 `SessionHandle::docker_exec` 便捷接口 |
| `src/ssh/impls/ssh.rs` | 使用已认证 russh handle 执行 Docker 请求并回传结果 |
| `src/terminal/impls/local.rs`、`telnet.rs`、`serial.rs` | 为新增命令保持枚举穷尽，并返回不支持响应 |
| `ui/docker_window.slint` | 独立 Docker 窗口和列表/详情交互 |
| `ui/sidebar.slint` | 左侧 Docker 摘要块和点击回调 |
| `ui/app.slint` | 导入/导出 Docker 类型、主窗口属性和回调转发 |
| `src/app/docker.rs` | UI 线程 Docker 控制器、目标路由、模型映射、刷新和详情请求 |
| `src/app/core.rs` | 每个主窗口保存 Docker 强句柄、弱句柄和控制器状态 |
| `src/app.rs` | 创建窗口、注册回调、启动定时器、处理活动标签变化和主题/窗口位置 |
| `src/app/sidebar.rs` | 把 Docker 控制器状态转换为主窗口侧边栏摘要 |
| `src/app/resource_ui.rs` | Docker 窗口主题同步、窗口定位和 Slint 模型辅助函数（若实现需要） |
| `lang/en/LC_MESSAGES/meatshell.po`、`lang/zh/LC_MESSAGES/meatshell.po` | 新增 Docker UI 字符串翻译 |

---

### Task 1: 建立 Docker 领域模型、命令请求和纯函数解析

**Files:**
- Create: `src/docker/mod.rs`
- Create: `src/docker/model.rs`
- Create: `src/docker/parse.rs`
- Modify: `src/main.rs:10-24`，增加 `mod docker;`
- Test: `src/docker/parse.rs` 内的 `#[cfg(test)]` 模块

**Interfaces:**
- Produces `DockerRequest`, `DockerExecResult`, `DockerTarget`, `DockerErrorKind`, `DockerError`、`DockerSnapshot`、`DockerContainerSummary`、`DockerImageSummary`、`DockerContainerDetail`、`DockerImageDetail`。
- Produces `DockerStatus` (`Loading`, `Ready`, `Empty`, `NotInstalled`, `Error`) and `DockerTab` (`Containers`, `Images`) for the UI controller.
- Produces `parse_container_rows(&str) -> Result<Vec<DockerContainerSummary>, DockerError>`。
- Produces `parse_image_rows(&str) -> Result<Vec<DockerImageSummary>, DockerError>`。
- Produces `parse_container_detail(&str) -> Result<DockerContainerDetail, DockerError>` 和 `parse_image_detail(&str) -> Result<DockerImageDetail, DockerError>`。
- Produces `filter_containers(&[DockerContainerSummary], &str, ContainerFilter) -> Vec<DockerContainerSummary>` 和 `filter_images(&[DockerImageSummary], &str) -> Vec<DockerImageSummary>`。
- Produces `classify_failure(&DockerExecResult) -> DockerErrorKind`，供本机和远程执行共用。

- [ ] **Step 1: 先写失败测试，固定数据结构和搜索约定。**

```rust
#[test]
fn parses_json_lines_and_keeps_stopped_containers() {
    let stdout = r#"{"ID":"abc123","Image":"nginx:1.27","Names":"web","State":"running","Status":"Up 3 days","CreatedAt":"2026-09-08 10:00:00 +0000 UTC","Ports":"0.0.0.0:80->80/tcp"}
{"ID":"def456","Image":"worker:old","Names":"worker","State":"exited","Status":"Exited (0) 2 days ago","CreatedAt":"2026-09-06 10:00:00 +0000 UTC","Ports":""}"#;
    let rows = parse_container_rows(stdout).expect("valid Docker JSON lines");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "web");
    assert_eq!(rows[1].state, ContainerState::Stopped);
}

#[test]
fn search_matches_name_id_and_image_case_insensitively() {
    let rows = vec![container("abc123", "web-nginx", "nginx:1.27", ContainerState::Running)];
    assert_eq!(filter_containers(&rows, "NGINX", ContainerFilter::All).len(), 1);
    assert_eq!(filter_containers(&rows, "ABC123", ContainerFilter::All).len(), 1);
    assert!(filter_containers(&rows, "postgres", ContainerFilter::All).is_empty());
}

#[test]
fn filters_running_and_stopped_without_mutating_snapshot() {
    let rows = vec![
        container("run", "running", "busybox", ContainerState::Running),
        container("stop", "stopped", "busybox", ContainerState::Stopped),
    ];
    assert_eq!(filter_containers(&rows, "", ContainerFilter::Running).len(), 1);
    assert_eq!(filter_containers(&rows, "", ContainerFilter::Stopped).len(), 1);
    assert_eq!(rows.len(), 2);
}

#[test]
fn classifies_not_installed_permission_daemon_and_parse_failures() {
    assert_eq!(classify_failure(&exec_failure("docker: command not found")), DockerErrorKind::NotInstalled);
    assert_eq!(classify_failure(&exec_failure("permission denied while trying to connect")), DockerErrorKind::PermissionDenied);
    assert_eq!(classify_failure(&exec_failure("Cannot connect to the Docker daemon")), DockerErrorKind::DaemonUnavailable);
    assert_eq!(classify_failure(&exec_failure("unexpected failure")), DockerErrorKind::CommandFailed);
}
```

- [ ] **Step 2: 运行领域测试，确认新模块和接口尚未实现。**

Run: `cargo test docker::parse::tests --lib`

Expected: FAIL because `docker` module, model types, parser functions, and test helpers do not exist yet.

- [ ] **Step 3: 写最小领域模型和解析实现。**

在 `model.rs` 中实现以下稳定接口；字段保持 `pub(crate)`，不让 UI 直接依赖 JSON：

```rust
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
pub(crate) enum ContainerState { Running, Stopped }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerFilter { All, Running, Stopped }

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockerTarget {
    Local,
    Remote { tab_id: String, label: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockerTab { Containers, Images }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockerErrorKind { NotInstalled, PermissionDenied, DaemonUnavailable, CommandFailed, ParseFailed }

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerError { pub kind: DockerErrorKind, pub message: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerContainerSummary {
    pub id: String, pub name: String, pub image: String, pub state: ContainerState,
    pub status: String, pub created: String, pub ports: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerImageSummary {
    pub id: String, pub repository: String, pub tag: String, pub size: String, pub created: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerContainerDetail {
    pub id: String, pub image: String, pub command: String, pub created: String,
    pub ports: String, pub mounts: String, pub networks: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerImageDetail {
    pub id: String, pub repository_tags: String, pub size: String, pub created: String,
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

fn container(id: &str, name: &str, image: &str, state: ContainerState) -> DockerContainerSummary {
    DockerContainerSummary { id: id.into(), name: name.into(), image: image.into(), state, status: String::new(), created: String::new(), ports: String::new() }
}

fn exec_failure(stderr: &str) -> DockerExecResult {
    DockerExecResult { stderr: stderr.into(), exit_code: Some(1), ..DockerExecResult::default() }
}
```

The `container` and `exec_failure` helpers in that test block belong inside the test module; production code contains only the domain types and functions.

In `parse.rs`, parse one JSON object per line for list commands, parse the first object from `docker inspect` arrays, map missing optional fields to empty strings, and deliberately ignore `Config.Env`. Use `serde_json::Value` for Docker's version-dependent nested fields. Normalize names by removing one leading `/` from container names and determine `ContainerState` from the JSON `State` field.

- [ ] **Step 4: 运行领域测试，确认解析和筛选通过。**

Run: `cargo test docker::parse::tests --lib`

Expected: PASS, including the stopped-container, case-insensitive search, combined state filter, and error-classification cases.

- [ ] **Step 5: 提交领域层。**

```powershell
git add src/main.rs src/docker
git commit -m "feat: add docker domain models and parsers"
```

### Task 2: 实现固定 Docker CLI 命令和本机执行器

**Files:**
- Create: `src/docker/command.rs`
- Modify: `src/docker/mod.rs`
- Test: `src/docker/command.rs` 内的 `#[cfg(test)]` 模块

**Interfaces:**
- Produces `docker_args(&DockerRequest) -> Vec<String>`，参数只由固定操作和已选中的 Docker ID 组成。
- Produces `remote_command(&DockerRequest) -> String`，用于 SSH exec channel；ID 通过单引号 shell quoting 保护。
- Produces `pub(crate) async fn run_local(request: DockerRequest) -> DockerExecResult`。

- [ ] **Step 1: 写失败测试，固定 CLI 参数和远程引用行为。**

```rust
#[test]
fn list_commands_emit_json_lines_without_shell_pipeline() {
    assert_eq!(docker_args(&DockerRequest::Containers), vec![
        "ps", "-a", "--no-trunc", "--format", "{{json .}}"
    ].into_iter().map(String::from).collect::<Vec<_>>());
}

#[test]
fn inspect_id_is_quoted_in_remote_command() {
    let command = remote_command(&DockerRequest::InspectContainer("abc123".into()));
    assert_eq!(command, "docker inspect --type container 'abc123'");
}
```

- The JSON template assertion uses the exact string `{{json .}}`; the implementation and test fixture must use the same five arguments shown here.

- [ ] **Step 2: 运行命令构造测试，确认尚未实现。**

Run: `cargo test docker::command::tests --lib`

Expected: FAIL because the command module and builders do not exist.

- [ ] **Step 3: 实现命令构造和本机执行。**

`docker_args` must return these exact argument sequences:

```rust
pub(crate) fn docker_args(request: &DockerRequest) -> Vec<String> {
    match request {
        DockerRequest::Version => ["version", "--format", "{{json .}}"].into_iter().map(str::to_owned).collect(),
        DockerRequest::Containers => ["ps", "-a", "--no-trunc", "--format", "{{json .}}"].into_iter().map(str::to_owned).collect(),
        DockerRequest::Images => ["image", "ls", "--no-trunc", "--format", "{{json .}}"].into_iter().map(str::to_owned).collect(),
        DockerRequest::InspectContainer(id) => vec!["inspect".into(), "--type".into(), "container".into(), id.clone()],
        DockerRequest::InspectImage(id) => vec!["image".into(), "inspect".into(), id.clone()],
    }
}
```

`run_local` must use `tokio::process::Command::new("docker").args(docker_args(&request)).output()` inside a 5-second `tokio::time::timeout`. Set `not_found` only when the process spawn returns `ErrorKind::NotFound`; preserve stdout/stderr and convert exit status to `i32`. Do not invoke a shell and do not include environment variables in any request.

`remote_command` must use the same fixed arguments and shell-quote only the inspect ID. The JSON template must remain compatible with Docker versions that support Go-template `json`.

- [ ] **Step 4: 运行命令测试并补充本机错误测试。**

Run: `cargo test docker::command::tests --lib`

Expected: PASS. Add a platform-independent test that passes a synthetic `DockerExecResult { not_found: true, ..Default::default() }` through `classify_failure` and expects `NotInstalled`; do not require Docker to be installed on the test machine.

- [ ] **Step 5: 提交本机执行器。**

```powershell
git add src/docker
git commit -m "feat: execute docker cli locally"
```

### Task 3: 通过当前 SSH 会话执行远程 Docker 请求

**Files:**
- Modify: `src/ssh/struct/command.rs`
- Modify: `src/ssh/struct/event.rs`
- Modify: `src/ssh/impls/ssh.rs`
- Modify: `src/terminal/impls/local.rs`
- Modify: `src/terminal/impls/telnet.rs`
- Modify: `src/terminal/impls/serial.rs`
- Test: `src/ssh/impls/ssh.rs` 或 `src/docker/command.rs` 中的无网络命令/结果测试

**Interfaces:**
- Extends `SessionCommand` with `DockerExec { request: DockerRequest, reply: tokio::sync::oneshot::Sender<DockerExecResult> }`.
- Adds `SessionHandle::docker_exec(&self, request: DockerRequest) -> tokio::sync::oneshot::Receiver<DockerExecResult>`.
- Adds an SSH-only helper `run_remote_docker(handle: &std::sync::Arc<russh::client::Handle<ClientHandler>>, request: DockerRequest) -> DockerExecResult`.

- [ ] **Step 1: 写失败的 transport API 测试/编译检查。**

Add a compile-facing unit test in `src/ssh/struct/event.rs` that constructs a `SessionHandle` with an unbounded command channel and asserts that `docker_exec(DockerRequest::Version)` returns a receiver. The test must not start a real SSH connection.

```rust
#[tokio::test]
async fn session_handle_can_enqueue_docker_request() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = SessionHandle {
        tab_id: "tab".into(),
        commands: tx,
        join: tokio::spawn(async {}),
    };
    let _reply = handle.docker_exec(DockerRequest::Version);
    assert!(matches!(rx.try_recv(), Ok(SessionCommand::DockerExec { .. })));
}
```

- [ ] **Step 2: 运行测试，确认新增枚举分支不存在。**

Run: `cargo test session_handle_can_enqueue_docker_request --lib`

Expected: FAIL because `DockerRequest` is not wired into `SessionCommand` or `SessionHandle` yet.

- [ ] **Step 3: 增加命令和 handle 方法。**

In `src/ssh/struct/command.rs`, import `DockerExecResult` and `DockerRequest`, then add the `DockerExec` variant. In `src/ssh/struct/event.rs`, add the `docker_exec` method that creates a oneshot channel, sends the variant, and returns the receiver. Keep the response channel separate from `SessionEvent` so Docker output never enters the terminal renderer or command history.

- [ ] **Step 4: 在 SSH worker 中复用已认证 handle。**

After `run_session` has wrapped the authenticated russh handle in `Arc`, add a `SessionCommand::DockerExec { request, reply }` branch. Clone the existing `Arc<Handle<ClientHandler>>`, spawn a background task, open a short-lived session channel, call `channel.exec(true, remote_command(&request).as_bytes())`, collect stdout/stderr up to 4 MiB, capture exit status, and send `DockerExecResult` through `reply`. Bound the collection by the same 5-second timeout as local execution. The branch must not use `execute_command`, because that function creates a second SSH connection and would violate the approved design.

On channel-open, exec, timeout, or collection failure, send a non-panicking `DockerExecResult` with a useful stderr/message and `timed_out` when applicable. Never log command arguments or Docker output because inspect output may contain sensitive metadata.

- [ ] **Step 5: Make non-SSH workers exhaustive without changing their Docker target.**

In local, Telnet, and serial worker command matches, handle `DockerExec` by replying with a `DockerExecResult` whose stderr explains that the current worker does not provide remote Docker execution. The app layer never sends this branch for those session kinds; they use the local executor. Keep all existing `RawInput`, resize, tunnel, process, and close behavior unchanged.

- [ ] **Step 6: Run transport tests and compile the workers.**

Run: `cargo test session_handle_can_enqueue_docker_request --lib`

Expected: PASS.

Run: `cargo check`

Expected: PASS with all four session workers matching the new command variant.

- [ ] **Step 7: 提交 SSH transport。**

```powershell
git add src/ssh src/terminal/impls/local.rs src/terminal/impls/telnet.rs src/terminal/impls/serial.rs
git commit -m "feat: run docker queries over active ssh sessions"
```

### Task 4: 创建 Slint Docker 窗口、左侧摘要和翻译资源

**Files:**
- Create: `ui/docker_window.slint`
- Modify: `ui/app.slint`
- Modify: `ui/sidebar.slint`
- Modify: `lang/en/LC_MESSAGES/meatshell.po`
- Modify: `lang/zh/LC_MESSAGES/meatshell.po`

**Interfaces:**
- Produces `DockerContainerRow`, `DockerImageRow`, `DockerDetailRow` and exported `DockerWindow`.
- Adds these exact `AppWindow` properties: `docker-visible: bool`, `docker-target: string`, `docker-status: string`, `docker-error: string`, `docker-container-count: int`, `docker-running-count: int`, `docker-image-count: int`, `docker-window-open: bool`, plus callback `open-docker()`.
- Adds the same summary properties to `Sidebar` and callback `show-docker()`.

- [ ] **Step 1: 先写 Slint API 骨架，固定 Rust 侧生成类型。**

Add these exported structs and callback/property names to `ui/docker_window.slint` before implementing the full layout:

```slint
export struct DockerContainerRow {
    id: string,
    name: string,
    image: string,
    status: string,
    created: string,
}
export struct DockerImageRow {
    id: string,
    repository: string,
    tag: string,
    size: string,
    created: string,
}
export struct DockerDetailRow { label: string, value: string }

export component DockerWindow inherits Window {
    in property <string> target;
    in property <string> status;
    in property <string> error;
    in property <bool> loading;
    in property <[DockerContainerRow]> containers;
    in property <[DockerImageRow]> images;
    in property <[DockerDetailRow]> details;
    in-out property <int> active-tab: 0;
    in-out property <string> query;
    in-out property <int> container-filter: 0;
    callback close();
    callback refresh();
    callback search-changed(string);
    callback tab-changed(int);
    callback filter-changed(int);
    callback select-container(string);
    callback select-image(string);
    callback win-drag();
    callback win-resize-east();
    callback win-resize-se();
}
```

Add matching `AppWindow` properties/callbacks and `Sidebar` properties before wiring the visual blocks. The generated Rust names must remain snake-case equivalents such as `get_docker_window_open`, `set_docker_container_count`, and `on_open_docker`.

- [ ] **Step 2: 运行 Slint 编译检查，确认 UI API 自洽。**

Run: `cargo check`

Expected: PASS after `ui/app.slint` re-exports the new component and forwards the Sidebar properties/callbacks; no Rust controller is required yet because the generated setters are not called until Task 5.

- [ ] **Step 3: 实现窗口布局。**

In `DockerWindow`, copy the process window's theme bindings, custom titlebar drag callback, east/se resize callbacks, wallpaper overlay, and close behavior. Build a scrollable body with:

- Header showing `Docker 监控`/`Docker Monitor` and `target`.
- Container/Image tabs using `active-tab`.
- `LineEdit` bound to `query` and sending `search-changed` on edit.
- Refresh button sending `refresh`.
- Container filter buttons for all/running/stopped.
- Container and image rows with `TouchArea` selection callbacks.
- Detail rows and explicit error/empty/loading text.

The UI must never render an environment-variable field. The list receives already-filtered Slint models; no Docker command or string parsing is performed in Slint.

- [ ] **Step 4: 实现左侧摘要块。**

Add a `DockerBlock` in `ui/sidebar.slint` that is conditionally rendered only when `docker-visible` is true. Show target, connection/error status, container/running/image counts, and a click target that calls `show-docker`. Insert it into both the vertical and horizontal sidebar layouts so docking to any edge preserves the feature. Keep the existing StatsBlock, NetBlock, DiskBlock, collapse and drag behavior unchanged.

- [ ] **Step 5: 添加中英文翻译。**

Add the exact Chinese msgids used by the new UI to the Chinese catalog and matching English `msgstr` values to the English catalog, including Docker, containers, images, search, all/running/stopped, refresh, no results, no data, loading, permission denied, daemon unavailable, Docker not installed, and read-only detail labels. Do not rename existing msgids.

- [ ] **Step 6: Run the UI compiler and commit the UI surface.**

Run: `cargo check`

Expected: PASS; no edits are made to generated `target` output.

```powershell
git add ui/app.slint ui/sidebar.slint ui/docker_window.slint lang/en/LC_MESSAGES/meatshell.po lang/zh/LC_MESSAGES/meatshell.po
git commit -m "feat: add docker monitor slint interface"
```

### Task 5: 实现 UI 线程 Docker 控制器和快照视图

**Files:**
- Create: `src/app/docker.rs`
- Modify: `src/app/core.rs`
- Modify: `src/app.rs:4719-4730` for the SSH-target flag and window wiring
- Modify: `src/app/sidebar.rs`
- Modify: `src/resource/struct/system.rs` for the per-tab SSH-target flag
- Test: `src/app/docker.rs` 内的纯状态/视图测试

**Interfaces:**
- Produces `DockerUiState` with target, availability, last snapshot, query, tab, filter, selected ID, request generation, and in-flight state.
- Produces `DockerController::refresh_target(&self, target: DockerTarget)`, `DockerController::refresh_now(&self)`, `DockerController::set_query(&self, query: String)`, `DockerController::set_tab(&self, tab: DockerTab)`, `DockerController::set_filter(&self, filter: ContainerFilter)`, and `DockerController::select_item(&self, id: String)`.
- Produces `docker_summary(...)` and `docker_rows(...)` helpers that map domain values to generated Slint rows.

- [ ] **Step 1: 写失败的纯视图测试。**

```rust
#[test]
fn view_state_filters_rows_without_changing_raw_snapshot() {
    let snapshot = snapshot_with_running_and_stopped_containers();
    let mut state = DockerUiState::default();
    state.snapshot = Some(snapshot.clone());
    state.query = "nginx".into();
    state.filter = ContainerFilter::Running;
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
    state.begin_target(DockerTarget::Remote { tab_id: "term-2".into(), label: "server".into() });
    assert_eq!(state.generation, generation + 1);
    assert!(state.snapshot.is_none());
}
```

- [ ] **Step 2: 运行控制器测试，确认状态类型尚未实现。**

Run: `cargo test app::docker::tests --lib`

Expected: FAIL because the controller and generated row mapping do not exist.

- [ ] **Step 3: 实现 UI 状态和目标选择。**

Use `DockerTarget::Local` for the welcome tab and for any `TabStatus` whose transport is not SSH. Add a separate `is_ssh: bool` flag to `TabStatus` when the tab is seeded in `src/app.rs`; do not repurpose `is_local`, because changing it for Telnet or serial would alter their existing resource-panel behavior. For an SSH tab, keep the remote target even while connecting or disconnected so the UI reports the unavailable SSH target instead of silently showing the local machine.

`DockerUiState` must store:

```rust
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
```

Implement `DockerUiState::ready_for(target: DockerTarget) -> Self` with `status: DockerStatus::Loading`, empty models, `DockerTab::Containers`, `ContainerFilter::All`, `generation: 0`, and `in_flight: false`. Implement `begin_target(&mut self, target: DockerTarget)` to compare targets, increment generation, clear snapshot/errors/selection/details/query, reset tab/filter, and set loading.

When target changes, increment `generation`, clear snapshot/selection/details, reset query/filter, and set loading. When a result arrives, apply it only if both generation and target still match.

- [ ] **Step 4: 实现本地/远程快照调度。**

`refresh_target` must run the version probe, container list, and image list in the background. For `Local`, call `run_local`; for `Remote`, look up the active tab's `SessionHandle` and call `docker_exec` for each fixed request. Do not call `ssh::execute_command`. Aggregate successful lists into one `DockerSnapshot`; classify a missing executable as hidden and all other failures as visible status errors. If only one list fails, retain the successful list and attach an error to the failed page.

Return the result through `runtime.spawn` and `slint::invoke_from_event_loop`. Before applying it, compare the captured generation and target. Clear `in_flight` on every terminal result, including timeout and channel-close errors.

- [ ] **Step 5: 实现模型映射、搜索、筛选和详情。**

`docker_rows` uses `filter_containers` or `filter_images`, then maps to `DockerContainerRow`/`DockerImageRow` and replaces the shared `VecModel` contents in place. Query changes only update `DockerUiState` and the model. Tab/filter changes do the same without a Docker command. Summary counts always come from the unfiltered snapshot; window result counts come from the filtered model.

`select_item` validates that the selected ID is present in the current snapshot, then requests `InspectContainer` or `InspectImage`. Map only the approved detail fields into `DockerDetailRow`; ignore all `Config.Env` values and do not log the inspect payload.

- [ ] **Step 6: 实现错误、空状态和侧边栏映射。**

Add helpers that map `DockerStatus` to translated summary text and visibility. `NotInstalled` sets `docker-visible` false. Permission, daemon, command, parse, loading, and empty states set `docker-visible` true and keep a clear message. The active tab displays `container_error` or `image_error`, while a successful page remains usable. `refresh_sidebar` receives the controller's current state and updates the new AppWindow properties beside the existing CPU/memory/process fields without altering existing branches.

- [ ] **Step 7: 运行控制器测试。**

Run: `cargo test app::docker::tests --lib`

Expected: PASS for client-side filtering, target generation invalidation, count mapping, empty state, and environment-variable omission.

- [ ] **Step 8: 提交 UI 控制器。**

```powershell
git add src/app/docker.rs src/app/core.rs src/app/sidebar.rs src/app.rs src/resource/struct/system.rs
git commit -m "feat: coordinate docker snapshots in app ui"
```

### Task 6: 接入窗口生命周期、5 秒刷新和活动标签切换

**Files:**
- Modify: `src/app.rs`
- Modify: `src/app/core.rs`
- Modify: `src/app/session_event.rs`
- Modify: `src/app/session_runtime.rs` to call the controller's target invalidation when an SSH tab reconnects in place

**Interfaces:**
- `WindowState` holds `docker_win: Rc<DockerWindow>`, `docker_weak: Weak<DockerWindow>`, shared `VecModel`s, and `Rc<RefCell<DockerUiState>>`.
- `AppWindow.on_open_docker` shows/focuses the window, synchronizes target/theme, and requests immediate refresh.
- `DockerWindow.on_close` hides the window and sets `docker-window-open` false.
- Produces `docker_refresh_needed(dynamic_ui_active: bool, sidebar_visible: bool, window_open: bool) -> bool`.

- [ ] **Step 1: 写失败的 lifecycle test for target selection and visibility.**

Add tests for the pure helper used by the callbacks:

```rust
#[test]
fn docker_refresh_is_needed_only_when_sidebar_or_window_is_visible() {
    assert!(docker_refresh_needed(true, false, true));
    assert!(docker_refresh_needed(false, true, true));
    assert!(!docker_refresh_needed(false, false, true));
    assert!(!docker_refresh_needed(true, false, false));
}
```

Implement the helper as `dynamic_ui_active && (sidebar_visible || window_open)`; the first argument already includes the existing zen-mode/minimized visibility decision.

- [ ] **Step 2: 运行 lifecycle 测试，确认 helper 尚未实现。**

Run: `cargo test docker_refresh_is_needed_only_when_sidebar_or_window_is_visible --lib`

Expected: FAIL until the visibility helper is added.

- [ ] **Step 3: 创建并保存独立窗口及共享模型。**

Follow the existing `ProcWindow` setup in `src/app.rs`: create one `Rc<DockerWindow>`, one `VecModel<DockerContainerRow>`, one `VecModel<DockerImageRow>`, and one `VecModel<DockerDetailRow>`; attach the same `ModelRc`s to `AppWindow` and `DockerWindow`; set custom titlebar and initial tab/filter values; keep strong handles in `WindowState` so the hidden window remains alive.

Wire close, drag, east resize, south-east resize, and theme/scale/wallpaper synchronization just as `ProcWindow` does. Add a placement helper that centers the Docker window on the monitor of the main window while preserving a user-resized size.

- [ ] **Step 4: 接入左侧点击、弹窗操作和活动标签。**

Forward `Sidebar.show-docker` through `AppWindow.open-docker`. On open, set `docker-window-open`, sync target/connection text/theme, call `refresh_now`, show/focus the window, and place it. Search/filter/tab/select callbacks update `DockerUiState` and the shared models; only select callbacks launch `inspect` requests.

Extend the existing `changed active-tab-id`/`refresh-sidebar` path so target changes call `begin_target` and schedule a refresh. Update `SessionEvent::Connected`, `Closed`, and reconnect paths to invalidate the current remote Docker target and refresh its visible error/loading state. Do not change terminal output, SFTP, process, or system-stat event handling.

- [ ] **Step 5: 添加 5 秒 timer 和可见性暂停。**

Create one repeated `slint::Timer` owned by `WindowState`, with `Duration::from_secs(5)`. On each tick, call `docker_refresh_needed(dynamic_ui_active && !zen_mode, sidebar_updates_visible(window), window.get_docker_window_open())`; if true, call `refresh_now`. A hidden/collapsed/zen main window with a hidden Docker window must not execute Docker commands. Opening the Docker window immediately refreshes regardless of the last tick.

- [ ] **Step 6: 运行完整编译检查和窗口相关测试。**

Run: `cargo check`

Expected: PASS with generated `DockerWindow` bindings, shared models, callbacks, timer, and all existing windows compiled.

Run: `cargo test app::docker --lib`

Expected: PASS, including target changes, visibility gating, filtering, and stale-result invalidation.

- [ ] **Step 7: 提交生命周期接入。**

```powershell
git add src/app.rs src/app/core.rs src/app/session_event.rs src/app/session_runtime.rs ui/app.slint ui/sidebar.slint
git commit -m "feat: connect docker monitor to active tab lifecycle"
```

### Task 7: 回归验证、手工场景和发布前检查

**Files:**
- Modify only files identified by failing tests or formatting; do not broaden the feature scope.
- Test: existing unit tests plus Docker parser/controller tests from Tasks 1, 2, 5, and 6.

- [ ] **Step 1: 执行格式检查。**

Run: `cargo fmt --all -- --check`

Expected: PASS with no formatting diff.

- [ ] **Step 2: 执行编译检查。**

Run: `cargo check`

Expected: PASS for Rust, generated Slint bindings, and all supported target-specific code paths.

- [ ] **Step 3: 执行 Lint。**

Run: `cargo clippy --all-targets -- -D warnings`

Expected: PASS with zero warnings.

- [ ] **Step 4: 执行单元测试。**

Run: `cargo test --locked`

Expected: PASS for existing config/session/wallpaper/SSH/SFTP/pane tests and all new Docker tests.

- [ ] **Step 5: 手工验证本机 Docker。**

With Docker installed and accessible, confirm the welcome tab and local terminal show the local target, counts update, the popup opens, search filters containers/images, details omit environment variables, and manual/5-second refresh update the lists. Stop the Docker service and confirm the card remains visible with the daemon error. Run once on a machine without the Docker executable and confirm the card is hidden.

- [ ] **Step 6: 手工验证 SSH Docker。**

Connect to an SSH server with Docker, confirm the sidebar target is the server and that local Docker data is never substituted. Test permission denied and daemon-not-running states, switch between two SSH tabs and a local/Telnet/serial tab, and confirm the old server list never appears under the new target. Close the popup and collapse the sidebar; confirm no 5-second Docker commands continue while both surfaces are hidden.

- [ ] **Step 7: 回归现有功能并提交验证结果。**

Verify CPU/memory/swap/GPU/disk, process monitor, system-information window, SSH shell, SFTP, Telnet, serial, theme switching, window docking, reconnect, and existing fork-specific session/update behavior. After the checks pass, create the final feature commit or squash only the Docker implementation commits if the user requests a single commit.

---

## Traceability to the approved specification

| Spec requirement | Plan coverage |
| --- | --- |
| Local/SSH/Telnet/serial target mapping | Tasks 5 and 6; `TabStatus` remote flag preserves existing resource semantics |
| Docker absent hidden, operational errors visible | Tasks 1, 5, and 7 |
| Read-only containers/images window | Task 4 and Task 6 |
| Search and container state filters | Tasks 1, 4, and 5 |
| Container/image detail fields and no env vars | Tasks 1, 5, and 7 |
| JSON CLI output and active SSH reuse | Tasks 2 and 3 |
| 5-second visible-only refresh and stale-result rejection | Task 6 |
| Existing functionality and fork contracts preserved | Global Constraints and Task 7 |
| Unit, compile, lint, and manual verification | Every task's test step and Task 7 |
