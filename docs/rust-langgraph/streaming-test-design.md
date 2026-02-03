# Streaming 维度单元测试设计

以 **streaming** 为维度抽象单元测试：覆盖流式类型、`stream()` API、RunContext、事件来源与边界情况。测试风格为 BDD，用例即文档。

## 0. 当前 stream 相关测试覆盖率

以下为基于代码与用例对应关系的**手工覆盖率分析**，以及 **cargo llvm-cov** 实测数值。

### 0.1 stream 相关源码范围

| 文件 | 行/区域 | 说明 |
|------|---------|------|
| `langgraph/src/stream/mod.rs` | 全文 | `StreamMode`、`StreamMetadata`、`MessageChunk`、`StreamEvent<S>` |
| `langgraph/src/graph/run_context.rs` | 全文 | `RunContext`（config、stream_tx、stream_mode） |
| `langgraph/src/graph/compiled.rs` | 86–99 | `run_loop_inner` 内根据 run_ctx 发送 Values/Updates |
| `langgraph/src/graph/compiled.rs` | 164–191 | `stream()`：channel、spawn、RunContext、ReceiverStream |

### 0.2 现有测试与覆盖情况

| 测试位置 | 测试名称 | 覆盖的 stream 行为 |
|----------|----------|--------------------|
| `src/stream/mod.rs` | `stream_event_variants_hold_data` | `StreamEvent` 四变体、`MessageChunk`/`StreamMetadata` 构造与解构 |
| `src/stream/mod.rs` | `stream_mode_four_variants_hashset_equality` | `StreamMode` 四模式 Eq/Hash、放入 HashSet 去重（S-T1） |
| `src/graph/compiled.rs` | `stream_values_emits_states` | `stream()` 入口、`StreamMode::Values`、run_loop 发送 Values、最后一条为最终状态（S-A1） |
| `src/graph/compiled.rs` | `stream_updates_emit_node_ids_in_order` | `stream()` 入口、`StreamMode::Updates`、run_loop 发送 Updates、node_id 顺序（S-A2） |
| `src/graph/compiled.rs` | `stream_empty_graph_no_panic_zero_events` | 空图 `stream()` 不 panic、0 条事件（S-E1） |
| `src/graph/compiled.rs` | `stream_single_node_emits_one_values_one_updates` | 单节点图、Values+Updates 各一条（S-E2） |
| `src/graph/compiled.rs` | `stream_values_and_updates_both_enabled` | Values+Updates 同时开启、顺序 Values→Updates 每节点（S-A3） |
| `src/graph/compiled.rs` | `stream_with_some_config_no_panic` | `stream(_, Some(config), _)` 正常 collect、不 panic（S-A5） |
| `src/graph/compiled.rs` | `stream_mode_includes_messages_custom_collect_no_panic` | stream_mode 含 Messages/Custom 时 collect 不 panic、仅收到 Values/Updates（S-E3） |

**独立 stream 测试目录**：暂无（`tests/stream/` 未建）；上述用例均内联于 `src/stream/mod.rs` 与 `src/graph/compiled.rs`。

### 0.3 按模块的覆盖结论

| 模块/代码路径 | 已覆盖 | 未覆盖 |
|---------------|--------|--------|
| **stream/mod.rs** | `StreamEvent` 四变体、`MessageChunk`、`StreamMetadata`；`StreamMode` 四模式 Eq/Hash/HashSet（`stream_mode_four_variants_hashset_equality`） | — |
| **run_context.rs** | 通过 `compiled::stream()` 间接使用 | 无独立单元测试（可保持现状） |
| **compiled.rs — stream()** | 调用入口；Values/Updates 单模式；空图 0 事件；单节点图；Values+Updates 双模式；`config: Some(_)`；stream_mode 含 Messages/Custom 时 collect 不 panic | — |
| **compiled.rs — run_loop 发送** | Values 分支、Updates 分支、同一轮双分支（Values+Updates）均已测 | stream_mode 为空或仅 Messages/Custom 的“不发送”路径可后续补（低优先级） |

### 0.4 缺口汇总（建议补测）

1. **类型层**：已补——StreamMode 四模式 HashSet/相等性（`stream_mode_four_variants_hashset_equality`，对应 S-T1）。
2. **API/边界**：已补——空图 stream（S-E1）、单节点图（S-E2）、Values+Updates 同时开启（S-A3）、`config: Some(cfg)`（S-A5）、stream_mode 含 Messages/Custom（S-E3）均已在内联测试中实现。
3. **可选**：消费者提前 drop（S-E4）可文档化或后续补；stream_mode 为空或仅 Messages/Custom 的“不发送”路径可视需要补测。
4. **RunContext**：仍通过 stream 测试间接覆盖，无需单独文件除非后续要测“节点内使用 ctx.stream_tx”的集成。

### 0.5 覆盖率小结（定性）

