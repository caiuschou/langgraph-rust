# langgraph-cli 开发计划

> 针对本仓库 **langgraph-cli**（Rust 实现的 ReAct 运行 CLI/库）的现状、目标与开发规划。  
> 与 [agent-loop-and-output-refinement.md](./agent-loop-and-output-refinement.md)、[multi-turn-conversation.md](./multi-turn-conversation.md)、[search-result-handling.md](./search-result-handling.md) 互补：本文档侧重 **CLI 产品能力** 与 **落地步骤**。

---

## 1. 现状与目标

### 1.1 当前能力

- **定位**：在 langgraph（Rust）之上提供「单次运行 ReAct 图」的 CLI 与可复用库。
- **图结构**：think → act → observe → END；无输出精炼循环（见 [agent-loop-and-output-refinement.md](./agent-loop-and-output-refinement.md)）。
- **配置**：从 `.env` 读取 OpenAI 相关；CLI 支持 `-m`、`-t`、`--tool-choice`、`--thread-id`、`--user-id`、`--db-path`、Exa MCP 等。
- **内存**：库侧支持 `with_short_term_memory` / `with_long_term_memory` / `with_memory`；CLI 传 `--thread-id` / `--user-id` 时可写入 checkpoint/store，但**不加载历史**（见 [multi-turn-conversation.md](./multi-turn-conversation.md)）。
- **输出**：单次运行结束后打印全部 `state.messages`（System/User/Assistant），无流式、无结构化输出格式。

### 1.2 痛点与改进目标

| 痛点 | 改进目标 |
|------|----------|
| CLI 跨次执行不加载 checkpoint，传 `--thread-id` 仍每次「全新」对话 | 支持**跨请求多轮**：按 thread_id 加载上一轮 state，再追加本轮用户消息后 invoke。 |
| 仅打印整段 messages，无流式、无「最后一条助手回复」快捷输出 | 提供**流式输出**选项与**输出格式**选项（如仅最后 Assistant、JSON 等）。 |
| 配置分散在 .env + CLI 参数，无统一配置文件 | 可选支持 **langgraph.json** 或等效配置入口，便于与官方生态对齐、库复用。 |
| 无 dev/server 模式，难以做 API 或交互式会话 | 规划 **REPL 模式** 或 **本地 HTTP/SSE 服务**，便于多轮与集成。 |
| 可观测不足（仅可选 node logging） | 增强 **日志/追踪/中间结果** 的可配置输出，便于调试与评估。 |

下文按**模块**拆解开发计划，并标注优先级与依赖。

---

## 2. 与官方 LangGraph CLI 的对照（参考）

