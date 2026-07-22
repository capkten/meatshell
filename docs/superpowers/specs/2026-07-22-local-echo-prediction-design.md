# 本地Echo预测设计文档

**日期**: 2026-07-22
**版本**: 1.1
**状态**: 草稿

## 1. 概述

### 1.1 问题陈述

当前SSH终端输入存在明显延迟：每个字符必须等待服务器echo回来才能显示。在网络延迟较高时（即使是localhost的SSH），快速输入会出现"卡顿"感。

### 1.2 目标

实现**本地echo预测**机制，使以下输入在按键时立即显示：
- 普通可打印字符
- 退格键（Backspace）
- 方向键（↑↓←→）

同时保持与服务器的同步。

### 1.3 预期效果

- 普通字符输入延迟从 ~50-100ms 降至 ~0ms（立即显示）
- 退格键立即删除字符，方向键立即移动光标
- 服务器echo到达后自动验证/修正

## 2. 架构设计

### 2.1 核心组件

```
┌─────────────────────────────────────────────────────────┐
│  Slint UI (on_send_key)                                 │
│  ┌─────────────────────────────────────────────────────┐│
│  │ 1. key_to_pty_bytes() → bytes                       ││
│  │ 2. 立即发送到服务器 (send_raw)                         ││
│  │ 3. 如果是可预测字符 → 立即更新本地显示                    ││
│  │    + 记录预测条目                                      ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│  TermBuffer (预测状态)                                   │
│  - predictions: VecDeque<Prediction>                    │
│  - prediction_timeout: Duration (100ms)                 │
└─────────────────────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│  SSH Worker (接收服务器echo)                              │
│  ┌─────────────────────────────────────────────────────┐│
│  │ 1. 收到echo数据                                      ││
│  │ 2. ingest_terminal_output()                         ││
│  │ 3. 在ingest前检查预测队列                             ││
│  │    - 匹配：跳过预测字符，移除预测条目                    ││
│  │    - 不匹配：用服务器数据覆盖，清除预测                   ││
│  │    - 超时：不再匹配，正常处理                           ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
```

### 2.2 数据结构

```rust
/// 预测操作类型
enum PredictionAction {
    /// 插入字符
    Insert(char),
    /// 退格删除（删除光标前的字符）
    Backspace,
    /// 方向键移动光标
    MoveCursor(Direction),
}

/// 方向
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// 预测条目
struct Prediction {
    /// 预测的操作
    action: PredictionAction,
    /// 预测时的光标位置 (row, col)
    position: (u16, u16),
    /// 创建时间（用于超时检测）
    created_at: Instant,
    /// 是否已过期（超时后标记）
    expired: bool,
    /// 退格操作：被删除的字符（用于恢复）
    deleted_char: Option<char>,
}

/// TermBuffer新增字段
struct TermBuffer {
    // ... 现有字段 ...
    
    /// 预测队列
    predictions: VecDeque<Prediction>,
    /// 预测超时时间
    prediction_timeout: Duration,
}
```

### 2.3 预测规则

**可预测的输入**：

1. **普通可打印字符**（ASCII 32-126，不含控制字符）
   - 单个字符（不是组合键）
   - 非Alt/Ctrl修饰的字符

2. **退格键**（Backspace，0x7F 或 0x08）
   - 删除光标前一个字符
   - 光标左移一格
   - 记录被删除的字符（用于恢复）

3. **方向键**（↑↓←→）
   - 左键：光标左移一格（到达行首时可跳到上一行末尾）
   - 右键：光标右移一格（到达行末时可跳到下一行开头）
   - 上键：光标上移一行
   - 下键：光标下移一行

**不可预测的输入**：
- Tab（补全结果无法预测）
- Enter（服务器可能执行命令）
- Escape
- Ctrl+字母组合
- Alt+字母组合
- 功能键
- 其他控制序列

### 2.4 匹配算法