- **stream 类型**：高——四类事件与元数据均有构造/解构测试；StreamMode 四模式 Eq/Hash/HashSet 已单独测。
- **stream() 与 run_loop 发送**：高——Values/Updates 单模式与顺序、空图、单节点、双模式、`config: Some`、stream_mode 含 Messages/Custom 均已覆盖。
- **RunContext**：间接覆盖，无独立用例（可接受）。

### 0.6 覆盖率数值（cargo llvm-cov）

在项目根执行：`cargo llvm-cov test -p langgraph --no-fail-fast` 后，`cargo llvm-cov report -p langgraph` 得到如下 **stream 相关** 文件数值（最近一次实测）：

| 文件 | Regions 覆盖率 | Lines 覆盖率 | 说明 |
|------|----------------|--------------|------|
| **stream/mod.rs** | **94.12%**（68 区域，未覆盖 4） | **91.49%**（47 行，未覆盖 4 行） | 类型与内联测试；未覆盖多为派生/边界 |
| **graph/compiled.rs** | **91.78%**（596 区域，未覆盖 49） | **92.92%**（438 行，未覆盖 31 行） | 含 invoke/run_loop/stream/checkpoint；已补 checkpointer、Next::End、Next::Node 用例 |

说明：`graph/run_context.rs` 在 report 中未单独列出（无独立可执行区域或归入 compiled），其使用已通过 `compiled::stream()` 的测试间接覆盖。

**复现命令**：
```bash
cargo llvm-cov test -p langgraph --no-fail-fast
cargo llvm-cov report -p langgraph
```

---

## 1. 维度与范围

| 维度 | 说明 | 涉及类型/API |
|------|------|----------------|
| **Streaming** | 图执行过程中的流式事件：模式、事件类型、发送与消费 | `StreamMode`, `StreamEvent<S>`, `MessageChunk`, `StreamMetadata`, `CompiledStateGraph::stream()`, `RunContext` |

**范围**：

- **包含**：`stream` 模块类型、`stream()` 行为、RunContext 与 stream_tx/stream_mode、多模式组合、空图/单节点/顺序。
- **不包含**：非流式的 `invoke()` 行为（由 `state_graph/invoke` 等覆盖）、具体业务节点（如 ThinkNode 发 Messages）的集成测试可后续按需加。

## 2. 测试分层与文件布局

```
langgraph/
  src/stream/mod.rs          # 已有：StreamEvent/StreamMode 等类型及内联 tests
  src/graph/compiled.rs      # 已有：stream() 及 run_loop 的 stream 相关 tests
  tests/
    stream/                  # 新建：以 streaming 为维度的独立测试目录
      mod.rs
      types.rs               # StreamEvent / StreamMode / MessageChunk / StreamMetadata
      api.rs                 # stream() 行为：返回值、顺序、多模式
      run_context.rs         # RunContext 与 stream_tx/stream_mode 的配合（若可测）
      edge_cases.rs          # 空图、单节点、consumer 提前 drop 等
```

- **types**：纯类型与构造、枚举变体携带数据（与 `stream/mod.rs` 内联测试对齐或迁移到 `tests/stream/types.rs` 统一维护）。
- **api**：`stream()` 的 Given/When/Then 场景（模式组合、事件顺序、最后一条为 Values 等）。
- **run_context**：RunContext 中 `stream_tx: None` vs `Some`、`stream_mode` 空/部分/全量的行为（通过 `invoke` 不发射事件、`stream` 发射事件间接验证，或通过可构造的 RunContext 测试）。
- **edge_cases**：空图、单节点、多模式组合、channel 背压/消费者 drop 等。

## 3. 用例设计表（Streaming 维度）

### 3.1 类型层（Stream types）

| ID | Scenario（场景） | Given | When | Then | 文件 |
|----|------------------|--------|------|------|------|
| S-T1 | StreamMode 四种模式可区分且可哈希 | 无 | 构造 Values/Updates/Messages/Custom | 相等性、HashSet 去重符合预期 | `stream/types.rs` |
| S-T2 | StreamEvent 各变体携带正确数据 | 任意可 Clone+Debug 的 S | 构造 Values(s), Updates{node_id, state}, Messages{chunk, metadata}, Custom(json) | 解构后字段与构造一致 | `stream/types.rs` |
| S-T3 | MessageChunk / StreamMetadata 字段可读 | 给定 content、langgraph_node | 构造并访问字段 | content、langgraph_node 与给定一致 | `stream/types.rs` |

### 3.2 API 层（stream()）

