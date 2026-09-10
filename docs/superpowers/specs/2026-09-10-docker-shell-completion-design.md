# Docker 终端命令补全设计

日期：2026-09-10

状态：设计已确认，待用户审阅

## 1. 目标

在 MeatShell 的 SSH 终端中支持 Docker 相关的 Tab 补全：

- 远端已经安装并启用 Docker 原生补全时，继续使用远端原生行为。
- 远端没有 Docker 原生补全时，由 MeatShell 在当前 Shell 会话中注入兜底补全。
- 补全仍由远端 Bash/Zsh 的 readline/ZLE 负责光标移动、候选显示和命令行重绘。
- 不改变普通命令（例如 `cd`）、方向键、Ctrl+C、vim/nano 等现有终端行为。

当前终端输入路径会把按键编码后直接发送到远端 PTY，远端 Shell 负责回显和命令行编辑；Docker 监控使用独立的只读查询路径。因此本功能不在 Slint 端复制一套命令行编辑器，而是复用既有的 SSH Shell integration 注入点。

## 2. 范围

### 2.1 本次包含

- SSH 远端 Bash 会话的 Docker 补全。
- SSH 远端 Zsh 会话的 Docker 补全。
- 有原生 Docker completer 时不覆盖、不替换原生 completer。
- 没有原生 completer 时注入以下兜底补全：
  - `docker run`：镜像仓库、tag 和可用镜像 ID。
  - `docker exec`：容器名称和容器 ID。
  - `docker start`：容器名称和容器 ID。
  - `docker stop`：容器名称和容器 ID。
  - `docker rm`：容器名称和容器 ID。
  - `docker logs`：容器名称和容器 ID。
  - `docker inspect`：容器和镜像名称/ID。
- 候选项按当前输入前缀过滤，并保持 Shell 原生补全的插入行为。
- 对常用选项进行基本跳过和定位，使资源名出现在选项后时仍能补全；无法安全判断的复杂语法不主动插入候选。

### 2.2 本次不包含

- MeatShell 自己绘制悬浮候选框或维护完整的本地命令行缓冲区。
- 覆盖远端已经存在的 Docker 原生补全。
- 安装 `bash-completion`、修改远端 `.bashrc`/`.zshrc` 或写入永久配置。
- 自动补全任意 Docker 参数、Compose、Swarm、Kubernetes、Podman 或第三方插件命令。
- PowerShell、cmd、fish、ash/dash 等非 SSH Bash/Zsh 场景的兜底补全。
- 修改 Docker 监控窗口已有的查询、刷新、详情和权限行为。

本期兜底能力随现有 SSH Shell integration 生效；用户关闭 Shell integration 时，终端保持原有行为，不注入补全脚本。

## 3. 方案比较

### 方案 A：Shell 原生优先 + 会话级兜底（采用）

在当前 SSH 的 Bash/Zsh 探测和 prompt setup 之后，只检查一次 Docker 原生 completion 是否已经注册。已注册时整个会话都使用远端 completion；明确未注册时只在当前会话注册 MeatShell 的补全函数，并保持该选择到会话结束。

优点：

- 复用远端 Shell 对命令行、光标、候选列表和多行重绘的处理。
- 不会因为 MeatShell 不理解 readline/ZLE 的内部状态而破坏编辑行为。
- 候选查询直接在远端执行，天然对应当前 Docker daemon 和当前用户权限。
- 不修改远端持久配置，断开会话后自动消失。

代价：

- 第一次按 Tab 需要在远端执行只读 Docker 列表命令。
- 兜底只覆盖 Bash/Zsh 和本期列出的 Docker 常用命令。

### 方案 B：MeatShell 本地跟踪整行输入并弹出候选

由应用记录每个终端的按键、光标和当前命令，在检测到 `docker` 时从 Docker 监控快照生成候选。

