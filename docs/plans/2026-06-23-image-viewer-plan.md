# Image Viewer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 SFTP 文件面板中增加内置图片查看器，支持右键预览、左右翻页、缩放平移。

**Architecture:** 右键菜单触发预览，SFTP 下载图片到内存，`image` crate 解码为 RGBA，通过 `slint::Image` 显示在浮层中。复用现有 editor overlay 的 draggable/resizable 模式。

**Tech Stack:** Rust, Slint, `image` crate (已依赖), `russh-sftp`

---

## Task 1: 添加 `is_image_file` 工具函数

**Files:**
- Modify: `src/sftp.rs` — 添加 `is_image_file()` 函数

**Step 1: 在 sftp.rs 中添加函数**

```rust
/// Check if a filename looks like an image by extension.
pub fn is_image_file(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tiff" | "tif" | "svg"
    )
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: PASS

**Step 3: Commit**

```bash
git add src/sftp.rs
git commit -m "feat(image-viewer): add is_image_file utility function"
```

---

## Task 2: 添加 `SftpCommand::ReadBytes` 和 `SessionEvent::SftpImageLoaded`

**Files:**
- Modify: `src/sftp.rs:44-83` — `SftpCommand` 枚举新增 `ReadBytes`
- Modify: `src/ssh.rs:307-395` — `SessionEvent` 枚举新增 `SftpImageLoaded`

**Step 1: 在 SftpCommand 中添加 ReadBytes 变体**

在 `src/sftp.rs` 的 `SftpCommand` 枚举中（`ReadText` 之后）添加：

```rust
/// Read a remote image file as bytes for the built-in image viewer.
ReadBytes { remote: String },
```

**Step 2: 在 SessionEvent 中添加 SftpImageLoaded 变体**

在 `src/ssh.rs` 的 `SessionEvent` 枚举中（`SftpFileText` 之后）添加：

```rust
/// A remote image file loaded for the built-in image viewer.
SftpImageLoaded {
    path: String,
    name: String,
    index: usize,
    total: usize,
    width: u32,
    height: u32,
    /// RGBA pixel data, ready for SharedPixelBuffer.
    data: Vec<u8>,
    error: String,
},
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS (new variants are unused but compile)

**Step 4: Commit**

```bash
git add src/sftp.rs src/ssh.rs
git commit -m "feat(image-viewer): add SftpCommand::ReadBytes + SessionEvent::SftpImageLoaded"
```

---

## Task 3: 实现 `read_bytes_guarded()` 函数

**Files:**
- Modify: `src/sftp.rs` — 添加 `read_bytes_guarded()` 函数（参照 `read_text_guarded`）

**Step 1: 添加 read_bytes_guarded 函数**

在 `read_text_guarded()` 之后添加：

