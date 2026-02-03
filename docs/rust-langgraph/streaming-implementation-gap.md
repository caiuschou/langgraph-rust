# LangGraph Rust 与 Python LangGraph Streaming 实现差距分析

本文档对比 langgraph-rust 与 Python LangGraph 的 streaming 能力，标识已实现、部分实现与未实现的功能，并提供 API 对比、源码引用与实现建议。

---

## 1. 概述

Python LangGraph 提供完整的 streaming 系统，支持多种 stream 模式及 LLM token 级流式输出。langgraph-rust 目前已实现**图状态级**的 Values 和 Updates streaming，但尚未实现 Messages、Custom 等节点内流式能力。Python 侧的实现机制见 [§2.3 Python LangGraph 实现机制说明](#23-python-langgraph-实现机制说明)。

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
| **messages** | LLM 逐 token 流式输出 `(message_chunk, metadata)` | `ThinkNode.run_with_context()` + `LlmClient.invoke_stream()` | ✅ 已实现 |
| **custom** | 节点/工具通过 `get_stream_writer()` 发送自定义数据 | `StreamWriter` + `ToolStreamWriter` API | ✅ 已完成 |
| **checkpoints** | checkpoint 创建时发出事件 | `StreamMode::Checkpoints`, `CheckpointEvent` | ✅ 已实现 |
| **tasks** | 任务开始/结束事件 | `StreamMode::Tasks`, `TaskStart`, `TaskEnd` | ✅ 已实现 |
| **debug** | 组合 checkpoints + tasks | `StreamMode::Debug` (包含两者) | ✅ 已实现 |

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

### 2.3 Python LangGraph 实现机制说明

以下说明 Python LangGraph 的 streaming 是如何实现的，便于与 Rust 实现对照。

#### 2.3.1 运行时与 Pregel

- **编译产物**：`StateGraph.compile()` 得到的是 **Pregel** 运行时实例（非“图 + run_loop”的简单结构）。Pregel 基于 Google Pregel 的 BSP 模型：每步分为 Plan → Execution → Update。
- **Actors 与 Channels**：节点对应 Pregel 的 **actor**，订阅并写入 **channel**；state 由多个 channel 组成（如 `LastValue`、`Topic`）。
- **stream 入口**：`graph.stream(...)` / `graph.astream(...)` 是 Pregel 的方法，内部根据 `stream_mode` 注册不同的**流式输出通道**，在执行过程中向这些通道推送事件。

#### 2.3.2 Config 与 Context 传递

- **RunnableConfig**：每次 `invoke` / `stream` 调用会构造或传入 `RunnableConfig`，其中包含 `configurable`（如 thread_id、checkpoint_id）、以及 LangGraph 注入的 **callbacks**、**metadata** 等。
- **Context 传播**：
  - **Python ≥ 3.11**：通过 **contextvars** 自动传播。asyncio 任务会继承 context，因此节点内调用的 `model.ainvoke(..., config)` 即使不显式传 config，LangChain 也能通过 context 拿到 callbacks，从而把 token 流传给 LangGraph 的 streaming 层。
  - **Python < 3.11**：asyncio 的 `create_task` 不支持 context 参数，**必须**在节点中显式接收 `config` 并传给 `model.ainvoke(..., config)`，否则 callbacks 不会传到 LLM 调用，messages 流式会失效。
- **get_stream_writer()**：来自 `langgraph.config`，从**当前运行时 context** 中取出本 run 的 stream writer。节点/工具无需显式接收参数，直接调用 `get_stream_writer()` 即可获得 `writer`，再调用 `writer(chunk)` 发送 custom 数据。在 Python < 3.11 的 async 中，context 不传播，因此文档要求把 `writer` 作为节点/工具的参数显式传入。

#### 2.3.3 Messages 模式（LLM token 流）

- **LangChain 集成**：LangChain 的 Chat Model（如 `ChatOpenAI`）实现 `Runnable`，其 `invoke` / `ainvoke` 支持通过 **RunnableConfig 中的 callbacks** 上报事件。
- **Streaming tracer**：LangGraph 在发起 stream 时，会向 config 注入一个 **streaming tracer**（一种 callback）。节点内调用 `model.invoke(...)` 或 `model.ainvoke(..., config)` 时，LangChain 把 token 流通过该 callback 上报。
- **事件形态**：上报的是 `(message_chunk, metadata)`。metadata 由 LangChain 与 LangGraph 共同填充，包含 `langgraph_node`、`langgraph_step`、`langgraph_path`、`tags`（若模型带 tags）、以及 LangSmith 相关字段（`ls_provider`、`ls_model_name` 等）。
- **无需节点显式写流**：节点只需正常写 `model.invoke(...)`，不写任何 stream 相关代码；是否产生 messages 流完全由 `stream_mode="messages"` 和 config/callback 传播决定。

#### 2.3.4 Custom 模式（get_stream_writer）

- **Writer 来源**：`get_stream_writer()` 从当前 context 中读取本 run 绑定的 writer（由 Pregel 在 stream 开始时注入）。
- **调用方式**：节点或工具内执行 `writer = get_stream_writer()`，然后 `writer({"key": "value"})` 即可。writer 接受任意可序列化数据，通常为 dict。
- **与 stream_mode 的关系**：仅当调用 `stream(..., stream_mode="custom")` 或 `stream_mode` 列表中含 `"custom"` 时，该 run 才会注入 writer；否则 `get_stream_writer()` 可能返回无操作或 None。

#### 2.3.5 Values / Updates 模式

- **发送方**：由 **Pregel 运行时**在每步 **Update 阶段**结束后统一发送，而非节点主动发送。
  - **values**：当前所有 channel 的合并状态（或配置的 output channels）。
  - **updates**：本步各 actor 写回的 **增量**（按 node 维度，格式如 `{ node_id: partial_state }`）。
- **与 Rust 的差异**：Rust 的 `Updates` 当前发送的是**合并后的全量 state**；Python 的 updates 是**该节点返回的 partial**，语义更接近“增量”。

#### 2.3.6 Checkpoints / Tasks / Debug 模式

- **Checkpoints**：Pregel 在写入 checkpoint（如调用 checkpointer.put）后，若 `stream_mode` 含 `"checkpoints"`，会向流发送一条 checkpoint 事件，格式与 `get_state()` 返回结构一致，便于时间旅行 UI 或审计。
- **Tasks**：任务（节点或子图）开始/结束时，运行时发送 task 开始/结束事件，可带结果或错误信息。
- **Debug**：`stream_mode="debug"` 等价于同时开启 checkpoints 与 tasks，用于调试。

#### 2.3.7 多模式与 yield 形式

- **多模式**：`stream_mode=["updates", "values", "messages"]` 时，同一流中会交错出现多种事件。
- **(mode, chunk)**：多模式时，Python 的迭代器 yield 的是 `(mode, chunk)` 元组，便于消费者按 mode 分支处理；单模式时可直接 yield chunk。

#### 2.3.8 小结（Python 侧）

| 机制 | 说明 |
|------|------|
| 运行时 | Pregel（actors + channels），stream 由运行时按 stream_mode 驱动 |
| Config/Context | RunnableConfig + contextvars（≥3.11）或显式传 config（<3.11） |
| Messages | LangChain callback 自动上报 token，节点无需写流代码 |
| Custom | get_stream_writer() 从 context 取 writer，节点/工具调用 writer(data) |
| Values/Updates | 运行时在每步 Update 后统一发送，Updates 为节点 partial |
| Checkpoints/Tasks | 运行时在写 checkpoint / 任务起止时发送 |

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
| `StreamEvent::Custom(Value)` | ✅ 已定义 |
| `StreamWriter` | ✅ 已实现，封装 stream sender 和 mode 检查 |
| `RunContext::stream_writer()` | ✅ 从 RunContext 创建 StreamWriter |
| `RunContext::emit_custom()` | ✅ 便捷方法，直接发送 Custom 事件 |
| 节点可访问 `ctx.stream_tx` | ✅ 是，通过 `run_with_context` |
| `ToolStreamWriter` | ✅ 已实现，类型擦除的工具流写入器 |
| `ToolCallContext::stream_writer` | ✅ 已实现，工具可访问 stream writer |
| `ToolCallContext::emit_custom()` | ✅ 已实现，工具便捷发送 Custom 事件 |
| `ActNode::run_with_context` | ✅ 已实现，创建并传递 ToolStreamWriter 给工具 |

**节点使用示例**：

```rust
async fn run_with_context(&self, state: S, ctx: &RunContext<S>) -> Result<(S, Next), AgentError> {
    // 方法 1: 使用 StreamWriter
    let writer = ctx.stream_writer();
    writer.emit_custom(serde_json::json!({"progress": 50})).await;
    
    // 方法 2: 使用便捷方法
    ctx.emit_custom(serde_json::json!({"status": "done"})).await;
    
    Ok((state, Next::Continue))
}
```

**工具使用示例**：

```rust
async fn call(&self, args: Value, ctx: Option<&ToolCallContext>) -> Result<ToolCallContent, ToolSourceError> {
    if let Some(ctx) = ctx {
        // 发送进度更新
        ctx.emit_custom(serde_json::json!({"status": "starting"}));
        
        // 或使用 stream_writer
        if let Some(writer) = &ctx.stream_writer {
            writer.emit_custom(serde_json::json!({"progress": 50}));
        }
    }
    
    // 执行工具逻辑...
    Ok(ToolCallContent { text: "Done".to_string() })
}
```

**全部已完成** ✅

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

（Python 侧对应机制见 [§2.3 Python LangGraph 实现机制说明](#23-python-langgraph-实现机制说明)。）

1. **run_loop 仅处理 Values/Updates**  
   `run_loop_inner` 在节点执行后只根据 `StreamMode::Values` 和 `StreamMode::Updates` 发送事件，未实现 Messages/Custom 的发送逻辑。Messages 与 Custom 应由**节点内部**根据 `stream_tx` 与 `stream_mode` 主动发送。

2. **节点默认不参与流式**  
   `Node::run_with_context` 默认实现调用 `run()` 并忽略 `RunContext`。ThinkNode、ActNode 等均未覆盖 `run_with_context`，因此无法使用 `ctx.stream_tx`。

3. **LLM 层无流式抽象**  
   `LlmClient` 仅有 `invoke()`，返回完整 `LlmResponse`。要支持 Messages streaming，需新增如 `fn stream(...) -> impl Stream<Item = MessageChunk>` 或回调式 API，并在 ThinkNode 的 `run_with_context` 中连接 `stream_tx`。

4. ~~**工具层无 stream writer 注入**~~  
   ~~工具通过 `ActNode` 调用，ActNode 不将 `RunContext` 传给工具。~~ **已解决**：`ActNode` 现在实现 `run_with_context`，创建 `ToolStreamWriter` 并通过 `ToolCallContext::stream_writer` 传递给工具。工具可通过 `ctx.emit_custom()` 或 `ctx.stream_writer.emit_custom()` 发送 Custom 事件。

---

## 8. 任务跟踪（补全 Streaming）

### 8.1 任务表

| ID | 任务 | 优先级 | 状态 |
|----|------|--------|------|
| T1 | 为 `LlmClient` 添加 `stream()` 或 `invoke_stream()` 方法 | 高 | ✅ 已完成 |
| T2 | `ThinkNode` 实现 `run_with_context`，在 `stream_mode` 含 Messages 时使用流式 LLM 并发送 `StreamEvent::Messages` | 高 | ✅ 已完成 |
| T3 | `ChatOpenAI` 实现流式 API（如 `async_openai` 的 stream 接口） | 高 | ✅ 已完成 |
| T4 | 设计 `StreamWriter` 或 `get_stream_writer` 等价 API，供节点使用 | 中 | ✅ 已完成 |
| T5 | 支持 Custom：节点通过 `ctx.stream_tx` 发送 `StreamEvent::Custom`，文档化用法 | 中 | ✅ 已完成 |
| T6 | 工具 Custom streaming：设计 writer 注入路径（config 或工具参数） | 中 | ✅ 已完成 |
| T7 | checkpoints stream 模式：在 `cp.put` 后向 stream 发送 checkpoint 事件 | 低 | ✅ 已完成 |
| T8 | tasks / debug stream 模式（如需要） | 低 | ✅ 已完成 |
| T9 | 子图 streaming（如有子图设计） | 低 | 待办 |

### 8.2 依赖关系

```
T1 (LlmClient stream) ──┬──> T2 (ThinkNode run_with_context)
                        └──> T3 (ChatOpenAI stream 实现)

T4 (StreamWriter API) ──> T5 (节点 Custom)
T5 ──> T6 (工具 Custom)
```

---

## 9. 补充功能详细说明

以下对每个待补充功能给出详细规格：功能目标、API 设计、数据流、涉及模块、实现要点与测试建议。

---

### 9.1 Messages streaming（T1、T2、T3）

#### 9.1.1 功能目标

- 在 `stream_mode` 包含 `Messages` 时，LLM 调用产生的 token 逐块流式输出到 `StreamEvent::Messages`。
- 消费者可实时展示打字机效果，无需等待完整回复。

#### 9.1.2 预期 API

**LlmClient 扩展**（`langgraph/src/llm/mod.rs`）：

```rust
// 新增方法签名（二选一或并存）
async fn invoke_stream(
    &self,
    messages: &[Message],
    stream_tx: Option<&mpsc::Sender<StreamEvent<S>>>,  // 或通过回调
) -> Result<LlmResponse, AgentError>;
// 或
fn stream(
    &self,
    messages: &[Message],
) -> impl Stream<Item = Result<MessageChunk, AgentError>> + Send;
```

- 若采用回调式：在每收到一个 token 时调用 `stream_tx.send(StreamEvent::Messages { chunk, metadata })`。
- 若采用 Stream 返回：ThinkNode 在 `run_with_context` 中消费该 stream 并转发到 `stream_tx`。

**StreamMetadata 扩展**（可选，对齐 Python）：

- 当前 `StreamMetadata` 仅有 `langgraph_node: String`。
- Python 的 metadata 还含 `tags`（用于过滤特定 LLM 调用）、`langgraph_node` 等。
- 可扩展为 `HashMap<String, Value>` 或新增可选字段，便于后续过滤。

#### 9.1.3 数据流

```
graph.stream(..., [Messages])
    → run_loop 调用 ThinkNode.run_with_context(s, ctx)
    → ThinkNode 检查 ctx.stream_mode.contains(Messages)
    → 若包含，调用 llm.invoke_stream(msgs, Some(ctx.stream_tx))
    → ChatOpenAI 使用 async_openai 的 create_stream()
    → 每收到 delta.content，构造 MessageChunk + StreamMetadata
    → stream_tx.send(StreamEvent::Messages { chunk, metadata })
    → 消费者 stream.next() 收到事件
```

#### 9.1.4 涉及模块与修改点

| 模块 | 修改内容 |
|------|----------|
| `llm/mod.rs` | 为 `LlmClient` 添加 `invoke_stream` 或 `stream`；`LlmResponse` 仍需在流结束后返回（含完整 content + tool_calls） |
| `llm/openai.rs` | 使用 `client.chat().create_stream(request)`，遍历 stream 累积 content 并逐 chunk 发送；tool_calls 需在流结束时从最后一轮解析 |
| `react/think_node.rs` | 实现 `run_with_context`：若 `stream_mode.contains(Messages)` 且 `stream_tx` 为 Some，则调用 `invoke_stream`，否则调用 `invoke` |
| `stream/mod.rs` | 视需扩展 `StreamMetadata`（如 `tags`） |

#### 9.1.5 实现要点

- **tool_calls 处理**：OpenAI 流式 API 中，tool_calls 在流式 chunks 中分段到达，需在流结束时合并解析。可参考 async_openai 的 stream 类型（如 `ChatCompletionResponseStream`）。
- **背压**：`stream_tx.send` 为 `async`，消费者慢时 sender 会 await，天然背压。
- **MockLlm**：需实现 `invoke_stream`，可逐字符 yield 或简单一次性发送整句，用于测试。
- **错误**：流式过程中若 LLM 失败，应通过 `StreamEvent` 传递错误或结束 stream；需定义错误事件形态（或沿用 `Custom` 携带错误信息）。

#### 9.1.6 测试建议

- 使用 `MockLlm` 的 `invoke_stream` 逐字符发送，断言 `stream_mode=Messages` 时收到多个 `StreamEvent::Messages`，且拼接 content 等于最终 `LlmResponse.content`。
- 断言 `metadata.langgraph_node == "think"`。
- 断言 `stream_mode` 不含 Messages 时，ThinkNode 走 `invoke` 路径，不发送 Messages 事件。

---

### 9.2 Custom streaming — 节点（T4、T5）

#### 9.2.1 功能目标

- 节点在执行过程中可向 stream 发送任意 JSON 数据（进度、中间结果等）。
- 仅当 `stream_mode` 含 `Custom` 时发送，且需有简便 API 供节点使用。

#### 9.2.2 预期 API

**StreamWriter 或辅助函数**：

```rust
// 方案 A：从 RunContext 解构
fn stream_custom(ctx: &RunContext<S>, value: Value) {
    if let (Some(tx), true) = (&ctx.stream_tx, ctx.stream_mode.contains(&StreamMode::Custom)) {
        let _ = tx.try_send(StreamEvent::Custom(value));  // 或 .send().await
    }
}

// 方案 B：StreamWriter 类型（可克隆、可传入节点）
pub struct StreamWriter<S> {
    tx: Option<mpsc::Sender<StreamEvent<S>>>,
    mode: HashSet<StreamMode>,
}
impl<S> StreamWriter<S> {
    pub fn write(&self, value: Value) { ... }
}
// RunContext 增加 stream_writer: Option<StreamWriter<S>>
```

- 节点通过 `ctx.stream_writer.as_ref().map(|w| w.write(json!({...})))` 或辅助函数发送。

#### 9.2.3 数据流

```
graph.stream(..., [Custom])
    → run_loop 调用 Node.run_with_context(s, ctx)
    → 节点内部调用 ctx.stream_writer.write({ "progress": 50 }) 等
    → StreamWriter 检查 mode.contains(Custom)，send(StreamEvent::Custom(value))
    → 消费者收到 StreamEvent::Custom
```

#### 9.2.4 涉及模块与修改点

| 模块 | 修改内容 |
|------|----------|
| `graph/run_context.rs` | 增加 `stream_writer: Option<StreamWriter<S>>` 或保持仅 `stream_tx` + `stream_mode`，通过辅助函数封装 |
| `stream/mod.rs` | 新增 `StreamWriter` 或 `emit_custom(ctx, value)` 辅助函数 |
| 业务节点 | 覆盖 `run_with_context`，在需要时调用 writer |

#### 9.2.5 实现要点

- **同步 vs 异步**：`stream_tx.send` 为 async，若在同步上下文中调用需 `try_send` 或 spawn 小任务；节点 `run_with_context` 为 async，可直接 `await tx.send(...)`。
- **Python 的 get_stream_writer**：从 context var 获取，Rust 无等价物，只能通过 `RunContext` 显式传入。
- **文档**：在 README 或模块文档中给出节点发送 Custom 的示例。

#### 9.2.6 测试建议

- 自定义节点实现 `run_with_context`，发送若干 `StreamEvent::Custom`。
- 断言 `stream_mode=Custom` 时收到对应事件；`stream_mode` 不含 Custom 时不发送。
- 断言多模式 `[Values, Custom]` 时，Values 与 Custom 事件均存在且顺序符合预期。

---

### 9.3 Custom streaming — 工具（T6）

#### 9.3.1 功能目标

- 工具在执行（如长时间查询、多步操作）过程中，可向 stream 发送进度或中间结果。
- Python 中工具通过 `get_stream_writer()` 获取 writer，在工具内部调用。

#### 9.3.2 预期 API

**注入路径**：工具当前通过 `Tool::call(args, ctx)` 调用，`ToolCallContext` 仅有 `recent_messages`。需扩展：

```rust
// ToolCallContext 扩展
pub struct ToolCallContext {
    pub recent_messages: Vec<Message>,
    pub stream_writer: Option<StreamWriter<S>>,  // 或 Arc<dyn Fn(Value)>
}

// ActNode 在调用 call_tool 前，从 RunContext 构建 ToolCallContext
// 需要 ActNode 能访问 RunContext，即 ActNode.run_with_context 覆盖
```

- 另一方案：将 `stream_writer` 放在 `RunnableConfig` 的扩展字段中，工具通过 `config` 获取（若工具签名支持传入 config）。

#### 9.3.3 数据流

```
graph.stream(..., [Custom])
    → run_loop 调用 ActNode.run_with_context(s, ctx)
    → ActNode 构建 ToolCallContext { stream_writer: ctx.stream_writer.clone() }
    → tools.call_tool_with_context(name, args, &tool_ctx)
    → 工具内部 tool_ctx.stream_writer.as_ref().map(|w| w.write(...))
    → 发送 StreamEvent::Custom
```

#### 9.3.4 涉及模块与修改点

| 模块 | 修改内容 |
|------|----------|
| `tool_source/context.rs` 或等价 | `ToolCallContext` 增加 `stream_writer: Option<StreamWriter<ReActState>>>` |
| `react/act_node.rs` | 实现 `run_with_context`，从 `ctx` 构建含 `stream_writer` 的 `ToolCallContext`，再调用 `call_tool_with_context` |
| `tools/trait.rs` | `Tool::call` 的 `ctx` 类型已为 `Option<&ToolCallContext>`，工具实现中可检查 `ctx.stream_writer` |

#### 9.3.5 实现要点

- **类型耦合**：`ToolCallContext` 若携带 `StreamWriter<ReActState>`，则与 `ReActState` 耦合；工具 trait 目前与 state 无关，可考虑 `StreamWriter` 泛型为 `()` 或使用 `serde_json::Value` 的通用 writer，避免 trait 污染。
- **可选**：工具 trait 可增加 `call_with_stream_writer` 或通过扩展 context 实现，保持向后兼容。

#### 9.3.6 测试建议

- 编写一工具，在 `call` 中检查 `ctx.stream_writer`，若有则发送 2～3 个 Custom 事件。
- 使用 ReAct 图调用该工具，`stream_mode=Custom`，断言收到工具发出的 Custom 事件。

---

### 9.4 checkpoints stream 模式（T7）

#### 9.4.1 功能目标

- 每次 run_loop 写入 checkpoint 后，向 stream 发送 checkpoint 事件。
- 事件格式与 `get_state()` 类似，便于调试、时间旅行 UI 或审计。

#### 9.4.2 预期 API

**新增 StreamEvent 变体或复用 Custom**：

- 方案 A：新增 `StreamEvent::Checkpoint { checkpoint_id, state, metadata }` 或 `Checkpoint(Checkpoint<S>)`。
- 方案 B：使用 `StreamEvent::Custom(serde_json::to_value(checkpoint))`，约定 JSON 结构。
- 建议方案 A，类型明确，消费者可直接 match。

**StreamMode 扩展**：

```rust
pub enum StreamMode {
    Values,
    Updates,
    Messages,
    Custom,
    Checkpoints,  // 新增
}
```

#### 9.4.3 数据流

```
graph.stream(..., [Checkpoints]) 且图有 checkpointer
    → run_loop 在 Next::End 或 Next::Continue 结尾调用 cp.put(cfg, checkpoint)
    → put 成功后，若 stream_mode.contains(Checkpoints)，send(StreamEvent::Checkpoint(...))
    → 消费者收到 checkpoint 事件
```

#### 9.4.4 涉及模块与修改点

| 模块 | 修改内容 |
|------|----------|
| `stream/mod.rs` | 增加 `StreamMode::Checkpoints`，`StreamEvent::Checkpoint` 或 `Checkpoint(Checkpoint<S>)` |
| `graph/compiled.rs` | 在 `cp.put` 之后、`return`/`continue` 之前，检查 `run_ctx.stream_mode.contains(Checkpoints)`，若包含则 send checkpoint 事件 |

#### 9.4.5 实现要点

- checkpoint 事件应在 `put` 成功后发送，确保与持久化一致。
- 无 checkpointer 时，不发送 checkpoint 事件。
- `Checkpoint<S>` 需可序列化或可转换为 stream 友好格式；若 `S` 过大，可只发送 `checkpoint_id` + 元数据，消费者再按需 `get_state`。

#### 9.4.6 测试建议

- 使用带 checkpointer 的图，`stream_mode=[Checkpoints]`，运行多节点，断言收到数量与 checkpoint 写入次数一致。
- 无 checkpointer 时，断言不收到 Checkpoint 事件。

---

### 9.5 tasks / debug stream 模式（T8）

#### 9.5.1 功能目标

- **tasks**：任务（节点执行）开始/结束时发出事件，含 node_id、结果或错误。
- **debug**：checkpoints + tasks 的组合，用于调试。

#### 9.5.2 预期 API

**StreamEvent 扩展**：

```rust
pub enum StreamEvent<S> {
    Values(S),
    Updates { node_id, state },
    Messages { chunk, metadata },
    Custom(Value),
    Checkpoint(Checkpoint<S>),           // 若 T7 采用新变体
    TaskStart { node_id: String },       // 新增
    TaskEnd { node_id: String, result: Result<(), String> },  // 新增
}
```

**StreamMode**：`Tasks`、`Debug`（Debug = Checkpoints | Tasks，或单独模式在 run_loop 中同时发送两者）。

#### 9.5.3 数据流

```
run_loop 执行节点前：send(TaskStart { node_id })
run_loop 执行节点后：send(TaskEnd { node_id, result: Ok(()) }) 或 Err(...)
```

- 可与现有 middleware 结合：middleware 的 `around_run` 在调用前后发送 TaskStart/TaskEnd。

#### 9.5.4 涉及模块与修改点

| 模块 | 修改内容 |
|------|----------|
| `stream/mod.rs` | `StreamMode::Tasks`、`StreamMode::Debug`；`StreamEvent::TaskStart`、`TaskEnd` |
| `graph/compiled.rs` | 在节点执行前后根据 `stream_mode` 发送 TaskStart/TaskEnd；或由 middleware 负责 |
| `graph/node_middleware.rs` | 若有通用 logging middleware，可在此发出 TaskStart/TaskEnd |

#### 9.5.5 实现要点

- 错误传递：节点返回 `Err` 时，run_loop 会短路，需在 `Err` 分支发送 `TaskEnd { result: Err(...) }` 再 return。
- Debug 模式：可定义为 `stream_mode.contains(Debug)` 时同时发送 Checkpoints 与 Tasks，避免重复定义。

#### 9.5.6 测试建议

- `stream_mode=Tasks` 时，断言每个节点有 TaskStart、TaskEnd 且顺序正确。
- 模拟节点失败，断言 TaskEnd 含 Err。

---

### 9.6 子图 streaming（T9）

#### 9.6.1 功能目标

- 当图包含子图（nested graph）时，流式输出包含子图内部节点的 events。
- 事件带有 namespace，标识调用路径（如 `("parent_node:task_id", "child_node")`）。

#### 9.6.2 前置条件

- 当前 Rust 实现无子图结构；仅 `checkpoint_ns` 预留命名空间。
- 需先有**子图设计**：如某节点可持有 `CompiledStateGraph<S>` 作为子图，run_loop 递归执行时传入 namespace。

#### 9.6.3 预期 API（需随子图设计确定）

- `StreamEvent` 扩展 `namespace: Vec<String>` 或等效字段，表示事件来源路径。
- `stream(subgraphs=true)` 或通过 `stream_mode` 控制是否包含子图事件。
- 父图与子图共享同一 `stream_tx`，子图 run_loop 发送事件时附加 namespace。

#### 9.6.4 涉及模块与修改点

- 依赖子图架构设计，涉及 `StateGraph`、`CompiledStateGraph`、节点类型等，范围较大。
- 建议作为后续迭代，待子图设计落地后再细化。

#### 9.6.5 实现要点

- namespace 的传递：子图执行时，RunContext 的 `stream_tx` 可复用，但需注入当前 namespace，以便发送时附带。
- Python 的 `(namespace, data)` 元组：Rust 可在 `StreamEvent` 中增加 `namespace` 字段，或使用 `StreamEvent::Subgraph { namespace, inner: Box<StreamEvent> }` 等。

---

## 10. 新增 API 使用说明

以下说明各新增 streaming 功能实现后的**使用方式**，面向图构建者、节点/工具实现者与 stream 消费者。

---

### 10.1 stream() 消费 — 多模式联合使用

```rust
use std::collections::HashSet;
use tokio_stream::StreamExt;
use langgraph::{StreamEvent, StreamMode};

// 同时订阅 Values、Updates、Messages、Custom
let modes = HashSet::from_iter([
    StreamMode::Values,
    StreamMode::Updates,
    StreamMode::Messages,
    StreamMode::Custom,
]);
let mut stream = compiled.stream(initial_state, config, modes);

while let Some(event) = stream.next().await {
    match event {
        StreamEvent::Values(s) => {
            // 全量状态，可用于重建 UI
        }
        StreamEvent::Updates { node_id, state } => {
            // 节点更新，可用于增量 UI 或日志
        }
        StreamEvent::Messages { chunk, metadata } => {
            // LLM token，实现打字机效果
            print!("{}", chunk.content);
        }
        StreamEvent::Custom(v) => {
            // 自定义数据，如进度、中间结果
            if let Some(p) = v.get("progress") {
                eprintln!("Progress: {}%", p);
            }
        }
        StreamEvent::Checkpoint(cp) => {  /* T7 实现后 */ }
        StreamEvent::TaskStart { node_id } => {  /* T8 实现后 */ }
        StreamEvent::TaskEnd { node_id, result } => {  /* T8 实现后 */ }
        _ => {}
    }
}
```

---

### 10.2 Messages streaming — 消费者用法

```rust
// 仅订阅 Messages，实时显示 LLM 输出（打字机效果）
let modes = HashSet::from_iter([StreamMode::Messages]);
let mut stream = compiled.stream(state, config, modes);

while let Some(event) = stream.next().await {
    if let StreamEvent::Messages { chunk, metadata } = event {
        if !chunk.content.is_empty() {
            print!("{}", chunk.content);
            std::io::Write::flush(&mut std::io::stdout()).ok();
        }
        // 可按 metadata.langgraph_node 过滤特定节点
        // 例如仅显示 think 节点的输出
        if metadata.langgraph_node == "think" {
            // ...
        }
    }
}
```

---

### 10.3 Custom streaming — 节点实现者用法

节点需覆盖 `run_with_context`，通过 `StreamWriter` 或辅助函数发送自定义数据：

```rust
use async_trait::async_trait;
use langgraph::{Node, Next, RunContext, AgentError};
use langgraph::stream::emit_custom;  // 或 StreamWriter
use serde_json::json;

struct ProgressNode;

#[async_trait]
impl Node<MyState> for ProgressNode {
    fn id(&self) -> &str { "progress" }

    async fn run_with_context(
        &self,
        state: MyState,
        ctx: &RunContext<MyState>,
    ) -> Result<(MyState, Next), AgentError> {
        // 发送进度开始（emit_custom 为 async，需 await）
        emit_custom(ctx, json!({ "phase": "start", "step": 0 })).await;

        for i in 1..=10 {
            // 模拟耗时操作
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            emit_custom(ctx, json!({ "phase": "progress", "step": i, "percent": i * 10 })).await;
        }

        emit_custom(ctx, json!({ "phase": "done" })).await;
        Ok((state, Next::Continue))
    }
}
```

**StreamWriter 用法**（若采用方案 B）：

```rust
async fn run_with_context(&self, state: MyState, ctx: &RunContext<MyState>) -> ... {
    if let Some(writer) = &ctx.stream_writer {
        writer.write(json!({ "custom_key": "value" })).await;
    }
    // ...
}
```

---

### 10.4 Custom streaming — 工具实现者用法

工具在 `call` 或 `call_with_context` 中通过 `ToolCallContext.stream_writer` 发送：

```rust
use async_trait::async_trait;
use langgraph::tools::Tool;
use langgraph::tool_source::{ToolCallContent, ToolCallContext, ToolSpec};
use serde_json::{Value, json};

struct QueryTool;

#[async_trait]
impl Tool for QueryTool {
    fn name(&self) -> &str { "query_db" }
    fn spec(&self) -> ToolSpec { ... }

    async fn call(
        &self,
        args: Value,
        ctx: Option<&ToolCallContext>,
    ) -> Result<ToolCallContent, ToolSourceError> {
        if let Some(ctx) = ctx {
            if let Some(writer) = &ctx.stream_writer {
                writer.write(json!({ "status": "querying", "progress": 0 })).await;
            }
        }

        let results = fetch_from_db(&args).await;

        if let Some(ctx) = ctx {
            if let Some(writer) = &ctx.stream_writer {
                writer.write(json!({ "status": "done", "count": results.len() })).await;
            }
        }

        Ok(ToolCallContent { text: format!("{:?}", results) })
    }
}
```

---

### 10.5 LlmClient::invoke_stream — 库扩展者用法

自定义 LLM 客户端实现 `invoke_stream`，用于非 OpenAI 模型或自建 API：

```rust
use async_trait::async_trait;
use langgraph::llm::{LlmClient, LlmResponse};
use langgraph::stream::{MessageChunk, StreamMetadata};

struct MyCustomLlm;

#[async_trait]
impl LlmClient for MyCustomLlm {
    async fn invoke(&self, messages: &[Message]) -> Result<LlmResponse, AgentError> {
        self.invoke_stream(messages, None).await
    }

    async fn invoke_stream(
        &self,
        messages: &[Message],
        stream_tx: Option<&mpsc::Sender<StreamEvent<ReActState>>>,
    ) -> Result<LlmResponse, AgentError> {
        let mut full_content = String::new();
        let mut stream = my_api_client.stream_complete(messages).await?;

        while let Some(token) = stream.next().await {
            full_content.push_str(&token);
            if let Some(tx) = stream_tx {
                let _ = tx.send(StreamEvent::Messages {
                    chunk: MessageChunk { content: token },
                    metadata: StreamMetadata { langgraph_node: "think".into() },
                }).await;
            }
        }

        Ok(LlmResponse { content: full_content, tool_calls: vec![] })
    }
}
```

---

### 10.6 checkpoints stream 模式 — 消费者用法（T7 实现后）

```rust
let modes = HashSet::from_iter([StreamMode::Checkpoints]);
let mut stream = compiled.stream(state, Some(config_with_thread_id), modes);

while let Some(event) = stream.next().await {
    if let StreamEvent::Checkpoint(cp) = event {
        println!("Checkpoint: id={}, step={}", cp.id, cp.metadata.step);
        // 可用于时间旅行 UI：展示历史快照列表
    }
}
```

---

### 10.7 tasks / debug stream 模式 — 消费者用法（T8 实现后）

```rust
// 调试模式：获取任务起止与 checkpoint
let modes = HashSet::from_iter([StreamMode::Debug]);
let mut stream = compiled.stream(state, config, modes);

while let Some(event) = stream.next().await {
    match event {
        StreamEvent::TaskStart { node_id } => {
            eprintln!("[start] {}", node_id);
        }
        StreamEvent::TaskEnd { node_id, result } => {
            match result {
                Ok(()) => eprintln!("[end] {} ok", node_id),
                Err(e) => eprintln!("[end] {} err: {}", node_id, e),
            }
        }
        StreamEvent::Checkpoint(cp) => {
            eprintln!("[checkpoint] {} at step {}", cp.id, cp.metadata.step);
        }
        _ => {}
    }
}
```

---

### 10.8 说明

- 上述示例中的 `StreamEvent::Checkpoint`、`TaskStart`、`TaskEnd` 在 T7/T8 实现前不存在，使用时需按实际 `StreamEvent` 定义调整 `match`。
- `emit_custom`、`StreamWriter`、`ToolCallContext::stream_writer` 为预期 API，具体签名以实现为准。

### 10.9 API 速查表

| API / 类型 | 用途 | 使用者 |
|------------|------|--------|
| `compiled.stream(state, config, modes)` | 启动流式执行 | 图消费者 |
| `StreamEvent::Messages { chunk, metadata }` | 接收 LLM token | 消费者 |
| `StreamEvent::Custom(Value)` | 接收自定义 JSON | 消费者 |
| `emit_custom(ctx, value)` / `StreamWriter::write` | 发送自定义数据 | 节点、工具 |
| `LlmClient::invoke_stream` | 流式 LLM 调用 | LlmClient 实现者 |
| `Node::run_with_context` | 获取 RunContext 并发送 Custom | 节点实现者 |
| `ToolCallContext::stream_writer` | 工具内发送 Custom | 工具实现者 |

---

## 11. 现有测试覆盖

详见 [streaming-test-design.md](./streaming-test-design.md)。当前 `compiled.rs` 中已有：

- `stream_values_emits_states`：Values 单模式
- `stream_updates_emit_node_ids_in_order`：Updates 单模式
- `stream_empty_graph_no_panic_zero_events`：空图
- `stream_single_node_emits_one_values_one_updates`：单节点
- `stream_values_and_updates_both_enabled`：Values + Updates 双模式
- `stream_with_some_config_no_panic`：config 非空
- `stream_mode_includes_messages_custom_collect_no_panic`：stream_mode 含 Messages/Custom 时不 panic，但不会收到这两种事件

---

## 12. 相关文档

- [streaming-test-design.md](./streaming-test-design.md) — Streaming 维度单元测试设计
- [Python LangGraph Streaming](https://docs.langchain.com/oss/python/langgraph/streaming) — 官方 streaming 文档
