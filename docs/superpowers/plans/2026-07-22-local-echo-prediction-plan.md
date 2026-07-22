# 本地Echo预测实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现本地echo预测机制，使普通字符、退格键、方向键在按键时立即显示，提升SSH终端输入流畅度。

**Architecture:** 在TermBuffer中新增预测队列，按键时立即应用预测并记录，服务器echo到达时匹配/修正预测。使用超时机制处理服务器不响应的情况。

**Tech Stack:** Rust, vt100 parser, Slint UI, tokio async

## Global Constraints

- 预测超时时间：100ms
- 仅预测普通可打印字符（ASCII 32-126）、退格键、方向键
- 不预测Tab、Enter、Escape、Ctrl/Alt组合键
- 预测状态在TermBuffer内，与vt100 parser共享锁
- 所有修改集中在src/app.rs

---

### Task 1: 添加预测数据结构

**Files:**
- Modify: `src/app.rs:22-61` (TermBuffer结构体)
- Modify: `src/app.rs:64` (新增常量)

**Interfaces:**
- Produces: `PredictionAction`, `Direction`, `Prediction` 类型定义
- Produces: `TermBuffer::predictions`, `TermBuffer::prediction_timeout` 字段

- [ ] **Step 1: 在app.rs顶部添加预测类型定义**

在`src/app.rs`的`CsiState`枚举定义之前（约第70行）添加：

```rust
/// 预测操作类型
#[derive(Debug, Clone)]
enum PredictionAction {
    /// 插入字符
    Insert(char),
    /// 退格删除（删除光标前的字符）
    Backspace,
    /// 方向键移动光标
    MoveCursor(Direction),
}

/// 方向
#[derive(Debug, Clone, PartialEq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// 预测条目
#[derive(Debug, Clone)]
struct Prediction {
    /// 预测的操作
    action: PredictionAction,
    /// 预测时的光标位置 (row, col)
    position: (u16, u16),
    /// 创建时间（用于超时检测）
    created_at: std::time::Instant,
    /// 是否已过期（超时后标记）
    expired: bool,
    /// 退格操作：被删除的字符（用于恢复）
    deleted_char: Option<char>,
}
```

- [ ] **Step 2: 在TermBuffer结构体中添加预测字段**

在`TermBuffer`结构体的`raw`字段之后添加：

```rust
    /// 预测队列
    predictions: std::collections::VecDeque<Prediction>,
    /// 预测超时时间
    prediction_timeout: std::time::Duration,
```

- [ ] **Step 3: 添加预测超时常量**

在`OUTPUT_MERGE_BYTE_CAP`常量之后添加：

```rust
/// 预测超时时间（毫秒）
const PREDICTION_TIMEOUT_MS: u64 = 100;
```

- [ ] **Step 4: 更新TermBuffer::new()初始化**

找到`TermBuffer::new()`方法，在初始化`raw`字段之后添加：

```rust
            predictions: std::collections::VecDeque::new(),
            prediction_timeout: std::time::Duration::from_millis(PREDICTION_TIMEOUT_MS),
```

- [ ] **Step 5: 编译验证**

Run: `cargo check`
Expected: 编译通过，无错误

- [ ] **Step 6: 提交**

```bash
git add src/app.rs
git commit -m "feat: add prediction data structures to TermBuffer"
```

---

### Task 2: 实现预测判断逻辑

**Files:**
- Modify: `src/app.rs` (新增方法)

**Interfaces:**
- Consumes: `PredictionAction`, `Direction`, `Prediction`
- Produces: `is_predictable()`, `create_prediction()` 函数

- [ ] **Step 1: 添加预测判断函数**

在`TermBuffer`的`impl`块之后添加：

```rust
/// 判断按键是否可预测
fn is_predictable(key: &str, ctrl: bool, alt: bool) -> Option<PredictionAction> {
    // Ctrl/Alt组合键不可预测
    if ctrl || alt {
        return None;
    }
    
    // 空字符串（单独修饰键）不可预测
    if key.is_empty() {
        return None;
    }
    
    // 检查是否为退格键
    if key == "\u{0008}" || key == "\u{007F}" {
        return Some(PredictionAction::Backspace);
    }
    
    // 检查是否为方向键（Slint PUA编码）
    match key {
        "\u{F700}" => return Some(PredictionAction::MoveCursor(Direction::Up)),
        "\u{F701}" => return Some(PredictionAction::MoveCursor(Direction::Down)),
        "\u{F702}" => return Some(PredictionAction::MoveCursor(Direction::Left)),
        "\u{F703}" => return Some(PredictionAction::MoveCursor(Direction::Right)),
        _ => {}
    }
    
    // 检查是否为普通可打印字符（单个字符，ASCII 32-126）
    if key.chars().count() == 1 {
        let ch = key.chars().next().unwrap();
        let cp = ch as u32;
        if cp >= 32 && cp <= 126 {
            return Some(PredictionAction::Insert(ch));
        }
    }
    
    None
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check`
Expected: 编译通过，无错误

