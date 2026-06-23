# Image Viewer Design / 图片查看器设计

## Overview / 概述

在 SFTP 文件面板中增加内置图片查看器，支持右键预览、左右翻页、缩放平移。

Add an in-app image viewer to the SFTP file panel: right-click "Preview", navigate
with arrow keys / on-screen buttons, zoom + pan with mouse wheel / drag.

## Requirements / 需求

### Trigger / 触发方式

- **右键菜单**新增"预览"选项，仅对图片文件显示（根据扩展名判断）
- 双击行为不变（仍为下载）
- 图片扩展名：`.png` `.jpg` `.jpeg` `.gif` `.webp` `.bmp` `.ico` `.tiff` `.tif` `.svg`

### Navigation / 翻页

- 翻页范围 = 当前目录下所有图片文件（按列表当前排序）
- **左右箭头键**翻页
- 屏幕左右两侧**悬浮翻页按钮**（鼠标悬停时显示）
- 首张/末张时按钮灰显

### Zoom & Pan / 缩放平移

- **鼠标滚轮**缩放（以光标为中心）
- **拖拽**平移（缩放后图片超出视口时）
- **双击**重置为 fit-to-window

### Viewer UI / 查看器界面

- 暗色半透明浮层（参照现有 editor overlay）
- 可拖拽、可调整大小（复用 editor 的 draggable/resizable 模式）
- 标题栏：文件名 + 序号（如 "3/12"）+ 缩放比例 + 关闭按钮
- Esc 关闭

### Settings / 设置

- 图片预览最大文件大小：默认 100MB，可在设置中调整
- 存储在 ConfigStore 中

## Architecture / 架构

### Data Flow / 数据流

```
1. SFTP list_dir 完成后
   → Rust 侧从文件列表中筛选图片 → 维护 image_entries: Vec<String>

2. 右键"预览"
   → sftp.read_bytes(path)  下载图片到内存（上限 100MB）
   → image::load_from_memory()  解码
   → SharedPixelBuffer<Rgba8Pixel> → slint::Image
   → 打开 viewer 浮层

3. 翻页（左/右）
   → index ± 1 → read_bytes(new_path) → 解码 → 显示

4. 缩放/平移
   → Slint 侧处理：滚轮修改 scale，拖拽修改 offset
```

### New Components / 新增组件

#### Rust Side

1. **`SftpCommand::ReadBytes`** — 新增 SFTP 命令，读取远程文件到 `Vec<u8>`
   - 类似 `ReadText` 但不做 UTF-8 转换
   - 大小检查：上限可配置（默认 100MB）

2. **`SessionEvent::SftpImageLoaded`** — 新增事件
   ```rust
   SftpImageLoaded {
       path: String,
       name: String,
       index: usize,        // 在 image_entries 中的位置
       total: usize,        // image_entries 总数
       width: u32,
       height: u32,
       data: Vec<u8>,       // RGBA pixels
       error: Option<String>,
   }
   ```

3. **`ConfigStore`** — 新增字段
   - `image_preview_max_bytes: u64` — 默认 100MB

#### Slint Side

4. **`ImageViewer`** 组件（新建 `ui/image_viewer.slint`）
   - 参照 editor overlay 的布局
   - `Image` 元素显示图片，`image-fit: contain`
   - 滚轮缩放 + 拖拽平移状态
   - 左右翻页按钮
   - 标题栏：文件名 + 序号 + 缩放比例 + 关闭

5. **`AppWindow`** 新增属性
   - `image-viewer-open: bool`
   - `image-viewer-name: string`
   - `image-viewer-index: int`
   - `image-viewer-total: int`
   - `image-viewer-scale: float`
   - `image-viewer-image: image`  (slint::Image)
   - `image-viewer-max-bytes: int`  (设置项)

6. **`SftpPanel`** 新增
   - `preview(path: string)` callback — 右键"预览"
   - 仅对图片文件的右键菜单显示

### Existing Code to Reuse / 复用

| 模块 | 复用内容 |
|------|---------|
| `wallpaper.rs` | `image::load_from_memory()` → `SharedPixelBuffer` → `slint::Image` 转换流水线 |
| `sftp.rs` `read_text_guarded()` | 参照其文件读取逻辑，改为 `read_bytes()` |
| `app.slint` editor overlay | 浮层的 draggable/resizable 模式 |
| `sftp_panel.slint` context menu | 新增"预览"菜单项 |

### File Extensions / 图片扩展名判断

```rust
fn is_image_file(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), 
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tiff" | "tif" | "svg"
    )
}
```

## Error Handling / 错误处理

| 场景 | 处理 |
|------|------|
| 文件超过大小限制 | 提示"文件过大（XX MB），超过预览限制（YY MB）" |
| 解码失败 | 提示"无法解码该图片" |
| SFTP 读取失败 | 提示网络/权限错误 |
| 翻页时加载失败 | 显示错误，停留在当前图片 |

## Testing / 测试

- [ ] 右键菜单：图片文件显示"预览"，非图片文件不显示
- [ ] 预览：PNG/JPEG/WebP/BMP 正常显示
- [ ] 翻页：左右箭头键 + 按钮，首张/末张灰显
- [ ] 缩放：滚轮缩放以光标为中心
- [ ] 平移：缩放后拖拽平移
- [ ] 双击重置：恢复 fit-to-window
- [ ] 大文件限制：超过限制提示错误
- [ ] 设置：修改最大文件大小生效
- [ ] Esc 关闭
