# 工具重构 (Tools Refactor)

将单体 `ToolSource` 架构重构为"一个工具一个文件"架构。

## 核心概念

### 问题

- **工具定义耦合** - 多个工具在单个文件中
- **难以扩展** - 添加新工具需要修改现有代码
- **测试分散** - 工具测试与 ToolSource 打包在一起
- **复用性受限** - 工具逻辑与 ToolSource 紧密耦合

### 解决方案

```
Tool trait      → 单个工具的抽象接口
ToolRegistry    → 管理工具的中央注册表
AggregateToolSource → 基于 ToolRegistry 的 ToolSource 实现
适配器          → 保留现有 API 的兼容层
```

## 快速开始

### 创建工具

```rust
// 1. 定义工具结构
struct MyTool {
    // 依赖项
}

// 2. 实现 Tool trait
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn spec(&self) -> ToolSpec { /* ... */ }
    async fn call(&self, args: Value, ctx: Option<&ToolCallContext>) -> Result<...> { /* ... */ }
}

// 3. 注册并使用
let mut registry = ToolRegistry::new();
registry.register(Box::new(MyTool::new()));
```

### 使用现有 API（无变化）

```rust
// 重构前后的代码完全相同
let store = Arc::new(InMemoryStore::new());
let ns = vec!["user-123".to_string()];
let source = StoreToolSource::new(store, ns);

let tools = source.list_tools().await?;
let result = source.call_tool("remember", json!({ "key": "k", "value": "v" })).await?;
```

## 文档导航

### 按角色

| 角色 | 首选文档 |
|------|---------|
| 新手 | [概述](overview.md) |
| 创建工具的开发者 | [创建工具指南](guides/creating-tools.md) |
| 迁移代码的开发者 | [迁移指南](guides/migration-guide.md) |
| 实施架构的开发者 | [架构设计](architecture/) |
| 项目经理 | [实施计划](implementation/) |

### 按主题

| 主题 | 文档 |
|------|------|
| 问题分析 | [概述](overview.md) |
| 架构设计 | [架构设计](architecture/) |
| API 变化 | [改动说明](changes.md) |
| 使用方法 | [使用指南](guides/) |
| 实施时间表 | [实施计划](implementation/) |

## 文档结构

```
tools-refactor/
├── README.md              # 本文件 - 主入口
├── overview.md            # 详细概述 - 问题、目标、架构
├── changes.md             # 改动说明 - API 和代码变化
├── architecture/          # 架构设计
│   ├── tool-trait.md      # Tool trait 设计
│   ├── tool-registry.md   # ToolRegistry 设计
│   ├── aggregate-source.md # AggregateToolSource 设计
│   └── common-interface-mcp.md # 工具与 MCP 共用接口方案（单集合 + 适配器）
├── guides/                # 使用指南
│   ├── creating-tools.md  # 如何创建工具
│   └── migration-guide.md # 如何迁移代码
└── implementation/         # 实施计划
    ├── phases.md          # 7 个实施阶段
    └── tasks.md           # 任务跟踪
```

## 推荐阅读路径

### 路径 1：理解重构（新手）

1. [概述](overview.md) - 15 分钟
2. [改动说明](changes.md) - 10 分钟

**总时间：** ~25 分钟

### 路径 2：创建工具（开发者）

1. [Tool trait 设计](architecture/tool-trait.md) - 20 分钟
2. [创建工具指南](guides/creating-tools.md) - 30 分钟

**总时间：** ~50 分钟

### 路径 3：迁移代码（开发者）

1. [概述](overview.md) - 15 分钟
2. [迁移指南](guides/migration-guide.md) - 40 分钟
3. [改动说明](changes.md) - 10 分钟

**总时间：** ~65 分钟

### 路径 4：实施架构（开发者）

1. [架构设计](architecture/) - 75 分钟
2. [实施计划](implementation/phases.md) - 20 分钟

**总时间：** ~95 分钟

### 路径 5：项目管理（PM）

1. [概述](overview.md) - 15 分钟
2. [实施计划](implementation/phases.md) - 20 分钟
3. [任务跟踪](implementation/tasks.md) - 10 分钟

**总时间：** ~45 分钟

## 项目状态

**当前状态：** 规划阶段 ✅

所有设计文档已完成，准备开始实施。

## 关键概念速查

| 概念 | 说明 |
|------|------|
| `Tool` | 定义单个工具的 trait |
| `ToolRegistry` | 管理工具集合的中央注册表 |
| `AggregateToolSource` | 实现 ToolSource trait 的结构体 |
| `适配器` | 包装新架构以保持现有 API |
| `McpToolAdapter` | 将 MCP 工具实现为 `dyn Tool`，进同一 Registry（见 [common-interface-mcp](architecture/common-interface-mcp.md)） |
| `上下文` | 每次调用的数据（例如最近的消息） |

## 常见问题

### Q: 我的代码需要改动吗？

**A:** 如果你使用 `StoreToolSource`、`ShortTermMemoryToolSource` 或 `MemoryToolsSource`，不需要改动。这些 API 保持完全向后兼容。

### Q: 新 API 是必须的吗？

**A:** 不是。新 API（Tool trait、ToolRegistry）是可选的，用于创建自定义工具。

### Q: 如何查看实施进度？

**A:** 查看 [任务跟踪](implementation/tasks.md) 了解最新状态。

## 相关文档

- [详细概述](overview.md)
- [架构设计](architecture/)
- [使用指南](guides/)
- [实施计划](implementation/)
- [改动说明](changes.md)