- [ ] **Step 3: 提交**

```bash
git add src/app.rs
git commit -m "feat: add predictability check function"
```

---

### Task 3: 实现本地预测应用逻辑

**Files:**
- Modify: `src/app.rs` (TermBuffer新增方法)

**Interfaces:**
- Consumes: `PredictionAction`, `Prediction`
- Produces: `apply_prediction()` 方法

- [ ] **Step 1: 在TermBuffer中添加apply_prediction方法**

在`TermBuffer`的`impl`块中添加：

```rust
    /// 应用预测到本地屏幕（立即显示）
    fn apply_prediction(&mut self, action: PredictionAction) {
        let screen = self.parser.screen();
        let (row, col) = screen.cursor_position();
        let (rows, cols) = screen.size();
        
        let deleted_char = match &action {
            PredictionAction::Insert(ch) => {
                // 在光标位置插入字符，光标右移
                // 注意：vt100 parser不支持直接插入，我们需要手动更新屏幕
                // 这里我们只记录预测，实际显示更新在render时处理
                None
            }
            PredictionAction::Backspace => {
                // 删除光标前的字符，光标左移
                if col > 0 {
                    // 获取被删除的字符（用于恢复）
                    let deleted = self.get_char_at(row, col - 1);
                    Some(deleted)
                } else if row > 0 {
                    // 行首退格：跳到上一行末尾
                    let prev_row = row - 1;
                    let prev_col = self.get_line_end_col(prev_row);
                    let deleted = self.get_char_at(prev_row, prev_col);
                    Some(deleted)
                } else {
                    None
                }
            }
            PredictionAction::MoveCursor(dir) => {
                // 移动光标
                None
            }
        };
        
        // 记录预测条目
        let prediction = Prediction {
            action,
            position: (row, col),
            created_at: std::time::Instant::now(),
            expired: false,
            deleted_char,
        };
        self.predictions.push_back(prediction);
    }
    
    /// 获取指定位置的字符
    fn get_char_at(&self, row: u16, col: u16) -> char {
        let screen = self.parser.screen();
        // vt100 screen没有直接获取字符的方法，返回空格作为默认值
        // 实际实现时需要根据vt100库的API调整
        ' '
    }
    
    /// 获取行尾列位置
    fn get_line_end_col(&self, row: u16) -> u16 {
        let (_, cols) = self.parser.screen().size();
        // 从行尾开始向前扫描，找到最后一个非空字符
        // 简化实现：返回cols-1
        cols - 1
    }
```

- [ ] **Step 2: 编译验证**

Run: `cargo check`
Expected: 编译通过，无错误

- [ ] **Step 3: 提交**

```bash
git add src/app.rs
git commit -m "feat: add local prediction application logic"
```

---

### Task 4: 实现预测超时检测

**Files:**
- Modify: `src/app.rs` (TermBuffer新增方法)

**Interfaces:**
- Consumes: `Prediction`, `prediction_timeout`
- Produces: `check_prediction_timeouts()` 方法

- [ ] **Step 1: 在TermBuffer中添加超时检测方法**

在`TermBuffer`的`impl`块中添加：

```rust
    /// 检查并标记超时的预测
    fn check_prediction_timeouts(&mut self) {
        let now = std::time::Instant::now();
        for pred in &mut self.predictions {
            if !pred.expired && now.duration_since(pred.created_at) > self.prediction_timeout {
                pred.expired = true;
                tracing::debug!("prediction expired: {:?}", pred.action);
            }
        }
    }
```

- [ ] **Step 2: 编译验证**

Run: `cargo check`
Expected: 编译通过，无错误

- [ ] **Step 3: 提交**

```bash
git add src/app.rs
git commit -m "feat: add prediction timeout detection"
```

---

### Task 5: 实现服务器echo匹配逻辑

**Files:**
- Modify: `src/app.rs` (TermBuffer新增方法)

**Interfaces:**
- Consumes: `Prediction`, `PredictionAction`, `Direction`
- Produces: `process_server_echo()` 方法

- [ ] **Step 1: 在TermBuffer中添加服务器echo处理方法**

在`TermBuffer`的`impl`块中添加：