| ID | Scenario | Given | When | Then | 文件 |
|----|----------|--------|------|------|------|
| S-A1 | stream(Values) 按节点发射状态快照且最后一条为最终状态 | 线性图（如 first→second），初始 state | 调用 stream(state, None, [Values])，collect | 事件数 ≥ 1，全部为 Values，最后一条等于 invoke 的最终 state | `stream/api.rs` |
| S-A2 | stream(Updates) 按执行顺序发射 Updates 且 node_id 顺序正确 | 同上图 | 调用 stream(..., [Updates])，collect | 事件全为 Updates，node_id 序列与边序一致 | `stream/api.rs` |
| S-A3 | stream(Values+Updates) 同时启用两种模式 | 同上图 | stream(..., [Values, Updates])，collect | 每个节点后先有 Values 再有 Updates（或顺序与 run_loop 一致），且两种事件均存在 | `stream/api.rs` |
| S-A4 | stream() 返回实现 Stream<Item = StreamEvent<S>> 的类型 | 任意已编译图 | 调用 stream(...) | 返回类型可 collect，且 Item 为 StreamEvent<S> | `stream/api.rs` |
| S-A5 | stream 无 config 与有 config 均不 panic | 图 + 可选 config | stream(state, None, mode) 与 stream(state, Some(cfg), mode) | 两次均能 collect 完成，无 panic | `stream/api.rs` |

### 3.3 RunContext 与发送条件

| ID | Scenario | Given | When | Then | 文件 |
|----|----------|--------|------|------|------|
| S-R1 | invoke 不设置 RunContext 时无流式发送 | 同上图 | invoke(state, config) | 无 channel 参与，仅返回最终 state | 已由 invoke 测试覆盖，可在此文档注明 |
| S-R2 | stream() 使用 RunContext 时 stream_tx 为 Some、stream_mode 与参数一致 | 图 + stream_mode 集合 | 内部 run_loop 使用 RunContext | Values/Updates 根据 stream_mode 被发送 | 通过 S-A1/S-A2/S-A3 覆盖 |

### 3.4 边界与异常

| ID | Scenario | Given | When | Then | 文件 |
|----|----------|--------|------|------|------|
| S-E1 | 空图 stream 不 panic 且尽快结束 | edge_order 为 [] 的 CompiledStateGraph | stream(initial_state, None, [Values])，collect | 事件数为 0，stream 正常结束 | `stream/edge_cases.rs` |
| S-E2 | 单节点图 stream 发射一条 Values 和一条 Updates | 仅一个节点（START→node→END） | stream(..., [Values, Updates])，collect | 各 1 条 Values、1 条 Updates，state 符合节点逻辑 | `stream/edge_cases.rs` |
| S-E3 | 多模式组合（Values+Updates+Messages+Custom）不 panic | 图当前仅由 run_loop 发射 Values/Updates | stream(..., [Values, Updates, Messages, Custom])，collect | 仅收到 Values 与 Updates（Messages/Custom 由节点发送时再测），无 panic | `stream/edge_cases.rs` |
| S-E4 | 消费者提前 drop 时 sender 不阻塞运行 | 图 + stream | 取 stream 后立即 drop，再 await 图任务结束（若可观测） | 任务能结束，不 deadlock | `stream/edge_cases.rs`（若当前 run_loop 在 spawn 内且无 join，可仅文档化或 mock channel 测试） |

## 4. BDD 风格约定

- 每个测试用 `/// **Scenario**: ...` 注释一句话场景。
- 命名：`stream_<mode>_emits_...`、`stream_empty_graph_...`、`stream_event_variants_...` 等。
- 断言消息清晰：`"last event should be final state"`、`"expected at least one Values event"`。

## 5. 任务跟踪表

| 任务 | 状态 |
|------|------|
| 新增 `langgraph/tests/stream/mod.rs` 并挂到 Cargo | 待办（当前采用内联测试，未建独立目录） |
| 实现 stream 类型层 S-T1（StreamMode HashSet/相等性） | 已完成（`src/stream/mod.rs`：`stream_mode_four_variants_hashset_equality`） |
| 实现 stream API/边界 S-A1～S-A5、S-E1～S-E3 | 已完成（`src/graph/compiled.rs` 内联） |
| 实现 stream/edge_cases S-E4（消费者提前 drop） | 待办（可选） |
| 与 `compiled.rs` 内现有 stream 测试去重或迁移 | 保留现状，不迁移 |
| 全量 `cargo test -p langgraph stream` 通过 | 已完成 |

## 6. 与现有测试的关系

- **`langgraph/src/stream/mod.rs`**：现有 `stream_event_variants_hold_data` 与 S-T2 重叠，可保留内联或迁移到 `tests/stream/types.rs` 统一维护。
- **`langgraph/src/graph/compiled.rs`**：现有 `stream_values_emits_states`、`stream_updates_emit_node_ids_in_order` 对应 S-A1、S-A2；可保留在 compiled 或迁移到 `tests/stream/api.rs`，避免重复。

建议：**先保留 compiled 内现有两个 stream 测试**，新用例在 `tests/stream/` 按上表补充；若后续希望“所有 streaming 测试一处可见”，再迁移到 `tests/stream/api.rs`。

---

以上为以 **streaming** 为维度的单元测试设计与用例表，可直接按表实现并勾选任务状态。