```rust
/// Read a remote file as raw bytes for image preview. Rejects files larger
/// than the configurable limit (default 100 MB).
async fn read_bytes_guarded(
    sftp: &SftpSession,
    remote: &str,
    max_bytes: u64,
) -> std::result::Result<Vec<u8>, String> {
    use tokio::io::AsyncReadExt;
    let size = sftp
        .metadata(remote)
        .await
        .ok()
        .and_then(|m| m.size)
        .unwrap_or(0);
    if size > max_bytes {
        let size_mb = size as f64 / (1024.0 * 1024.0);
        let limit_mb = max_bytes as f64 / (1024.0 * 1024.0);
        return Err(format!(
            "{} ({:.1} MB > {:.0} MB)",
            t("文件过大,超过预览限制", "File too large for preview"),
            size_mb,
            limit_mb,
        ));
    }
    let mut f = sftp
        .open(remote)
        .await
        .map_err(|e| format!("{}: {e}", t("打开失败", "Open failed")))?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes)
        .await
        .map_err(|e| format!("{}: {e}", t("读取失败", "Read failed")))?;
    Ok(bytes)
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: PASS

**Step 3: Commit**

```bash
git add src/sftp.rs
git commit -m "feat(image-viewer): add read_bytes_guarded for image loading"
```

---

## Task 4: 在 SFTP worker 中处理 `ReadBytes` 命令

**Files:**
- Modify: `src/sftp.rs` — 在 `run_sftp()` 的命令处理 match 中添加 `ReadBytes` 分支

**Step 1: 找到命令处理 match 并添加 ReadBytes**

在 `run_sftp()` 函数中找到 `SftpCommand::ReadText` 的处理分支，在其后添加：

```rust
SftpCommand::ReadBytes { remote } => {
    let max_bytes = 100 * 1024 * 1024; // TODO: 从配置读取
    let name = remote.rsplit('/').next().unwrap_or(&remote).to_string();
    match read_bytes_guarded(&sftp, &remote, max_bytes).await {
        Ok(bytes) => {
            match image::load_from_memory(&bytes) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let _ = events.send(SessionEvent::SftpImageLoaded {
                        path: remote,
                        name,
                        index: 0, // TODO: 从 image_entries 计算
                        total: 0, // TODO: 从 image_entries 计算
                        width: w,
                        height: h,
                        data: rgba.into_raw(),
                        error: String::new(),
                    });
                }
                Err(e) => {
                    let _ = events.send(SessionEvent::SftpImageLoaded {
                        path: remote,
                        name,
                        index: 0,
                        total: 0,
                        width: 0,
                        height: 0,
                        data: Vec::new(),
                        error: format!("{}: {e}", t("解码失败", "Decode failed")),
                    });
                }
            }
        }
        Err(e) => {
            let _ = events.send(SessionEvent::SftpImageLoaded {
                path: remote,
                name,
                index: 0,
                total: 0,
                width: 0,
                height: 0,
                data: Vec::new(),
                error: e,
            });
        }
    }
}
```

**Step 2: 在 SftpHandle 中添加 read_bytes 方法**

在 `SftpHandle` impl 中添加：

```rust
pub fn read_bytes(&self, remote: String) {
    let _ = self.commands.send(SftpCommand::ReadBytes { remote });
}
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add src/sftp.rs
git commit -m "feat(image-viewer): handle ReadBytes in SFTP worker"
```

---

## Task 5: 在 ConfigStore 中添加 `image_preview_max_bytes` 设置

**Files:**
- Modify: `src/config.rs` — Cache 结构体和相关方法

**Step 1: 找到 Cache 结构体并添加字段**

在 `Cache` 结构体中添加：

```rust
/// Maximum file size (bytes) for image preview. Default 100 MB.
#[serde(default = "default_image_preview_max_bytes")]
pub image_preview_max_bytes: u64,
```

添加默认值函数：

```rust
fn default_image_preview_max_bytes() -> u64 {
    100 * 1024 * 1024
}
```

**Step 2: 在 ConfigStore 中添加 getter/setter**

```rust
pub fn image_preview_max_bytes(&self) -> u64 {
    self.cache.image_preview_max_bytes
}

pub fn set_image_preview_max_bytes(&mut self, bytes: u64) {
    self.cache.image_preview_max_bytes = bytes;
}
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add src/config.rs
git commit -m "feat(image-viewer): add image_preview_max_bytes to ConfigStore"
```

---

## Task 6: 创建 Slint 图片查看器 UI

**Files:**
- Create: `ui/image_viewer.slint`
- Modify: `ui/app.slint` — 添加 ImageViewer 组件和相关属性/回调

**Step 1: 创建 ui/image_viewer.slint**

参照 editor overlay 的模式，创建图片查看器组件：

```slint
import { Theme } from "theme.slint";