```rust
    /// 处理服务器echo，匹配/修正预测
    fn process_server_echo(&mut self, bytes: &[u8]) {
        // 先检查超时
        self.check_prediction_timeouts();
        
        // 将bytes解析为字符
        let text = String::from_utf8_lossy(bytes);
        
        for ch in text.chars() {
            if let Some(pred) = self.predictions.front() {
                if pred.expired {
                    // 已过期，直接移除，正常处理
                    self.predictions.pop_front();
                    self.apply_char_to_screen(ch);
                } else if self.match_prediction(pred, ch) {
                    // 匹配成功，跳过预测，移除条目
                    self.predictions.pop_front();
                    // 不需要更新屏幕（预测已经显示了）
                } else {
                    // 不匹配，用服务器数据修正
                    let pred = self.predictions.pop_front().unwrap();
                    self.correct_prediction(&pred, ch);
                }
            } else {
                // 没有预测，正常处理
                self.apply_char_to_screen(ch);
            }
        }
    }
    
    /// 检查服务器字符是否匹配预测
    fn match_prediction(&self, pred: &Prediction, ch: char) -> bool {
        match &pred.action {
            PredictionAction::Insert(c) => *c == ch,
            PredictionAction::Backspace => {
                // 退格键：服务器echo可能是退格序列或删除序列
                ch == '\u{0008}' || ch == '\u{007F}' || ch == '\x1b[D'
            }
            PredictionAction::MoveCursor(dir) => {
                // 方向键：服务器echo可能是光标移动序列
                match dir {
                    Direction::Up => ch == '\x1b[A' || ch == '\x1bOA',
                    Direction::Down => ch == '\x1b[B' || ch == '\x1bOB',
                    Direction::Left => ch == '\x1b[D' || ch == '\x1bOD',
                    Direction::Right => ch == '\x1b[C' || ch == '\x1bOC',
                }
            }
        }
    }
    
    /// 修正预测错误
    fn correct_prediction(&mut self, pred: &Prediction, ch: char) {
        match &pred.action {
            PredictionAction::Insert(_) => {
                // 字符插入预测错误：用服务器字符覆盖
                self.overwrite_char_at(pred.position, ch);
            }
            PredictionAction::Backspace => {
                // 退格预测错误：恢复被删除的字符
                if let Some(deleted) = pred.deleted_char {
                    self.restore_deleted_char(pred.position, deleted);
                }
                self.apply_char_to_screen(ch);
            }
            PredictionAction::MoveCursor(_) => {
                // 方向键预测错误：服务器会自己设置光标位置
                // 不需要额外操作
            }
        }
    }
    
    /// 在指定位置覆盖字符
    fn overwrite_char_at(&mut self, position: (u16, u16), ch: char) {
        // vt100 parser不支持直接修改屏幕内容
        // 这里需要手动更新parser的内部状态
        // 简化实现：重新解析整个屏幕
        // 实际实现时需要根据vt100库的API调整
    }
    
    /// 恢复被删除的字符
    fn restore_deleted_char(&mut self, position: (u16, u16), ch: char) {
        // 在指定位置插入字符
        // 实际实现时需要根据vt100库的API调整
    }
    
    /// 应用字符到屏幕（正常处理）
    fn apply_char_to_screen(&mut self, ch: char) {
        // 将字符传递给vt100 parser处理
        let bytes = ch.to_string().into_bytes();
        self.ingest(&bytes);
    }
```

- [ ] **Step 2: 编译验证**

Run: `cargo check`
Expected: 编译通过，无错误

- [ ] **Step 3: 提交**

```bash
git add src/app.rs
git commit -m "feat: add server echo matching logic"
```

---

### Task 6: 集成预测到on_send_key回调

**Files:**
- Modify: `src/app.rs:8092-8360` (on_send_key回调)

**Interfaces:**
- Consumes: `is_predictable()`, `apply_prediction()`
- Produces: 修改on_send_key回调，添加预测逻辑

- [ ] **Step 1: 在on_send_key回调中添加预测逻辑**

找到`on_send_key`回调（约第8092行），在`key_to_pty_bytes`调用之后、`send_raw`调用之前添加：

```rust
            // 本地echo预测：立即应用预测
            if let Some(predictable_action) = is_predictable(key.as_str(), ctrl, alt) {
                if let Some(h) = term_buf(&bufs, tab_id.as_str()) {
                    let mut buf = h.lock().unwrap();
                    buf.apply_prediction(predictable_action);
                }
            }
```

- [ ] **Step 2: 编译验证**

Run: `cargo check`
Expected: 编译通过，无错误

- [ ] **Step 3: 提交**

```bash
git add src/app.rs
git commit -m "feat: integrate prediction into on_send_key callback"
```

---

### Task 7: 集成预测匹配到ingest流程

**Files:**
- Modify: `src/app.rs:9960-9969` (ingest方法)

**Interfaces:**
- Consumes: `process_server_echo()`
- Produces: 修改ingest方法，在处理服务器数据前先处理预测匹配

