# Cognee MCP

Rust MCP server for Cognee. The default agent surface reads and mutates the local Rust SQLite read model without calling Python/Cognee HTTP endpoints.

## Run

```powershell
$env:COGNEE_SERVICE_URL = "http://localhost:8000"
$env:COGNEE_MCP_READ_MODEL_PATH = ".cognee/read_model.sqlite"
cargo run --release -- --service-url $env:COGNEE_SERVICE_URL
```

## MCP Install

`cargo build --release` automatically installs the MCP surface for this user:

- creates the project root `.mcp.json`
- creates a stable launcher in `~/.local/bin` or `%USERPROFILE%\.local\bin`
- configures Claude Code user scope as `cognee-mcp`
- configures Codex in `~/.codex/config.toml` as `cognee-mcp`

Set `COGNEE_MCP_SKIP_AUTO_INSTALL=true` to disable auto-install for CI or packaging.

## Recall

`recall` defaults to fast Rust evidence retrieval:

- `llm_presummary=false` is the default.
- Default recall reads ranked evidence and graph relationships from the SQLite read model.
- Default recall does not call `POST /api/v1/recall`.
- `llm_presummary=true` is blocked on the default agent surface and requires operator tools.

The read model is stored at `COGNEE_MCP_READ_MODEL_PATH`, defaulting to `.cognee/read_model.sqlite`.

Any MCP tool can also be called from scripts through the same Rust binary:

```powershell
cognee-mcp-rs tool recall '{ "query": "fleet smoke API", "top_k": 5 }'
'{ "query": "fleet smoke API", "top_k": 5 }' | cognee-mcp-rs tool recall -
```

## Claude Code Hooks

The same Rust binary owns the Claude Code plugin lifecycle:

```powershell
cognee-mcp-rs hook session-start
cognee-mcp-rs hook context-lookup
cognee-mcp-rs hook store-user-prompt
cognee-mcp-rs hook store-tool-use
cognee-mcp-rs hook store-stop
cognee-mcp-rs hook pre-compact
cognee-mcp-rs hook session-end
cognee-mcp-rs daemon idle-watcher
cognee-mcp-rs status-line
```

Hook state is stored in `~/.cognee-plugin` by default. Set `COGNEE_PLUGIN_STATE_DIR` to use another state directory.

## Test

```powershell
cargo fmt --check
cargo check
cargo clippy -- -D warnings
$env:COGNEE_MCP_LIVE_TESTS = "true"
cargo test
cargo build --release
```

## Recall Shootout

```powershell
cargo build --release
.\scripts\recall_shootout.ps1 `
  -DatasetName "fleet_smoke_api_20260510" `
  -DatasetId "40089743-cf53-50f0-bf25-3ce347e8d6d7" `
  -ServiceUrl "http://localhost:8000"
```

The harness compares:

- `POST /api/v1/recall` on the running Cognee server
- MCP `recall` with `llm_presummary=false`
- MCP `recall` with `llm_presummary=true`

It writes `target/recall-shootout.md` with latency, scoring, winner, and failure notes.

## Tools

Default agent surfaces are Rust MCP tools. Read tools use the local read model; explicit writes and syncs go through the Rust bridge:

- `describe`
- `get_status`
- `recall`
- `search`
- `inspect_dataset`
- `inspect_graph`
- `remember`
- `forget`
- `sync_read_model`

Python-backed slow/background/operator surfaces are disabled by default and require:

```powershell
$env:COGNEE_MCP_ENABLE_OPERATOR_TOOLS = "true"
```

Operator surfaces:

- `add`
- `cognify`
- `improve`
- `manage_schema`
- `manage_ontology`