优点是界面可以统一，且可以直接复用应用中的快照；但终端无法可靠知道远端 readline/ZLE 的真实编辑状态，历史回滚、光标移动、别名、转义序列、vim/nano 和命令重绘都容易造成状态分叉。该方案还会让普通终端输入路径承担新的敏感状态，不采用。

### 方案 C：依赖远端 Docker CLI 生成官方 completion 脚本

尝试调用远端 Docker CLI 的 completion 生成能力并在当前会话执行。

该能力受 Docker CLI 版本、发行版打包方式和 Shell 支持影响，不是所有常见远端都具备；同时仍需要处理原生 completion 是否已注册和脚本输出兼容性。因此作为后续增强方向，不作为本期兜底基础。

## 4. 选定方案的行为设计

### 4.1 注册时机与优先级

现有 SSH worker 已经通过独立 probe 识别 Bash/Zsh，并在收到初始 prompt 后注入 prompt setup。Docker 补全脚本放在同一套会话级 setup 中，使用现有的隐藏回显和完成标记机制，避免内部命令出现在终端画面或命令历史里。

注册顺序如下，每个交互式 SSH 会话只执行一次：

1. 等待远端 Shell 初始化完成，确保用户配置中已经注册的 completion 可被观察到。
2. Bash 检查 `docker` 是否已有 completion specification；Zsh 检查 `docker` 是否已有可用 completion function/binding。
3. 已有原生 completion 时不声明同名兜底 completion。
4. 没有原生 completion 时定义 MeatShell 私有函数，并只在当前会话注册到 `docker` 命令。

检查和注册命令的 stdout/stderr 必须被隐藏。原生 completion 的检查结果必须锁存在当前 Shell 会话的注册状态中；后续每次按 Tab 都不得再次执行原生 completion 探测，也不得根据候选查询结果改变已选模式。若一次性检查无法确定远端状态，则不覆盖远端 completion，也不显示错误。

### 4.2 候选来源

候选在用户按 Tab 时从远端 Docker CLI 查询，不使用应用侧 Docker 监控窗口快照作为唯一来源。这个查询只负责获取当前候选，不负责重新检查原生 completion：

- 镜像：使用结构化模板列出仓库、tag 和 ID，过滤明显无效的 dangling 项，并按当前 token 前缀过滤。
- 容器：使用结构化模板列出容器名称和 ID，包含运行中与已停止容器。
- `inspect`：合并容器与镜像候选并去重。

查询命令只读且使用固定参数。用户输入只作为 Shell completion 的前缀匹配值，不拼接为 Docker 命令或 Shell 代码。

### 4.3 命令定位规则

- `docker run` 在还没有确定镜像位置时补全镜像；镜像之后的命令和参数不由本期兜底处理。
- `docker exec`、`start`、`stop`、`rm`、`logs` 在资源参数位置补全容器；支持同一命令中继续补全多个资源参数。
- `docker inspect` 在资源参数位置补全容器和镜像。
- 常见布尔选项、带值选项和 `--` 分隔符需要被识别，以避免把选项值误当成镜像或容器。
- 遇到未知或无法安全解析的选项组合时返回空候选，让 Shell 保留默认行为，不强行改写用户输入。

候选只针对命令行中第一个命令词为 `docker` 的场景。`sudo docker`、自定义别名、Shell 函数包装和 Docker Compose 不在本期范围内。

### 4.4 失败与性能

- Docker 未安装、daemon 不可用、权限不足、远端命令失败或输出异常时，补全函数静默返回空候选。
- 补全失败不得在终端打印错误、污染命令历史或改变当前输入。
- 查询使用当前远端用户权限，不提升权限，不执行 `sudo`。
- 首期不引入跨会话或持久缓存，以确保新建/删除镜像和容器后下一次 Tab 能看到最新状态；后续如实测延迟过高，再单独设计有过期时间的候选缓存。候选缓存不能重新触发原生 completion 探测。

## 5. 模块边界

### SSH Shell integration

