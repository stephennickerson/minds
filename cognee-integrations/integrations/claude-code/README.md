# Cognee Memory Plugin for Claude Code

Gives Claude Code persistent memory across sessions using Cognee's knowledge graph. Rust hooks capture tool calls and responses into session memory, inject relevant context on every prompt, and bridge session data into the permanent knowledge graph at session end.

## Install

### 1. Build the Rust MCP

```bash
cd /path/to/minds/cognee-mcp
cargo build --release
```

The release build installs `cognee-mcp-rs` and registers the `cognee-mcp` MCP server for Claude Code and Codex.

### 2. Configure

Connect to a local or remote Cognee API server:

```bash
export COGNEE_SERVICE_URL="http://localhost:8000"   # or your cloud URL
export COGNEE_API_KEY="your-api-key"                # optional if auth is disabled
```

**Cognee Cloud**:

```bash
export COGNEE_SERVICE_URL="https://your-instance.cognee.ai"
export COGNEE_API_KEY="ck_..."
```

Or create `~/.cognee-plugin/config.json`:

```json
{
  "service_url": "http://localhost:8000",
  "dataset": "claude_sessions"
}
```

### 3. Enable the plugin

**Option A — permanent (recommended):**

Add the plugin directory to your shell profile so it loads on every session:

```bash
# Add to ~/.zshrc or ~/.bashrc
alias claude="claude --plugin-dir /path/to/cognee-integrations/integrations/claude-code"
```

Then reload: `source ~/.zshrc`

**Option B — single session:**

```bash
claude --plugin-dir /path/to/cognee-integrations/integrations/claude-code
```

**Option C — validate first:**

```bash
claude plugin validate /path/to/cognee-integrations/integrations/claude-code
```

When the plugin loads, you'll see "Cognee Memory Connected" with the dataset and session ID at the start of your session.

## How it works

The plugin hooks into six Claude Code lifecycle events through `cognee-mcp-rs hook ...`:

| Hook | What it does |
|------|-------------|
| **SessionStart** | Loads config, computes a per-directory session ID, writes Rust state, starts idle watcher |
| **UserPromptSubmit** | Searches Rust session state and the Rust read model for context relevant to your prompt |
| **PostToolUse** | Captures tool name, input, and output into Rust session state with `agent_actions` bridge data |
| **Stop** | Captures the final assistant response when you interrupt |
| **PreCompact** | Builds a memory anchor from Rust session state and graph context |
| **SessionEnd** | Bridges session data into Cognee through Rust and refreshes the read model |

## Data categories

The plugin organizes knowledge into three categories via `node_set` tagging:

| Category | Node set | What belongs here |
|----------|----------|-------------------|
| **user** | `user_context` | User preferences, corrections, personal facts |
| **project** | `project_docs` | Repository docs, code context, architecture decisions |
| **agent** | `agent_actions` | Tool call logs, reasoning traces (auto-captured by hooks) |

When using `/cognee-memory:cognee-remember`, Claude routes data to the correct category through MCP. When searching with `/cognee-memory:cognee-search`, Claude uses the Rust MCP recall/search packet and preserves source handles.

## Session naming

Sessions are scoped per working directory by default. The session ID is derived from a prefix + directory name + hash:

```
cc_my-project_a1b2c3d4e5f6
```

You can change the strategy via config or env vars:

| Strategy | Env var | Behavior |
|----------|---------|----------|
| `per-directory` (default) | `COGNEE_SESSION_STRATEGY=per-directory` | One session per project directory |
| `git-branch` | `COGNEE_SESSION_STRATEGY=git-branch` | Includes git branch in session ID |
| `static` | `COGNEE_SESSION_ID=my-session` | Fixed session ID (legacy compat) |

## Skills

Three skills are available as slash commands:

- **`/cognee-memory:cognee-remember`** — permanently store data in the knowledge graph through the Rust MCP bridge. Routes to user/project/agent category.
- **`/cognee-memory:cognee-search`** — explicitly search session or graph memory, optionally filtered by category. Automatic search happens on every prompt via hooks.
- **`/cognee-memory:cognee-sync`** — force-sync session data to the permanent graph without waiting for session end

Explicit skill actions and lifecycle hooks use the Rust `cognee-mcp` MCP server/runtime.

## Status line (optional)

Adds a one-line status display at the bottom of your terminal showing cognee mode/dataset/session, recall hit counts from the most recent prompt, and saves accumulated for the current turn.

Claude Code's `statusLine` setting is per-user, so you wire it into `~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "cognee-mcp-rs status-line"
  }
}
```

Example output:

```
cognee[rust] ds=claude_sessions sess=74f2b7ad530a | recall: 5s/5t/1g | saving: 1p/0t/1a
```

The Rust status command reads three small JSON state files written by the plugin:

| File | Source | Surfaces |
|---|---|---|
| `~/.cognee-plugin/resolved.json` | SessionStart hook | mode, dataset, short session id |
| `~/.cognee-plugin/last_recall.json` | UserPromptSubmit hook | session/trace/graph_context hit counts |
| `~/.cognee-plugin/save_counter.json` | per-turn save hooks | prompt/trace/answer save counts (resets each prompt) |

Any missing piece is silently omitted, so the line stays short on idle turns.

## Audit log

The UserPromptSubmit hook also appends a JSONL entry per prompt to `~/.cognee-plugin/recall-audit.log`, recording the timestamp, session_id, prompt text, hit counts, and the full `additionalContext` injected into Claude's input. This is the source of truth for "what did the plugin give Claude on prompt X" — Claude Code does not persist hook `additionalContext` into its JSONL transcript.

```bash
tail -n 1 ~/.cognee-plugin/recall-audit.log | jq -r .context
```

## Configuration reference

| Key | Env var | Default | Description |
|-----|---------|---------|-------------|
| `dataset` | `COGNEE_PLUGIN_DATASET` | `claude_sessions` | Dataset name for permanent storage |
| `session_strategy` | `COGNEE_SESSION_STRATEGY` | `per-directory` | Session naming strategy |
| `session_prefix` | `COGNEE_SESSION_PREFIX` | `cc` | Prefix for session IDs |
| `service_url` | `COGNEE_SERVICE_URL` | -- | Cognee Cloud URL |
| `api_key` | `COGNEE_API_KEY` | -- | Cognee Cloud API key |
| `top_k` | -- | `5` | Results returned by automatic session search (per scope) |

Config is resolved in order: env vars > `~/.cognee-plugin/config.json` > defaults.
