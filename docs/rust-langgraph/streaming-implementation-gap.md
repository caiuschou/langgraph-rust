# LangGraph Rust 与 Python LangGraph Streaming 实现差距分析

本文档对比 langgraph-rust 与 Python LangGraph 的 streaming 能力，标识已实现、部分实现与未实现的功能，并提供 API 对比、源码引用与实现建议。

---

## 1. 概述

Python LangGraph 提供完整的 streaming 系统，支持多种 stream 模式及 LLM token 级流式输出。langgraph-rust 目前已实现**图状态级**的 Values 和 Updates streaming，但尚未实现 Messages、Custom 等节点内流式能力。

### 1.1 设计目标对比

| 维度 | Python LangGraph | langgraph-rust |
|------|------------------|----------------|
| **流式目标** | 图状态 + LLM token + 自定义 + checkpoint/task 调试 | 图状态（Values/Updates） |
| **事件来源** | run_loop + 节点/工具（通过 callback/writer） | 仅 run_loop |
| **消费方式** | 迭代器 `for chunk in graph.stream(...)` | `ReceiverStream` + `StreamExt::next()` |

---

## 2. Stream 模式详细对比

### 2.1 汇总表

| 模式 | Python LangGraph | langgraph-rust | 状态 |
|------|------------------|----------------|------|
| **values** | 每步后流式输出完整状态 | `StreamEvent::Values`，run_loop 每节点后发送 | ✅ 已实现 |
| **updates** | 每步后流式输出增量更新（node_id + state） | `StreamEvent::Updates`，run_loop 每节点后发送 | ✅ 已实现 |
| **messages** | LLM 逐 token 流式输出 `(message_chunk, metadata)` | 有类型定义，无实际发送逻辑 | ⚠️ 仅有类型 |
| **custom** | 节点/工具通过 `get_stream_writer()` 发送自定义数据 | 有 `StreamEvent::Custom(Value)` 类型，无 writer API | ⚠️ 仅有类型 |
| **checkpoints** | checkpoint 创建时发出事件 | 无 | ❌ 未实现 |
| **tasks** | 任务开始/结束事件 | 无 | ❌ 未实现 |
| **debug** | 组合 checkpoints + tasks | 无 | ❌ 未实现 |

### 2.2 各模式说明与使用场景

#### values

- **Python**：`stream_mode="values"`，每节点完成后输出完整图状态，适用于需要重建完整 UI 的场景。
- **Rust**：`StreamEvent::Values(S)`，与 Python 语义一致。
- **用例**：进度展示、状态持久化快照。

#### updates

- **Python**：`stream_mode="updates"`，输出 `{ node_id: partial_state }`，仅包含该节点返回的增量。
- **Rust**：`StreamEvent::Updates { node_id, state }`，当前实现中 `state` 为**合并后的全量状态**（与 Python 的 partial 有差异，见 [streaming-test-design.md](./streaming-test-design.md)）。
- **用例**：按节点增量更新 UI、日志审计。

#### messages

- **Python**：`stream_mode="messages"`，LLM 调用时通过 LangChain 的 callback 逐 token 输出 `(message_chunk, metadata)`；metadata 含 `langgraph_node`、`tags` 等，可过滤特定节点或 LLM 调用。
- **Rust**：类型已定义，无发射逻辑。
- **用例**：打字机效果、实时显示 LLM 输出。

#### custom

- **Python**：`stream_mode="custom"`，节点/工具通过 `get_stream_writer()` 获取 writer，调用 `writer({"key": "value"})` 发送任意 JSON。
- **Rust**：类型已定义，无 writer 注入机制。
- **用例**：进度条、工具执行状态、非 LangChain 模型的 token 流。

#### checkpoints / tasks / debug

- **Python**：checkpoints 模式在创建 checkpoint 时发出事件；tasks 模式发出任务开始/结束；debug 为二者组合，用于调试与时间旅行。
- **Rust**：无对应 stream 模式。

---

## 3. API 与调用方式对比

### 3.1 Python LangGraph

