# 国产 AI 推理平台技术选型调研报告

> **日期：** 2026-06-22
> **背景：** 调研多框架（ONNX/PyTorch/TensorFlow）推理服务在国产硬件（NPU/DCU 等）上的技术方案

---

## 1. 目标需求

- 支持多推理框架：ONNX Runtime、PyTorch、TensorFlow
- 支持多种硬件：NVIDIA GPU、海光 DCU、华为 NPU（昇腾）、平头哥含光 800
- 低延迟推理：减少跨进程/跨框架的数据传输开销
- 语言选型：Rust / Go / Python 或组合方案

---

## 2. 推理框架语言绑定现状

### 2.1 各框架的 Rust 绑定

| 框架 | Rust 方案 | 说明 |
|------|-----------|------|
| ONNX Runtime | `ort` crate | 官方 C API 绑定，生产可用 |
| PyTorch | `tch-rs` | 调 libtorch，版本绑定严格 |
| TensorFlow | `tensorflow-rust` | 官方 C 绑定，基本可用 |
| 纯 Rust 推理 | `candle` / `burn` | HuggingFace 出品，快速成长 |

### 2.2 各框架的 Go 绑定

| 框架 | Go 方案 | 说明 |
|------|---------|------|
| ONNX Runtime | `onnxruntime-go`（CGO） | 可用，但有 CGO 开销 |
| PyTorch | 无成熟方案 | 通常走 gRPC 调 Python |
| TensorFlow | `tensorflow/go` | 官方支持 |

### 2.3 核心发现

> **Rust/Go 调用的都是同一个底层 C/C++ 动态库**，不存在"Rust 版推理框架"。Python wheel 包（如 `onnxruntime-dcu`、`torch-dcu`）中的 `.so` 可提取出来被 Rust/Go 直接链接。

---

## 3. 国产硬件支持对比

### 3.1 硬件生态全景

| 硬件 | 厂商 | 技术路线 | ONNX Runtime | PyTorch | Rust/Go 可调性 |
|------|------|---------|-------------|---------|---------------|
| NVIDIA GPU | NVIDIA | CUDA | ✅ CUDA EP | ✅ 一等公民 | ✅ tch-rs / ort |
| 海光 DCU | 海光 | ROCm/HIP 兼容 | ✅ ROCm EP | ✅ torch-dcu | ✅ 提取 .so 链接 |
| 华为 NPU | 华为 | 自研 CANN | ⚠️ CANN EP | ⚠️ torch_npu | ⚠️ AscendCL C API 可调 |
| 含光 800 | 平头哥/阿里 | 自研 EAIS | ❌ 无 EP | ❌ 无 | ❌ 几乎不可调 |

### 3.2 海光 DCU

- **技术路线：** 兼容 AMD ROCm/HIP，与 AMD GPU 生态高度一致
- **ONNX Runtime：** 官方提供 ROCm Execution Provider，可直接使用
- **PyTorch：** 提供 `torch-dcu` pip 包，底层是 ROCm 版 libtorch
- **Rust 调用：** 从 pip 包提取 `libonnxruntime.so` / `libtorch.so`，通过 `ort` / `tch-rs` 直接链接
- **算子覆盖：** ≈ ROCm 原生水平，主流算子全覆盖
- **适配难度：** ⭐⭐ 低（ROCm 兼容）

```rust
// Rust 调用海光 DCU 上的 ONNX Runtime
use ort::{Session, execution_providers::ROCmExecutionProvider};

let model = Session::builder()?
    .with_execution_providers([ROCmExecutionProvider::default().build()])?
    .commit_from_file("model.onnx")?;
```

### 3.3 华为 NPU（昇腾）

- **技术路线：** 自研 CANN（Compute Architecture for Neural Networks）
- **ONNX Runtime：** 有 CANN Execution Provider，需自行编译 ORT
- **PyTorch：** 提供 `torch_npu`，华为持续维护，主流模型已适配
- **C SDK：** AscendCL 文档完整，可被 Rust FFI 调用
- **算子覆盖：** ≈ 90%+，Transformer/LLM 有专项优化
- **适配难度：** ⭐⭐⭐ 中等（文档齐全，社区活跃）

**华为优势：**
- Atlas 200/300/500 开发板可购买，支持自建部署
- 昇腾社区活跃，100+ 常见模型已适配
- LLaMA、ChatGLM 等 LLM 已验证

### 3.4 平头哥含光 800

- **技术路线：** 自研 ASIC + EAIS 推理服务（深度绑定阿里云）
- **ONNX Runtime：** ❌ 无 EP，不支持直接运行 ONNX 模型
- **PyTorch：** ❌ 无原生后端
- **模型转换：** ONNX → AMCT 工具 → 含光私有 `.om` 格式
- **算子覆盖：** ≈ 70-80%，Transformer 支持有限，LLM 基本不支持
- **适配难度：** ⭐⭐⭐⭐⭐ 极高

**含光问题：**
- 文档极少，社区几乎没有
- 几乎只能在阿里云上使用，无自建部署选项
- 动态 shape 支持差，需固定输入尺寸
- 自定义算子需手写 C++ 注册
- 无 PyTorch 后端，只有 ONNX → OM 单链路

### 3.5 适配难度排序（从易到难）

