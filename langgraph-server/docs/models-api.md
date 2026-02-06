# langgraph-server `/v1/models` 接口方案

## 1. 目标与约束

- **目标**：为 langgraph-server 增加 **GET /v1/models**（列表）与 **GET /v1/models/{model_id}**（详情），与 OpenAI Models API 行为一致。
- **约束**：
  - **不改动其他代码**：不修改 langgraph 库、不修改现有 chat/completions 或 ReactRunner/LlmClient 等逻辑，仅在 langgraph-server 内新增路由与 handler。
  - **直接使用 HTTP 请求**：通过 HTTP 客户端（如 reqwest）请求上游 API，不依赖 async_openai 的 models 模块。

## 2. 接口行为

| 方法 | 路径 | 行为 |
|------|------|------|
| GET | /v1/models | 代理到 `{OPENAI_BASE_URL}/v1/models`，带 `Authorization: Bearer {OPENAI_API_KEY}`，将上游响应体与状态码返回给客户端。 |
| GET | /v1/models/{model_id} | 代理到 `{OPENAI_BASE_URL}/v1/models/{model_id}`，同样带鉴权，原样返回。 |

- **前置条件**：需配置 `OPENAI_BASE_URL`（或 `OPENAI_API_BASE`）与 `OPENAI_API_KEY`。若未配置 base URL，可返回 503 或 400，并提示需配置上游地址。
- **响应**：不解析、不改写上游 JSON，仅做透明代理（状态码、Header 中的 Content-Type、Body 与上游一致；可选：只转发 Body 并统一 200，视需求而定）。

## 3. 实现方式：直接 HTTP 请求

### 3.1 依赖

- 在 **langgraph-server/Cargo.toml** 中增加：
  - `reqwest = { version = "0.12", features = ["json"] }`（或与 workspace 已有版本一致）。

### 3.2 上游请求构造

- **Base URL**：从现有 `ReactBuildConfig::from_env()` 或环境变量读取 `OPENAI_BASE_URL` / `OPENAI_API_BASE`，去掉末尾 `/`。
- **URL**：
  - 列表：`{base}/v1/models`
  - 详情：`{base}/v1/models/{model_id}`
- **Header**：`Authorization: Bearer {OPENAI_API_KEY}`（与 chat 使用同一 key）。
- **方法**：GET，无 body。
- **超时**：建议 10–30 秒（如 `reqwest::Client::new().timeout(Duration::from_secs(15))`）。

### 3.3 Handler 逻辑（伪代码）

```
GET /v1/models:
  若 base_url 缺失 => 返回 503 + 说明需配置 OPENAI_BASE_URL
  否则 => 
    res = reqwest::Client::get(base_url + "/v1/models")
           .header("Authorization", "Bearer " + api_key)
           .send().await
    将 res.status()、res.headers() 中 Content-Type、res.bytes() 转为响应返回

GET /v1/models/:model_id:
  若 base_url 缺失 => 返回 503
  否则 =>
    res = reqwest::Client::get(base_url + "/v1/models/" + model_id)
           .header("Authorization", "Bearer " + api_key)
           .send().await
    同上，原样转发状态码与 body
```

- 上游 4xx/5xx：建议**原样转发**状态码与 body，便于客户端看到真实错误；若需统一格式，可再包一层 `{ "error": { "message": "..." } }`。

### 3.4 状态与路由

- **状态**：现有 `State` 为 `Arc<ReactRunner>`。可选做法：
  - **A**：在注册 `/v1/models` 的路由时，使用 `Router::merge` 或嵌套子 Router，子 Router 的 state 为 `(base_url, api_key)`，从 `main` 里已读取的 `build_config.openai_base_url` / `build_config.openai_api_key` 传入；
  - **B**：扩展为统一 AppState，如 `struct AppState { runner: Arc<ReactRunner>, openai_base_url: Option<String>, openai_api_key: String }`，所有路由共用该 state。
- **路由**：在现有 `Router::new()` 上增加：
  - `.route("/v1/models", get(models_list))`
  - `.route("/v1/models/:model_id", get(model_retrieve))`
- 需在 axum 中 `use axum::routing::get` 与 `use axum::extract::Path`（用于 `model_id`）。

### 3.5 错误处理

- **上游超时 / 连接失败**：返回 502 Bad Gateway 或 504 Gateway Timeout，body 可为 `{ "error": { "message": "upstream timeout" } }`。
- **base_url 未配置**：返回 503，body 建议说明需设置 OPENAI_BASE_URL（或 OPENAI_API_BASE）。

## 4. 不改动的部分

- **langgraph**：不修改 RunnableConfig、LlmClient、ThinkNode、ChatOpenAI、ReactRunner 等。
- **langgraph-server**：不修改 `chat_completions` 的 State 类型或业务逻辑；仅新增 models 相关 handler 与路由（若采用统一 AppState，则仅扩展 state 类型并让 chat_completions 从 state 中取 `runner`）。

## 5. 实现步骤建议

1. 在 **langgraph-server/Cargo.toml** 添加 `reqwest` 依赖。
2. 在 **main.rs** 中增加 `models_list`、`model_retrieve` 两个 async handler，内部使用 `reqwest::Client` 请求上游，并按照 3.3 将响应状态码与 body 返回（必要时设置 Content-Type）。
3. 决定 state 方案（3.4 的 A 或 B），在 **main** 中构造 state 并注册 `GET /v1/models` 与 `GET /v1/models/:model_id`。
4. 在 **README** 中补充说明：`GET /v1/models`、`GET /v1/models/{id}` 为上游代理，需配置 `OPENAI_BASE_URL` 与 `OPENAI_API_KEY`。

## 6. 验收

- 配置好 `OPENAI_BASE_URL` 与 `OPENAI_API_KEY` 后：
  - `curl http://127.0.0.1:8123/v1/models` 返回与直接请求 `{OPENAI_BASE_URL}/v1/models` 一致的列表 JSON。
  - `curl http://127.0.0.1:8123/v1/models/gpt-4o-mini` 返回与上游一致的单个模型对象。
- 未配置 base_url 时，GET /v1/models 返回 503 及明确提示。

---

**总结**：通过直接 HTTP（reqwest）代理到上游 `/v1/models` 与 `/v1/models/{model_id}`，在不大改现有代码的前提下，为 langgraph-server 提供与 OpenAI 兼容的模型列表与详情接口。