```python
# 同步
for chunk in graph.stream(inputs, stream_mode="updates"):
    print(chunk)

# 多模式
for mode, chunk in graph.stream(inputs, stream_mode=["updates", "values"]):
    print(mode, chunk)

# 异步
async for chunk in graph.astream(inputs, stream_mode="messages"):
    print(chunk.content, end="|", flush=True)
```

- `stream_mode`：`str` 或 `list[str]`，可选 `"values"`, `"updates"`, `"messages"`, `"custom"`, `"debug"` 等。
- 多模式时 yield `(mode, chunk)` 元组。

### 3.2 langgraph-rust

```rust
use std::collections::HashSet;
use tokio_stream::StreamExt;

let modes = HashSet::from_iter([StreamMode::Updates, StreamMode::Values]);
let stream = compiled.stream(initial_state, config, modes);

while let Some(event) = stream.next().await {
    match event {
        StreamEvent::Values(s) => { /* 全量状态 */ }
        StreamEvent::Updates { node_id, state } => { /* 节点更新 */ }
        StreamEvent::Messages { chunk, metadata } => { /* 当前不会收到 */ }
        StreamEvent::Custom(v) => { /* 当前不会收到 */ }
    }
}
```

- `stream_mode`：`impl Into<HashSet<StreamMode>>`。
- 返回 `ReceiverStream<StreamEvent<S>>`，所有模式的事件混在同一流中，通过 `match` 区分。
- 多模式不区分 `(mode, chunk)`，统一为 `StreamEvent` 枚举。

### 3.3 签名对比

| 项目 | Python | Rust |
|------|--------|------|
| 方法名 | `stream`, `astream` | `stream` |
| 输入 | `inputs` (state dict), `config` (optional) | `state: S`, `config: Option<RunnableConfig>` |
| 模式参数 | `stream_mode: str \| list[str]` | `stream_mode: impl Into<HashSet<StreamMode>>` |
| 返回值 | 同步/异步迭代器 | `ReceiverStream<StreamEvent<S>>` |

---

## 4. 已实现部分（详细）

### 4.1 stream() 实现

**源码**：`langgraph/src/graph/compiled.rs` 第 163–191 行

```rust
pub fn stream(
    &self,
    state: S,
    config: Option<RunnableConfig>,
    stream_mode: impl Into<HashSet<StreamMode>>,
) -> ReceiverStream<StreamEvent<S>> {
    let (tx, rx) = mpsc::channel(128);  // 容量 128，无显式背压处理
    let graph = self.clone();
    let mode_set: HashSet<StreamMode> = stream_mode.into();

    tokio::spawn(async move {
        let mut state = state;
        let mut current_id = match graph.edge_order.first().cloned() {
            Some(id) => id,
            None => return,  // 空图直接返回，不 panic
        };
        let run_ctx = RunContext {
            config: config.clone().unwrap_or_default(),
            stream_tx: Some(tx),
            stream_mode: mode_set,
        };
        let _ = graph.run_loop_inner(&mut state, &config, &mut current_id, Some(&run_ctx)).await;
    });

    ReceiverStream::new(rx)
}
```

- 使用 `tokio::sync::mpsc` 单生产者多消费者 channel。
- 图在 `tokio::spawn` 中异步执行，stream 消费者通过 `rx` 消费事件。
- 消费者提前 drop 时，`tx` 会因 receiver 关闭而 `send` 失败，run_loop 会忽略（`let _ = tx.send(...)`），不会阻塞。

### 4.2 run_loop 中的 Values/Updates 发送

**源码**：`langgraph/src/graph/compiled.rs` 第 84–98 行

```rust
if let Some(ctx) = run_ctx {
    if let Some(tx) = &ctx.stream_tx {
        if ctx.stream_mode.contains(&StreamMode::Values) {
            let _ = tx.send(StreamEvent::Values(state.clone())).await;
        }
        if ctx.stream_mode.contains(&StreamMode::Updates) {
            let _ = tx.send(StreamEvent::Updates {
                node_id: current_id.clone(),
                state: state.clone(),
            }).await;
        }
    }
}
```

- 每节点执行完成后，根据 `stream_mode` 发送 Values 和/或 Updates。
- 发送顺序：先 Values，再 Updates（与测试 `stream_values_and_updates_both_enabled` 一致）。
- 未对 Messages、Custom 做任何 `send`。

