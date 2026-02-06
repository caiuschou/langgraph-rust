# langgraph-server

HTTP server that exposes:

- **POST /v1/chat/completions** – OpenAI Chat Completions–compatible SSE streaming.
- **POST /v1/responses** – [OpenAI Responses API](https://platform.openai.com/docs/api-reference/responses/create)–compatible (JSON or SSE).

Both are backed by the ReAct agent (langgraph).

## Config (env)

- **OPENAI_API_KEY** (required): OpenAI API key.
- **OPENAI_MODEL**: Model name (default: `gpt-4o-mini`).
- **OPENAI_BASE_URL** or **OPENAI_API_BASE**: Optional API base URL (e.g. `https://gptproto.com/v1`). If only `OPENAI_API_BASE` is set (as in many .env files), it is used.
- **LISTEN**: Bind address (default: `0.0.0.0:8123`).
- **DB_PATH**, **THREAD_ID**, **USER_ID**, **EXA_API_KEY**, etc.: Same as langgraph / ReactBuildConfig (see langgraph `ReactBuildConfig::from_env()`). If `THREAD_ID` is not set, the server uses `"default"` so the checkpointer is created.

`.env` is loaded at startup: first from the current working directory, then from the parent directory (so running from the repo root or from `langgraph-server/` both find a root `.env`).

## Run

```bash
export OPENAI_API_KEY=sk-...
cargo run -p langgraph-server
# listens on http://0.0.0.0:8123 (all interfaces)
```

## Request (OpenAI-compatible)

```bash
curl -X POST http://127.0.0.1:8123/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-4o-mini","messages":[{"role":"user","content":"Hello"}],"stream":true}'
```

Only `stream: true` is supported. Optional body fields:

- **thread_id**: For multi-turn checkpointing (same as OpenAI extension).
- **stream_options.include_usage**: Include token usage in the final SSE chunk.

Response: `Content-Type: text/event-stream` with `data: <JSON>\n\n` lines (OpenAI chat.completion.chunk format). When the agent calls tools, a chunk with `delta.tool_calls` and `finish_reason: "tool_calls"` is emitted before the next content turn.

### POST /v1/responses (Responses API)

Request body (minimal):

- **input** (required): String (user message) or array of items; last user text is used.
- **model** (optional): Model name (default from server).
- **instructions** (optional): System message (currently not passed to runner; reserved).
- **stream** (optional): If `true`, response is SSE in Responses API event format (`response.created`, `response.output_text.delta`, `response.completed`); otherwise JSON.

Example (non-stream):

```bash
curl -X POST http://127.0.0.1:8123/v1/responses \
  -H "Content-Type: application/json" \
  -d '{"input":"Hello, say hi in one sentence."}'
```

Example (stream):

```bash
curl -X POST http://127.0.0.1:8123/v1/responses \
  -H "Content-Type: application/json" \
  -d '{"input":"Hello","stream":true}'
```