- [ ] **Step 1: 修改ingest方法**

找到`ingest`方法（约第9960行），在方法开头添加预测匹配逻辑：

```rust
    fn ingest(&mut self, input: &[u8]) {
        // 处理服务器echo，匹配/修正预测
        self.process_server_echo(input);
        
        // 原有的处理逻辑
        let bytes = self.rewrite_hvp(input);
        self.raw.extend(bytes.iter().copied());
        self.cap_raw();
        self.feed_batched(&bytes);
    }
```

- [ ] **Step 2: 编译验证**

Run: `cargo check`
Expected: 编译通过，无错误

- [ ] **Step 3: 提交**

```bash
git add src/app.rs
git commit -m "feat: integrate prediction matching into ingest flow"
```

---

### Task 8: 添加单元测试

**Files:**
- Modify: `src/app.rs` (测试模块)

**Interfaces:**
- 测试: `is_predictable()`, `apply_prediction()`, `process_server_echo()`

- [ ] **Step 1: 添加预测功能单元测试**

在`src/app.rs`的测试模块中添加：

```rust
#[cfg(test)]
mod prediction_tests {
    use super::*;
    
    #[test]
    fn test_is_predictable() {
        // 普通字符
        assert!(is_predictable("a", false, false).is_some());
        assert!(is_predictable("Z", false, false).is_some());
        assert!(is_predictable("1", false, false).is_some());
        assert!(is_predictable("!", false, false).is_some());
        
        // 退格键
        assert!(is_predictable("\u{0008}", false, false).is_some());
        assert!(is_predictable("\u{007F}", false, false).is_some());
        
        // 方向键
        assert!(is_predictable("\u{F700}", false, false).is_some()); // Up
        assert!(is_predictable("\u{F701}", false, false).is_some()); // Down
        assert!(is_predictable("\u{F702}", false, false).is_some()); // Left
        assert!(is_predictable("\u{F703}", false, false).is_some()); // Right
        
        // 不可预测
        assert!(is_predictable("", false, false).is_none()); // 空字符串
        assert!(is_predictable("a", true, false).is_none()); // Ctrl
        assert!(is_predictable("a", false, true).is_none()); // Alt
        assert!(is_predictable("\t", false, false).is_none()); // Tab
        assert!(is_predictable("\n", false, false).is_none()); // Enter
        assert!(is_predictable("\u{001B}", false, false).is_none()); // Escape
    }
    
    #[test]
    fn test_prediction_timeout() {
        let mut buf = TermBuffer {
            // ... 初始化其他字段 ...
            predictions: std::collections::VecDeque::new(),
            prediction_timeout: std::time::Duration::from_millis(100),
        };
        
        // 添加一个预测
        buf.apply_prediction(PredictionAction::Insert('a'));
        assert_eq!(buf.predictions.len(), 1);
        assert!(!buf.predictions[0].expired);
        
        // 模拟超时（设置创建时间为过去）
        buf.predictions[0].created_at = std::time::Instant::now() - std::time::Duration::from_millis(200);
        
        // 检查超时
        buf.check_prediction_timeouts();
        assert!(buf.predictions[0].expired);
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test prediction_tests`
Expected: 所有测试通过

- [ ] **Step 3: 提交**

```bash
git add src/app.rs
git commit -m "test: add prediction unit tests"
```

---

### Task 9: 完整集成测试

**Files:**
- Test: 手动测试

**Interfaces:**
- 测试: 完整的SSH输入场景

- [ ] **Step 1: 编译release版本**

Run: `cargo build --release`
Expected: 编译成功

- [ ] **Step 2: 测试普通字符输入**

1. 启动meatshell
2. 连接到SSH服务器
3. 快速输入普通字符（如"hello world"）
4. 验证：字符立即显示，无延迟

- [ ] **Step 3: 测试退格键**

1. 输入一些字符
2. 按退格键删除
3. 验证：字符立即删除，无延迟

- [ ] **Step 4: 测试方向键**

1. 输入一些字符
2. 按左右方向键移动光标
3. 验证：光标立即移动，无延迟

- [ ] **Step 5: 测试边界情况**

1. 测试密码输入（服务器不echo）
2. 测试Tab补全
3. 测试快速连续输入

- [ ] **Step 6: 提交最终版本**

```bash
git add -A
git commit -m "feat: complete local echo prediction implementation"
```

---

## 实现注意事项

1. **vt100库限制**：vt100库不支持直接修改屏幕内容，需要找到合适的方法来应用预测
2. **光标位置管理**：预测会改变光标位置，需要正确管理预测前后的光标状态
3. **性能考虑**：预测逻辑应该轻量，避免阻塞UI线程
4. **错误恢复**：预测错误时应该能够正确恢复，避免显示错乱