### 4.3 RunContext

**源码**：`langgraph/src/graph/run_context.rs`

```rust
pub struct RunContext<S> {
    pub config: RunnableConfig,
    pub stream_tx: Option<mpsc::Sender<StreamEvent<S>>>,
    pub stream_mode: HashSet<StreamMode>,
}
```

- `invoke` 时 `run_ctx` 为 `None`，`stream_tx` 不参与。
- `stream` 时 `run_ctx` 为 `Some`，节点通过 `run_with_context(s, ctx)` 可访问 `ctx.stream_tx`，但当前节点默认实现忽略 `ctx`。

### 4.4 Stream 类型定义

**源码**：`langgraph/src/stream/mod.rs`

| 类型 | 行号 | 说明 |
|------|------|------|
| `StreamMode` | 12–22 | `Values`, `Updates`, `Messages`, `Custom` 四变体 |
| `StreamMetadata` | 25–30 | `langgraph_node: String` |
| `MessageChunk` | 33–36 | `content: String` |
| `StreamEvent<S>` | 38–54 | `Values(S)`, `Updates{node_id, state}`, `Messages{chunk, metadata}`, `Custom(Value)` |

---

## 5. 仅有类型、未实现发射逻辑（详细）

### 5.1 Messages

**Python 机制**：LangChain chat model 的 `invoke`/`ainvoke` 会通过 callback 机制将 token 流传给 LangGraph 的 streaming 层，自动产生 `(message_chunk, metadata)`。无需节点显式调用 stream writer。

**Rust 现状**：

| 组件 | 文件 | 说明 |
|------|------|------|
| `StreamEvent::Messages` | `stream/mod.rs` | 已定义，含 `chunk`, `metadata` |
| `ThinkNode` | `react/think_node.rs` | 仅实现 `run()`，调用 `llm.invoke()` 一次性取回内容 |
| `LlmClient` | `llm/mod.rs` | 仅有 `async fn invoke(&self, messages: &[Message]) -> Result<LlmResponse, AgentError>` |
| `ChatOpenAI` | `llm/openai.rs` | 使用 `client.chat().create()` 非流式 API |

**缺口**：

1. `LlmClient` 无 `stream()` 或 `invoke_stream()` 方法。
2. `ThinkNode` 未实现 `run_with_context`，无法访问 `ctx.stream_tx`。
3. 即使有 `stream_tx`，节点也需在 LLM 流式回调中逐 token 发送 `StreamEvent::Messages`。

### 5.2 Custom

**Python 机制**：`get_stream_writer()` 从运行时 context 获取 writer，节点/工具调用 `writer({...})` 即可发送。

**Rust 现状**：

| 组件 | 说明 |
|------|------|
| `StreamEvent::Custom(Value)` | 已定义 |
| `Node::run_with_context` | 默认实现调用 `run()` 并忽略 `_ctx` |
| 节点可访问 `ctx.stream_tx` | 是，但需覆盖 `run_with_context` |
| 工具 | 工具通过 `ActNode` 调用，无法直接拿到 `RunContext`，需通过参数或中间层注入 |

**缺口**：

1. 无 `get_stream_writer` 或等价 API，节点需自行从 `RunContext` 解构 `stream_tx` 并检查 `stream_mode.contains(&StreamMode::Custom)`。
2. 工具侧无标准方式获取 writer，需设计注入路径（如通过 `RunnableConfig` 或专用 context）。

---

## 6. 完全缺失的功能（详细）

### 6.1 checkpoints 流式

- **Python**：`stream_mode="checkpoints"` 时，每次创建 checkpoint 发出事件，格式与 `get_state()` 一致，便于调试与时间旅行 UI。
- **Rust**：有 `Checkpointer`、`Checkpoint`，run_loop 在 `Next::End` 和 `Next::Continue` 结尾会 `cp.put(...)`，但未向 stream 发送 checkpoint 事件。

### 6.2 tasks 流式

- **Python**：任务（节点或子图）开始/结束时发出事件，含结果或错误信息。
- **Rust**：无任务抽象，无对应事件。

### 6.3 debug 流式