官方 [LangGraph CLI](https://docs.langchain.com/langgraph-platform/langgraph-cli)（Python/JS）主要提供：

| 能力 | 官方 CLI | 本仓库 langgraph-cli |
|------|----------|----------------------|
| 本地开发 | `langgraph dev`（热重载） | 无；可规划「watch + 重跑」或 REPL。 |
| 本地 API 服务 | `langgraph up`（Docker） | 无；可规划独立 `serve` 子命令或示例。 |
| 构建与部署 | `langgraph build` / `langgraph dockerfile` | 无；可规划 Dockerfile 与构建脚本。 |
| 项目脚手架 | `langgraph new` | 无；可选。 |
| 配置 | `langgraph.json` | 当前仅 .env + CLI；可增加配置文件。 |

本仓库 CLI 定位为 **Rust 生态内运行 ReAct 的轻量入口**，与官方 CLI 功能对齐可作为长期参考，不必一一对应；优先满足「单机多轮、流式、可配置、可观测」。

---

## 3. 开发计划（按模块）

### 3.1 多轮对话（跨请求）

- **目标**：同一 `thread_id` 下多次执行 CLI（或多次库调用）时，能加载上一轮 checkpoint，形成连续对话。
- **现状**：见 [multi-turn-conversation.md § 3.1](./multi-turn-conversation.md#31-langgraph-clirun_react_graph)：当前初始 state 始终为全新 `[system_prompt, user_message]`，不执行 `checkpointer.get_tuple`。
- **实现要点**：
  1. 在 `run_react_graph`（或 `run_with_config` 调用前）中，若 `runnable_config.thread_id` 存在，则先 `checkpointer.get_tuple(&config)`。
  2. 若得到 `Some((checkpoint, _))`：从 `checkpoint.channel_values` 取出上一轮 `ReActState`，在其 `messages` 末尾追加本轮 `Message::User(user_message)`，并清空本轮不需携带的 `tool_calls`/`tool_results`（若实现上有该字段复用）。
  3. 若为 `None`：保持现有逻辑，初始 state = system + user。
  4. CLI：已有 `--thread-id`，无需改参数；库侧调用时传入带 `thread_id` 的 config 即可。
- **验收**：同一 thread_id 连续两次 `run`，第二次应包含第一次的对话历史并在其基础上回复。
- **优先级**：高（多轮是 CLI/API 的常见诉求）。

### 3.2 流式输出与输出格式

- **目标**：支持边推理边输出（流式）；支持只打印「最后一条助手消息」或机器可读格式（如 JSON）。
- **现状**：单次 `invoke` 结束后遍历 `state.messages` 打印。
- **实现要点**：
  1. **流式**：使用 `CompiledStateGraph::stream`（若已有 per-step 或 per-node 流），在 CLI 中增加 `--stream`，每收到一次增量即写入 stdout；需约定是「仅最后 Assistant 内容流式」还是「每步 state 摘要」。
  2. **输出格式**：增加 `--output` 或 `--format`：`full`（当前行为）、`last-assistant`（仅最后一条 Assistant）、`json`（整段 state 或 messages 的 JSON），便于管道与脚本。
- **依赖**：流式依赖 langgraph 的 `stream` API 稳定与语义明确。
- **优先级**：高（体验与集成友好）。

### 3.3 配置文件（可选）

- **目标**：支持通过 `langgraph.json`（或 `langgraph.toml`）指定默认 model、temperature、db_path、tool 源等，与 CLI 参数、.env 叠加（优先级：CLI > 配置文件 > .env）。
- **实现要点**：增加配置解析模块，在 `RunConfig` / `RunOptions` 构建时合并三者；若与官方命名部分对齐，便于后续与 LangGraph 平台对接。
- **优先级**：中（提升可维护性与一致性）。

### 3.4 REPL 或本地服务模式

- **目标**：支持「交互式多轮」或「HTTP/SSE API」，无需用户每次手传 `--thread-id`。
- **方案 A – REPL**：子命令 `langgraph repl`，在进程内循环读 stdin，维护 `thread_id`（可固定或按会话生成），每行作为 User 消息调用 `run_with_config`，并打印助手回复；历史通过内存或 checkpoint 保持。
- **方案 B – 本地服务**：子命令 `langgraph serve`，启动 HTTP 服务，提供例如 `POST /runs` 或 `/threads/:id/messages`，内部按 thread_id 加载 checkpoint 后 invoke，可选 SSE 流式响应。
- **优先级**：中高（REPL 可先做，服务可后续或单独示例）。

### 3.5 可观测与调试

- **目标**：便于排查工具调用、中间 state、耗时与错误。
- **实现要点**：
  1. **结构化日志**：已有 `WithNodeLogging`；可增加 `--verbose` / `--debug` 控制日志级别，并输出每步 node 名与 state 摘要（如 message 条数、最后一条类型）。
  2. **中间结果**：可选 `--dump-state` 在结束时输出完整 state 的 JSON 或路径，便于复现与评估。
  3. **错误信息**：确保 API/LLM/工具错误带上下文（如 thread_id、step），便于反馈与文档。
- **优先级**：中。

### 3.6 部署与构建（可选）

- **目标**：便于在容器或 CI 中运行 CLI 或基于 CLI 的 API。
- **实现要点**：提供或文档化 `Dockerfile`（多阶段构建，仅包含 `langgraph-cli` 二进制）；可选 `langgraph build` 子命令仅构建二进制并输出路径。与官方 `langgraph build`（镜像构建）区分即可。
- **优先级**：低（按需）。

---

## 4. 优先级与阶段汇总

| 阶段 | 内容 | 说明 |
|------|------|------|
| **P0** | 跨请求多轮（按 thread_id 加载 checkpoint） | 见 § 3.1；实现后 CLI/库即支持真实多轮。 |
| **P0** | 流式输出 + 输出格式（--stream、--output） | 见 § 3.2；提升体验与脚本友好。 |
| **P1** | REPL 模式 | 见 § 3.4 方案 A；同进程多轮 + 易用。 |
| **P1** | 可观测（--verbose、--dump-state） | 见 § 3.5。 |
| **P2** | 配置文件（langgraph.json / .toml） | 见 § 3.3。 |
| **P2** | 本地 HTTP/SSE 服务（或示例） | 见 § 3.4 方案 B。 |
| **P3** | Dockerfile / 构建脚本 | 见 § 3.6。 |

---

## 5. 相关代码索引（本仓库）

| 说明 | 文件与位置 |
|------|------------|
| CLI 入口与参数 | `langgraph-cli/src/main.rs` |
| 运行选项与配置 | `langgraph-cli/src/config/run_options.rs`、`run_config.rs` |
| 单次运行入口 | `langgraph-cli/src/run/run_with_config.rs` |
| ReAct 图构建与 invoke | `langgraph-cli/src/run/common.rs`（`run_react_graph`） |
| 初始 state 构造 | `langgraph-cli/src/run/common.rs`（约 55–61 行，当前固定为 system + user） |
| Checkpointer 写入 | `langgraph/src/graph/compiled.rs`（run_loop_inner）；加载需在 CLI/run 层实现 |
| 多轮实现说明 | [multi-turn-conversation.md § 4](./multi-turn-conversation.md#4-跨请求跨进程多轮的实现方式) |

---

## 6. 参考链接

| 主题 | URL |
|------|-----|
| 官方 LangGraph CLI | https://docs.langchain.com/langgraph-platform/langgraph-cli |
| 本仓库多轮对话 | [multi-turn-conversation.md](./multi-turn-conversation.md) |
| 本仓库 Agent 循环与输出精炼 | [agent-loop-and-output-refinement.md](./agent-loop-and-output-refinement.md) |
| 本仓库检索/工具结果处理 | [search-result-handling.md](./search-result-handling.md) |

---

*文档版本：初稿。实现 P0/P1 项后可在本文档中增加「实现状态」小节并链接到具体 PR 或提交。*