```
海光 DCU ⭐⭐  →  华为 NPU ⭐⭐⭐  →  含光 800 ⭐⭐⭐⭐⭐
  ROCm 兼容       CANN 生态完善        封闭 + 文档少
```

---

## 4. 架构方案对比

### 4.1 方案一：Go/Rust + Python 跨进程

```
Go/Rust（调度网关）
  ↕  gRPC / Socket
Python Worker（各厂商 SDK）
```

| 优点 | 缺点 |
|------|------|
| Python SDK 生态全保留 | 图像传输延迟 1-5ms/张 |
| 开发简单 | 跨进程 IPC 开销大 |
| | 需维护两套运行时 |

### 4.2 方案二：共享内存优化

```
Go/Rust ←──mmap 共享内存──→ Python Worker
```

| 优点 | 缺点 |
|------|------|
| 图像零拷贝（~0.01ms） | 仍有双进程管理成本 |
| | Python 运行时部署开销 |

### 4.3 方案三：Rust + pyo3 嵌入 Python（推荐）

```
Rust 进程
  ├── 主逻辑（Rust）
  └── 嵌入 CPython（pyo3）
       └── 直接调用各厂商 Python SDK
           同进程，零拷贝
```

| 优点 | 缺点 |
|------|------|
| 零 IPC 开销 | 需携带 Python 运行时 |
| Python SDK 生态全保留 | pyo3 学习成本 |
| 主框架 Rust，性能有保障 | |

### 4.4 方案四：C FFI 直调（极致性能）

```
Rust → FFI → 各厂商 C SDK（AscendCL / HIP / EAIS）
```

| 优点 | 缺点 |
|------|------|
| 最高性能，无 Python 依赖 | 需逐个硬件适配 C API |
| | 含光 EAIS 文档差，适配困难 |
| | AscendCL 可行，但 HIP 生态不如直接用 ORT |

### 4.5 方案五：ONNX 统一格式（最务实）

```
所有模型 → 导出 ONNX → Rust ort crate → 各硬件 EP
```

| 优点 | 缺点 |
|------|------|
| 只维护一套推理代码 | 部分模型导出 ONNX 有算子损失 |
| ort crate 成熟 | 含光无 EP，此方案不可用 |
| 海光 ROCm EP / 华为 CANN EP 均可用 | |

---

## 5. 推荐方案

### 5.1 分层架构

```
┌─────────────────────────────────────────┐
│           调度 / 网关层（Go 或 Rust）      │
│  请求路由 · 负载均衡 · 模型管理 · 监控     │
├─────────────────────────────────────────┤
│           推理引擎层（Rust）               │
│  ┌─────────────────────────────────┐    │
│  │  统一推理接口 trait InferenceEngine │   │
│  ├────────┬────────┬───────┬───────┤    │
│  │  ort   │tch-rs  │pyo3   │C FFI  │    │
│  │(ONNX)  │(PT)    │(Python)│(AscendCL)│  │
│  └────────┴────────┴───────┴───────┘    │
├─────────────────────────────────────────┤
│           硬件适配层                      │
│  NVIDIA CUDA · 海光 ROCm · 华为 CANN     │
└─────────────────────────────────────────┘
```

### 5.2 各硬件的接入策略

| 硬件 | 推荐接入方式 | 理由 |
|------|------------|------|
| NVIDIA GPU | `ort` CUDA EP / `tch-rs` | 原生支持，最成熟 |
| 海光 DCU | `ort` ROCm EP | 从 pip 包提取 .so，Rust 直接调用 |
| 华为 NPU | pyo3 嵌入 Python 调 torch_npu | CANN C API 可用但 Python 生态更完整 |
| 含光 800 | **暂不接入** | ROI 太低，除非客户强绑定阿里云 |

### 5.3 语言选型建议

| 角色 | 推荐语言 | 理由 |
|------|---------|------|
| 调度/网关 | Go | 开发快、团队好招、K8s 生态原生 |
| 推理引擎 | Rust | `ort`/`tch-rs` 成熟、FFI 调 C 库方便、无 GC 延迟 |
| 硬件适配 | Rust + pyo3（兜底 Python） | 优先 C FFI，不可用时走 pyo3 |

---

## 6. 风险与注意事项

1. **libtorch 版本绑定** — `tch-rs` 对 libtorch 版本有严格要求，海光/华为的 wheel 包版本要对齐
2. **DTK/ROCm 版本** — 海光 DTK 不完全等于 AMD ROCm，升级后需重新验证 .so 兼容性
3. **算子 fallback** — 部分算子不支持时可能静默 fallback 到 CPU，需主动检测性能断崖
4. **动态 shape** — 含光几乎不支持，海光/华为有限制，建议模型固定输入尺寸
5. **量化精度** — INT8 量化在国产硬件上精度下降可能比 GPU 明显，需反复校准

---

## 7. 结论

1. **ONNX 统一格式 + Rust `ort` crate 是最务实的推理方案**，覆盖海光 DCU 和华为 NPU
2. **含光 800 是适配成本最高的选项**，比昇腾困难一个数量级，建议暂缓
3. **Rust + pyo3 嵌入 Python 是兼顾性能和生态的最优架构**，零 IPC 开销 + 全硬件覆盖
4. **Go 适合做调度网关层**，Rust 适合做推理引擎层，Python 做硬件 SDK 兜底层
5. **海光 DCU 是国产硬件中 Rust 友好度最高的**，ROCm 兼容路线让接入成本极低