- **Python**：`stream_mode="debug"` = checkpoints + tasks。
- **Rust**：无。

### 6.4 subgraphs 流式

- **Python**：`subgraphs=True` 时，流式输出包含子图，格式为 `(namespace, data)`，namespace 表示调用路径。
- **Rust**：无子图结构（仅 `checkpoint_ns` 预留命名空间），无 subgraph streaming。

---

## 7. 实现差距根因（技术层面）

1. **run_loop 仅处理 Values/Updates**  
   `run_loop_inner` 在节点执行后只根据 `StreamMode::Values` 和 `StreamMode::Updates` 发送事件，未实现 Messages/Custom 的发送逻辑。Messages 与 Custom 应由**节点内部**根据 `stream_tx` 与 `stream_mode` 主动发送。

2. **节点默认不参与流式**  
   `Node::run_with_context` 默认实现调用 `run()` 并忽略 `RunContext`。ThinkNode、ActNode 等均未覆盖 `run_with_context`，因此无法使用 `ctx.stream_tx`。

3. **LLM 层无流式抽象**  
   `LlmClient` 仅有 `invoke()`，返回完整 `LlmResponse`。要支持 Messages streaming，需新增如 `fn stream(...) -> impl Stream<Item = MessageChunk>` 或回调式 API，并在 ThinkNode 的 `run_with_context` 中连接 `stream_tx`。

4. **工具层无 stream writer 注入**  
   工具通过 `ActNode` 调用，ActNode 不将 `RunContext` 传给工具。要实现 Custom streaming，需设计工具如何获得 writer（例如通过 `config` 或工具参数）。

---

## 8. 任务跟踪（补全 Streaming）

### 8.1 任务表

| ID | 任务 | 优先级 | 状态 |
|----|------|--------|------|
| T1 | 为 `LlmClient` 添加 `stream()` 或 `invoke_stream()` 方法 | 高 | 待办 |
| T2 | `ThinkNode` 实现 `run_with_context`，在 `stream_mode` 含 Messages 时使用流式 LLM 并发送 `StreamEvent::Messages` | 高 | 待办 |
| T3 | `ChatOpenAI` 实现流式 API（如 `async_openai` 的 stream 接口） | 高 | 待办 |
| T4 | 设计 `StreamWriter` 或 `get_stream_writer` 等价 API，供节点使用 | 中 | 待办 |
| T5 | 支持 Custom：节点通过 `ctx.stream_tx` 发送 `StreamEvent::Custom`，文档化用法 | 中 | 待办 |
| T6 | 工具 Custom streaming：设计 writer 注入路径（config 或工具参数） | 中 | 待办 |
| T7 | checkpoints stream 模式：在 `cp.put` 后向 stream 发送 checkpoint 事件 | 低 | 待办 |
| T8 | tasks / debug stream 模式（如需要） | 低 | 待办 |
| T9 | 子图 streaming（如有子图设计） | 低 | 待办 |

### 8.2 依赖关系

```
T1 (LlmClient stream) ──┬──> T2 (ThinkNode run_with_context)
                        └──> T3 (ChatOpenAI stream 实现)

T4 (StreamWriter API) ──> T5 (节点 Custom)
T5 ──> T6 (工具 Custom)
```

---

## 9. 现有测试覆盖

详见 [streaming-test-design.md](./streaming-test-design.md)。当前 `compiled.rs` 中已有：

- `stream_values_emits_states`：Values 单模式
- `stream_updates_emit_node_ids_in_order`：Updates 单模式
- `stream_empty_graph_no_panic_zero_events`：空图
- `stream_single_node_emits_one_values_one_updates`：单节点
- `stream_values_and_updates_both_enabled`：Values + Updates 双模式
- `stream_with_some_config_no_panic`：config 非空
- `stream_mode_includes_messages_custom_collect_no_panic`：stream_mode 含 Messages/Custom 时不 panic，但不会收到这两种事件

---

## 10. 相关文档

- [streaming-test-design.md](./streaming-test-design.md) — Streaming 维度单元测试设计
- [Python LangGraph Streaming](https://docs.langchain.com/oss/python/langgraph/streaming) — 官方 streaming 文档
