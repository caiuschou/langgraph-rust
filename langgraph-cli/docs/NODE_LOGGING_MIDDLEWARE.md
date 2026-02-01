# langgraph-cli 通过 Middleware 增加 Node 调用日志

本文档描述如何通过 `NodeMiddleware` 为 langgraph-cli 增加 node 调用的日志打印。

## 目标

- 在 graph 执行时，打印每个 node 的 enter / exit 日志
- 使用 langgraph 已有的 `NodeMiddleware` 机制，无需修改 langgraph 核心
- 方案简单，易于维护

## 背景

### langgraph 已支持

- `NodeMiddleware<S>` trait：定义于 `langgraph::graph::node_middleware`
- `StateGraph::compile_with_middleware(middleware)`：compile 时传入 middleware
- `StateGraph::with_middleware(middleware)`：链式 API，返回 `Self`，可与 `.compile()` 连用
- `around_run(node_id, state, inner)`：在调用实际 `node.run` 前后可插入逻辑

### langgraph-cli 现状

- `run_with_config` 使用链式调用：`graph.with_node_logging().compile()?`（见下）
- 图结构：think → act → observe

## 方案

### 1. 实现 LoggingMiddleware

在 langgraph-cli 中实现一个简单的 `LoggingMiddleware`，实现 `NodeMiddleware<ReActState>`：

```rust
// langgraph-cli/src/logging_middleware.rs

use async_trait::async_trait;
use std::pin::Pin;
use std::sync::Arc;

use langgraph::{AgentError, NodeMiddleware, Next, ReActState};

/// Middleware that logs node enter/exit around each node.run call.
pub struct LoggingMiddleware;

#[async_trait]
impl NodeMiddleware<ReActState> for LoggingMiddleware {
    async fn around_run(
        &self,
        node_id: &str,
        state: ReActState,
        inner: Box<
            dyn FnOnce(ReActState)
                -> Pin<Box<dyn std::future::Future<Output = Result<(ReActState, Next), AgentError>> + Send>>
                + Send,
        >,
    ) -> Result<(ReActState, Next), AgentError> {
        eprintln!("[node] enter node={}", node_id);
        let result = inner(state).await;
        match &result {
            Ok((_, ref next)) => eprintln!("[node] exit node={} next={:?}", node_id, next),
            Err(e) => eprintln!("[node] exit node={} error={}", node_id, e),
        }
        result
    }
}
```

### 2. 扩展方法与 run_with_config

langgraph-cli 为 `StateGraph<ReActState>` 提供扩展 trait `WithNodeLogging`，链式挂上日志 middleware 再编译：

```rust
// 推荐（链式）：
let compiled: CompiledStateGraph<ReActState> = graph.with_node_logging().compile()?;

// 等价于：
let compiled = graph.with_middleware(Arc::new(LoggingMiddleware)).compile()?;

// 仍可单独传 middleware 编译（兼容）：
let compiled = graph.compile_with_middleware(Arc::new(LoggingMiddleware))?;
```

### 3. 依赖

- `async-trait`：langgraph-cli 需增加 `async-trait` 依赖（若未使用）
- `NodeMiddleware`：从 `langgraph` 已导出，直接使用

## 任务列表

| 任务 | 状态 |
|------|------|
| 添加 async-trait 依赖到 langgraph-cli Cargo.toml | 完成 |
| 创建 logging_middleware.rs 模块 | 完成 |
| 在 lib.rs 中集成 LoggingMiddleware 到 compile | 完成 |
| 验证：运行 CLI 能看到 node enter/exit 日志 | 完成 |

## 可选扩展

### 环境变量控制

可通过 `LANGGRAPH_LOG_NODES=1` 控制是否启用日志 middleware：

```rust
let compiled = if std::env::var("LANGGRAPH_LOG_NODES").is_ok() {
    graph.with_node_logging().compile()?
} else {
    graph.compile()?
};
```

### 结构化日志

若后续使用 `tracing` 等，可将 `eprintln!` 改为 `tracing::info!`，便于与现有日志系统集成。

## 参考

- idea/NODE_MIDDLEWARE.md：NodeMiddleware 设计与 around 模式
- idea/NODE_MIDDLEWARE_OPTIONS.md：链式 API（with_middleware / with_node_logging）与外部扩展链
- idea/MIDDLEWARE_CRATE.md：可选 langgraph-middleware 独立 crate 方案
- langgraph/tests/state_graph.rs：LoggingMiddleware 测试示例