在 `src/ssh/impls/ssh.rs` 的 Shell probe、prompt setup 和隐藏回显流程中接入补全 setup。现有 `PROMPT_BODY` 的 OSC 7 当前目录通知、OSC 697 命令历史捕获、OSC 699 完成标记和 Bash/Zsh 支持判断必须保持不变。

如脚本体积或测试复杂度明显增加，补全脚本和 Shell-specific 构造逻辑提取到独立模块（例如 `src/ssh/impls/docker_completion.rs`），由 SSH worker 只负责组合并注入。该模块不得依赖 Slint 或 Docker 监控窗口状态。

### Docker 领域模块

复用现有 `src/docker` 的固定格式和解析约定，只在确有必要时增加面向 completion 的固定查询格式或纯函数。不得把用户当前输入直接传给 `DockerRequest` 或远程命令构造器。

### 终端输入路径

不修改 `ui/terminal_view.slint`、`src/app.rs` 的通用按键转发策略和 `key_to_pty_bytes` 的 Tab 编码。Tab 仍然作为原始按键发送给远端 Shell；补全逻辑只改变远端 Shell 收到 Tab 后的 completion 注册状态。

## 6. 测试策略

### 6.1 纯逻辑测试

- Bash/Zsh 原生 completion 已存在时，setup 脚本不会注册兜底 completer。
- 没有原生 completion 时，setup 脚本会注册且使用 MeatShell 私有命名空间。
- 同一会话内多次触发补全时，不会重复执行原生 completion 检查，且不会在 native/fallback 两种模式之间切换。
- `docker run` 只在镜像位置生成候选。
- `exec/start/stop/rm/logs` 只在容器资源位置生成候选。
- `inspect` 合并容器和镜像候选并去重。
- 常见选项、选项值、`--` 和多个资源参数的定位结果正确。
- 前缀过滤、空候选、特殊字符和 Docker 查询失败不会产生 Shell 注入或可见错误。
- 现有 prompt setup 的历史清理、OSC 标记隐藏、Bash/Zsh 探测测试继续通过。

### 6.2 Shell 行为测试

在可用的 Bash/Zsh 环境中使用固定的 Docker 输出替身，验证：

- 没有原生 Docker completer 时，按命令类别返回预期候选。
- 已有 native completer 时，兜底函数不夺取控制权。
- 查询失败时命令行内容、退出状态和终端输出不被污染。

如果 CI 环境缺少 Zsh 或真实 Docker daemon，Shell 测试必须使用可注入的命令替身或纯脚本断言，不依赖宿主 Docker 服务。

### 6.3 回归验证

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --locked`
- `cargo check`
- 手工连接一个带 Docker 原生 completion 的 Bash/Zsh 主机，确认继续使用原生行为。
- 手工连接一个没有 Docker 原生 completion 但有 Docker CLI 的 Bash/Zsh 主机，验证上述七个命令的补全。
- 验证 Docker 不可用、权限不足、Shell integration 关闭、非 Bash/Zsh Shell 时不出现可见错误。
- 回归验证 `cd<Tab>`、方向键、Ctrl+C、vim/nano、普通 Shell 输入和现有 Docker 监控。

## 7. 验收标准

- 在 Bash/Zsh 中，已有远端 Docker completion 时不被覆盖。
- 没有远端 Docker completion 时，`docker run` 能补全镜像，`exec/start/stop/rm/logs` 能补全容器，`inspect` 能补全容器和镜像。
- 原生 completion 的远端检查每个会话只执行一次；进入兜底模式后，后续 Tab 不再重复检查远端是否存在原生 completion。
- 候选来自当前远端 Docker 环境，且包含停止容器。
- Tab 补全不会在终端显示内部查询命令，不会写入 Shell 历史，不会泄露环境变量或敏感 Docker inspect 内容。
- Docker 查询失败时只表现为空候选，不破坏正在编辑的命令。
- 关闭 Shell integration 或使用不支持的 Shell 时，现有终端行为保持不变。
- 所有现有测试与格式、Lint、检查命令通过。