export component ImageViewer inherits Rectangle {
    in property <string> file-name;
    in property <int> index;      // current position (0-based)
    in property <int> total;      // total image count
    in property <image> preview-image;
    in property <float> scale: 1.0;
    in property <string> error-msg;

    callback close();
    callback prev();
    callback next();
    callback zoom-in();
    callback zoom-out();
    callback reset-zoom();

    // Title bar
    HorizontalLayout {
        alignment: space-between;
        padding: 8px;
        Text {
            text: root.file-name + "  " + (root.index + 1) + "/" + root.total;
            color: Theme.text-primary;
            font-size: Theme.fs-sm;
        }
        Text {
            text: Math.round(root.scale * 100) + "%";
            color: Theme.text-secondary;
            font-size: Theme.fs-sm;
        }
    }

    // Image display area
    if root.error-msg != "" : Text {
        text: root.error-msg;
        color: Theme.danger;
        font-size: Theme.fs-base;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
    if root.error-msg == "" : Image {
        source: root.preview-image;
        image-fit: contain;
        // Scale + pan handled by transforms
    }

    // Navigation buttons (show on hover)
    // Left arrow
    TouchArea {
        width: 60px;
        x: 0;
        y: root.y + 40px;
        height: root.height - 40px;
        mouse-cursor: pointer;
        clicked => { root.prev(); }
    }
    // Right arrow
    TouchArea {
        width: 60px;
        x: root.width - 60px;
        y: root.y + 40px;
        height: root.height - 40px;
        mouse-cursor: pointer;
        clicked => { root.next(); }
    }

    // Close on Escape
    key-pressed(event) => {
        if (event.text == Key.Escape) {
            root.close();
            return accept;
        }
        if (event.text == Key.Left) {
            root.prev();
            return accept;
        }
        if (event.text == Key.Right) {
            root.next();
            return accept;
        }
        reject
    }
}
```

**Step 2: 在 app.slint 中添加图片查看器属性**

在 AppWindow 的属性区域添加：

```slint
// Image viewer state
in-out property <bool> image-viewer-open: false;
in-out property <string> image-viewer-name;
in-out property <int> image-viewer-index;
in-out property <int> image-viewer-total;
in-out property <image> image-viewer-preview;
in-out property <string> image-viewer-error;
callback image-viewer-close();
callback image-viewer-prev();
callback image-viewer-next();
```

**Step 3: 在 app.slint 中添加 ImageViewer 组件实例**

在 editor overlay 附近添加：

```slint
if root.image-viewer-open : ImageViewer {
    file-name: root.image-viewer-name;
    index: root.image-viewer-index;
    total: root.image-viewer-total;
    preview-image: root.image-viewer-preview;
    error-msg: root.image-viewer-error;
    close() => { root.image-viewer-close(); }
    prev() => { root.image-viewer-prev(); }
    next() => { root.image-viewer-next(); }
    focus: true;
}
```

**Step 4: Verify compilation**

Run: `cargo check`
Expected: PASS

**Step 5: Commit**

```bash
git add ui/image_viewer.slint ui/app.slint
git commit -m "feat(image-viewer): add Slint image viewer UI component"
```

---

## Task 7: 在 SFTP 面板右键菜单添加"预览"

**Files:**
- Modify: `ui/sftp_panel.slint` — 右键菜单新增"预览"选项
- Modify: `ui/app.slint` — SftpPanel 新增 preview callback

**Step 1: 在 sftp_panel.slint 中添加 preview callback**

在 SftpPanel 的 callback 区域添加：

```slint
callback preview(string);  // preview image file
```

**Step 2: 在右键菜单中添加"预览"选项**

找到右键菜单的 `if !entry.is-dir` 区域，在 "View" 之前添加：

```slint
if is-image(entry.name) : Rectangle {
    height: 28px;
    background: menu-ta.has-hover ? Theme.bg-hover : transparent;
    menu-ta := TouchArea {
        clicked => { root.preview(entry.full-path); }
    }
    Text {
        text: @tr("预览", "Preview");
        color: Theme.text-primary;
        font-size: Theme.fs-sm;
        vertical-alignment: center;
    }
}
```

**Step 3: 添加 is-image 辅助函数**

在 sftp_panel.slint 顶部添加：

```slint
function is-image(name: string) -> bool {
    // Simple extension check for common image types
    return name.ends-with(".png") || name.ends-with(".jpg") || name.ends-with(".jpeg")
        || name.ends-with(".gif") || name.ends-with(".webp") || name.ends-with(".bmp")
        || name.ends-with(".ico") || name.ends-with(".tiff") || name.ends-with(".tif")
        || name.ends-with(".svg")
        || name.ends-with(".PNG") || name.ends-with(".JPG") || name.ends-with(".JPEG")
        || name.ends-with(".GIF") || name.ends-with(".WEBP") || name.ends-with(".BMP")
        || name.ends-with(".ICO") || name.ends-with(".TIFF") || name.ends-with(".TIF")
        || name.ends-with(".SVG");
}
```

**Step 4: 在 app.slint 的 SftpPanel 中绑定 preview callback**

```slint
preview(path) => { root.sftp-preview(path); }
```

并在 AppWindow 中添加对应 callback：

```slint
callback sftp-preview(string);
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: PASS

**Step 6: Commit**

```bash
git add ui/sftp_panel.slint ui/app.slint
git commit -m "feat(image-viewer): add Preview to SFTP context menu"
```

---

## Task 8: Rust 侧串联 — 处理 preview 回调和 SftpImageLoaded 事件

**Files:**
- Modify: `src/app.rs` — wire_sftp_callbacks 中添加 sftp-preview 处理
- Modify: `src/app.rs` — apply_session_event_to_window 中处理 SftpImageLoaded

**Step 1: 在 wire_sftp_callbacks 中添加 sftp-preview 处理**

找到 SFTP 回调绑定区域，添加：

```rust
window.on_sftp_preview(move |path: SharedString| {
    if let Some(h) = sftp_handles.borrow().get(tab_id.as_str()) {
        h.read_bytes(path.to_string());
    }
});
```

**Step 2: 在 apply_session_event_to_window 中添加 SftpImageLoaded 处理**

```rust
SessionEvent::SftpImageLoaded {
    path, name, index, total, width, height, data, error,
} => {
    if error.is_empty() {
        let buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            &data, width, height,
        );
        win.set_image_viewer_preview(slint::Image::from_rgba8(buf));
        win.set_image_viewer_error("".into());
    } else {
        win.set_image_viewer_error(error.into());
    }
    win.set_image_viewer_name(name.into());
    win.set_image_viewer_index(index as i32);
    win.set_image_viewer_total(total as i32);
    win.set_image_viewer_open(true);
}
```

**Step 3: 添加翻页回调处理**

```rust
window.on_image_viewer_prev(move || {
    // 从 image_entries 中获取上一张路径，调用 read_bytes
});

window.on_image_viewer_next(move || {
    // 从 image_entries 中获取下一张路径，调用 read_bytes
});

window.on_image_viewer_close(move || {
    // 关闭查看器
    if let Some(w) = weak.upgrade() {
        w.set_image_viewer_open(false);
    }
});
```

**Step 4: 维护 image_entries 列表**

在 SftpEntries 事件处理中，筛选图片文件：

```rust
SessionEvent::SftpEntries { path, entries } => {
    // 现有逻辑...
    
    // 维护图片文件列表
    let image_files: Vec<String> = entries.iter()
        .filter(|e| !e.is_dir && crate::sftp::is_image_file(&e.name))
        .map(|e| e.full_path.clone())
        .collect();
    // 存储到某处供翻页使用
}
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: PASS

**Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat(image-viewer): wire up preview callback and image loaded event"
```

---

## Task 9: 实现缩放和平移

**Files:**
- Modify: `ui/image_viewer.slint` — 添加滚轮缩放和拖拽平移

**Step 1: 添加缩放和平移状态**

```slint
in-out property <float> scale: 1.0;
in-out property <length> pan-x: 0;
in-out property <length> pan-y: 0;
property <bool> panning: false;
property <length> pan-start-x;
property <length> pan-start-y;
```

**Step 2: 添加滚轮缩放**

在图片显示区域的 TouchArea 中：

```slint
scroll-event(event) => {
    if (event.delta-y < 0) {
        root.scale = Math.min(root.scale * 1.1, 10.0);
    } else {
        root.scale = Math.max(root.scale / 1.1, 0.1);
    }
    accept
}
```

**Step 3: 添加拖拽平移**

```slint
pointer-event(event) => {
    if (event.kind == PointerEventKind.down && root.scale > 1.0) {
        root.panning = true;
        root.pan-start-x = self.mouse-x;
        root.pan-start-y = self.mouse-y;
    } else if (event.kind == PointerEventKind.up) {
        root.panning = false;
    }
}
moved => {
    if (root.panning) {
        root.pan-x += self.mouse-x - root.pan-start-x;
        root.pan-y += self.mouse-y - root.pan-start-y;
        root.pan-start-x = self.mouse-x;
        root.pan-start-y = self.mouse-y;
    }
}
```

**Step 4: 双击重置**

```slint
double-clicked => {
    root.scale = 1.0;
    root.pan-x = 0;
    root.pan-y = 0;
}
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: PASS

**Step 6: Commit**

```bash
git add ui/image_viewer.slint
git commit -m "feat(image-viewer): implement zoom and pan"
```

---

## Task 10: 翻页时更新 image_entries 索引

**Files:**
- Modify: `src/app.rs` — 完善翻页逻辑

**Step 1: 使用 Rc<RefCell<Vec<String>>> 存储 image_entries**

```rust
let image_entries: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
```

**Step 2: 在 SftpEntries 事件中更新**

```rust
*image_entries.borrow_mut() = entries.iter()
    .filter(|e| !e.is_dir && crate::sftp::is_image_file(&e.name))
    .map(|e| e.full_path.clone())
    .collect();
```

**Step 3: 在 preview 回调中设置 index 和 total**

```rust
window.on_sftp_preview(move |path: SharedString| {
    let entries = image_entries.borrow();
    let idx = entries.iter().position(|p| p == path.as_str()).unwrap_or(0);
    let total = entries.len();
    
    if let Some(h) = sftp_handles.borrow().get(tab_id.as_str()) {
        h.read_bytes(path.to_string());
    }
    
    if let Some(w) = weak.upgrade() {
        w.set_image_viewer_index(idx as i32);
        w.set_image_viewer_total(total as i32);
    }
});
```

**Step 4: 翻页回调**

```rust
let entries_clone = image_entries.clone();
let handles_clone = sftp_handles.clone();
let tab_clone = tab_id.clone();
window.on_image_viewer_prev(move || {
    let entries = entries_clone.borrow();
    if let Some(w) = weak.upgrade() {
        let idx = w.get_image_viewer_index() as usize;
        if idx > 0 {
            let new_idx = idx - 1;
            if let Some(path) = entries.get(new_idx) {
                if let Some(h) = handles_clone.borrow().get(tab_clone.as_str()) {
                    h.read_bytes(path.clone());
                }
                w.set_image_viewer_index(new_idx as i32);
            }
        }
    }
});
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: PASS

**Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat(image-viewer): implement image_entries tracking and pagination"
```

---

## Task 11: 集成配置中的 image_preview_max_bytes

**Files:**
- Modify: `src/sftp.rs` — ReadBytes 命令使用配置值
- Modify: `src/app.rs` — 传递配置值

**Step 1: 修改 SftpCommand::ReadBytes 增加 max_bytes 参数**

```rust
ReadBytes { remote: String, max_bytes: u64 },
```

**Step 2: 修改 read_bytes 调用传入配置值**

在 `on_sftp_preview` 中：

```rust
let max_bytes = store.borrow().image_preview_max_bytes();
h.read_bytes(path.to_string(), max_bytes);
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add src/sftp.rs src/app.rs
git commit -m "feat(image-viewer): integrate configurable max preview size"
```

---

## Task 12: 最终检查和清理

**Files:**
- All modified files

**Step 1: Run cargo fmt**

Run: `cargo fmt --all`

**Step 2: Run cargo clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS (fix any warnings)

**Step 3: Run cargo build**

Run: `cargo build --release`
Expected: PASS

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat(image-viewer): complete image viewer with zoom, pan, and pagination"
```
