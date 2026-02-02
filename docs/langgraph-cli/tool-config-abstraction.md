# langgraph-cli 工具配置抽象方案（方案一）

## 目标

- 将工具相关配置抽象为独立类型 `ToolSourceConfig`，仅保留 `exa_api_key: Option<String>`。
- 无 key 时默认关闭 Exa MCP（`has_exa = exa_api_key.is_some()`）。
- 为 `config.memory`、`config.db_path` 提供默认值。

## 变更概要

| 项 | 变更 |
|----|------|
| **ToolSourceConfig** | 新增，仅字段 `exa_api_key: Option<String>`，`Default` 为 `None`。 |
| **RunConfig** | 移除 `use_exa_mcp`、`exa_api_key`；新增 `tool_source: ToolSourceConfig`。保留 `mcp_exa_url`、`mcp_remote_cmd`、`mcp_remote_args` 用于 MCP 客户端构造。 |
| **默认值** | `memory`：`from_env` 未设置 THREAD_ID/USER_ID 时为 `MemoryConfig::NoMemory`（已有）。`db_path`：`from_env` 未设置 DB_PATH 时为 `Some("memory.db".to_string())`。 |
| **run_with_config** | `has_exa = config.tool_source.exa_api_key.is_some()`；key 取自 `config.tool_source.exa_api_key`。 |
| **main** | 使用 `config.tool_source.exa_api_key`；移除对 `use_exa_mcp` 的赋值；`--mcp-exa` 仅在与 `--exa-api-key` 或 env EXA_API_KEY 同时存在时有效。 |

## 文件变更

1. **新增** `langgraph-cli/src/config/tool_source_config.rs`  
   - 定义 `ToolSourceConfig { exa_api_key: Option<String> }`，`impl Default`。

2. **修改** `langgraph-cli/src/config/mod.rs`  
   - 增加 `mod tool_source_config` 与 `pub use tool_source_config::ToolSourceConfig`。

3. **修改** `langgraph-cli/src/config/run_config.rs`  
   - 移除 `use_exa_mcp`、`exa_api_key`；新增 `tool_source: ToolSourceConfig`。  
   - `from_env`：`tool_source.exa_api_key = std::env::var("EXA_API_KEY").ok()`；`db_path = std::env::var("DB_PATH").ok().or_else(|| Some("memory.db".to_string()))`。

4. **修改** `langgraph-cli/src/run/run_with_config.rs`  
   - `has_exa = config.tool_source.exa_api_key.is_some()`；Exa key 使用 `config.tool_source.exa_api_key.as_ref().unwrap()`。

5. **修改** `langgraph-cli/src/main.rs`  
   - 用 `config.tool_source.exa_api_key` 替代 `config.exa_api_key`；删除 `config.use_exa_mcp = true`；`--exa-api-key` 写入 `config.tool_source.exa_api_key`。

## 行为说明

- **Exa 开关**：仅当 `tool_source.exa_api_key.is_some()` 时启用 Exa MCP；无 key 即默认关闭。
- **db_path 默认**：未设置 `DB_PATH` 时使用 `"memory.db"`。
- **memory 默认**：未设置 `THREAD_ID`/`USER_ID` 时为 `NoMemory`（与现有行为一致）。