```rust
fn process_server_echo(&mut self, bytes: &[u8]) {
    // 将bytes解析为字符/控制序列
    for event in parse_server_output(bytes) {
        if let Some(pred) = self.predictions.front() {
            if pred.expired {
                // 已过期，直接移除，正常处理
                self.predictions.pop_front();
                self.apply_server_event(event);
            } else if match_prediction(pred, &event) {
                // 匹配成功，跳过预测，移除条目
                self.predictions.pop_front();
                // 不需要更新屏幕（预测已经显示了）
            } else {
                // 不匹配，用服务器数据修正
                self.predictions.pop_front();
                self.correct_prediction(pred, &event);
            }
        } else {
            // 没有预测，正常处理
            self.apply_server_event(event);
        }
    }
}

/// 检查服务器事件是否匹配预测
fn match_prediction(pred: &Prediction, event: &ServerEvent) -> bool {
    match (&pred.action, event) {
        // 字符插入：服务器echo相同字符
        (PredictionAction::Insert(c), ServerEvent::Char(ec)) => c == ec,
        // 退格：服务器echo退格序列
        (PredictionAction::Backspace, ServerEvent::Backspace) => true,
        // 方向键：服务器echo光标移动序列
        (PredictionAction::MoveCursor(dir), ServerEvent::CursorMove(edir)) => dir == edir,
        _ => false,
    }
}

/// 修正预测错误
fn correct_prediction(&mut self, pred: &Prediction, event: &ServerEvent) {
    match (&pred.action, event) {
        // 字符插入预测错误：用服务器字符覆盖
        (PredictionAction::Insert(_), ServerEvent::Char(ec)) => {
            self.overwrite_char_at(pred.position, *ec);
        }
        // 退格预测错误：恢复被删除的字符
        (PredictionAction::Backspace, _) => {
            if let Some(deleted) = pred.deleted_char {
                self.restore_deleted_char(pred.position, deleted);
            }
            self.apply_server_event(event);
        }
        // 方向键预测错误：用服务器光标位置修正
        (PredictionAction::MoveCursor(_), ServerEvent::CursorMove(_)) => {
            // 服务器会自己设置光标位置，不需要额外操作
        }
        _ => {
            // 其他情况：直接应用服务器事件
            self.apply_server_event(event);
        }
    }
}
```

## 3. 实现细节

### 3.1 修改点

1. **TermBuffer结构** (app.rs:22)
   - 新增 `predictions: VecDeque<Prediction>`
   - 新增 `prediction_timeout: Duration`

2. **on_send_key回调** (app.rs:8092)
   - 判断是否可预测字符
   - 如果可预测：立即更新本地显示 + 记录预测
   - 发送逻辑不变

3. **ingest方法** (app.rs:9960)
   - 在处理服务器数据前，先处理预测匹配
   - 匹配逻辑如上述算法

4. **超时检测**
   - 在ingest时检查队列头部的预测是否超时
   - 超时标记为expired，后续匹配时跳过

### 3.2 线程安全

预测状态在TermBuffer内，与vt100 parser使用相同的锁（Arc<Mutex<TermBuffer>>）。访问预测队列时已经持有锁，无需额外同步。

### 3.3 边界情况

1. **快速连续输入**
   - 预测队列可能积累多个条目
   - 服务器echo按顺序匹配

2. **服务器不echo（密码输入）**
   - 预测字符会一直显示
   - 超时后标记为expired
   - 后续服务器数据正常处理（不会尝试匹配）

3. **服务器修改输入（自动大写）**
   - 匹配失败，用服务器数据覆盖预测

4. **Tab补全**
   - Tab不可预测，直接走原有流程
   - 服务器返回补全结果，正常处理

5. **退格键边界情况**
   - 服务器不处理退格（某些程序）：预测删除了字符，但服务器没删除，需要恢复
   - 退格删除预测字符：如果预测了字符插入，然后退格，需要正确处理队列

6. **方向键边界情况**
   - 光标到达边界：需要处理行首/行尾/屏幕边界
   - 程序内部移动：某些程序可能拦截方向键，预测可能不准确

## 4. 测试策略

### 4.1 单元测试

- 预测创建和超时
- 匹配算法（匹配、不匹配、超时）
- 快速连续输入的队列管理
- 退格键预测和恢复
- 方向键预测和边界处理

### 4.2 集成测试

- 本地SSH连接测试输入流畅度
- 密码输入场景（服务器不echo）
- Tab补全场景
- 快速输入场景
- 退格键连续删除场景
- 方向键快速移动场景
- 混合输入场景（字符+退格+方向键）

## 5. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| 预测字符残留 | 显示错误 | 超时机制 + 服务器数据最终覆盖 |
| 匹配失败导致显示错乱 | 用户体验差 | 用服务器数据立即修正 |
| 性能影响 | 输入延迟 | 预测逻辑轻量，锁竞争最小化 |
| 退格预测错误 | 删除了不该删的字符 | 记录被删除字符，支持恢复 |
| 方向键预测错误 | 光标位置不对 | 服务器光标位置最终同步 |

## 6. 未来扩展

- **智能预测**：根据历史输入模式预测（如常用命令）
- **批量预测**：对快速输入的多个字符一起预测
- **配置选项**：允许用户开关预测功能、调整超时时间
- **Home/End键预测**：支持行首/行尾快速跳转
- **Ctrl+退格**：支持删除前一个单词
