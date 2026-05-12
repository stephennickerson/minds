# Rust Plugin Migration Plan

## Target

Convert the Claude Code plugin from Python scripts plus `cognee-cli` into one Rust binary that owns all explicit agent tools and all hook commands.

Keep the hook contract. Replace the implementation behind it.

## Current Split

Rust MCP now owns explicit agent actions:

| Action | Target Rust surface |
|---|---|
| recall/search | MCP `recall`, `search` |
| inspect memory | MCP `describe`, `get_status`, `inspect_dataset`, `inspect_graph` |
| explicit remember | MCP `remember` |
| explicit sync | MCP `sync_read_model` |
| forget obsolete memory | MCP `forget` |

Rust now owns automatic lifecycle work:

| Hook | Rust command |
|---|---|
| SessionStart | `cognee-mcp-rs hook session-start` |
| UserPromptSubmit lookup | `cognee-mcp-rs hook context-lookup` |
| UserPromptSubmit store | `cognee-mcp-rs hook store-user-prompt` |
| PostToolUse | `cognee-mcp-rs hook store-tool-use` |
| Stop | `cognee-mcp-rs hook store-stop` |
| PreCompact | `cognee-mcp-rs hook pre-compact` |
| SessionEnd | `cognee-mcp-rs hook session-end` |
| Idle bridge | `cognee-mcp-rs daemon idle-watcher` |
| Status line | `cognee-mcp-rs status-line` |

## Rust Application Support Needed

Add CLI surfaces beside `mcp`:

```bash
cognee-mcp-rs hook session-start
cognee-mcp-rs hook context-lookup
cognee-mcp-rs hook store-user-prompt
cognee-mcp-rs hook store-tool-use
cognee-mcp-rs hook store-stop
cognee-mcp-rs hook pre-compact
cognee-mcp-rs hook session-end
cognee-mcp-rs daemon idle-watcher
cognee-mcp-rs status-line
cognee-mcp-rs tool <tool-name> '<json-arguments>'
```

Shared Rust modules:

| Module | Responsibility |
|---|---|
| `plugin_config` | load env plus `~/.cognee-plugin/config.json` |
| `plugin_state` | read/write `resolved.json`, counters, audit log, watcher files |
| `hook_input` | parse Claude hook stdin payloads |
| `hook_output` | emit valid `hookSpecificOutput` JSON only |
| `session_id` | reproduce per-directory/git-branch/static session IDs |
| `session_store` | store prompt, answer, and trace entries |
| `context_lookup` | build prompt-time memory context from Rust recall packets |
| `bridge` | session cache to graph/read-model sync |
| `watcher` | idle/session-end daemon behavior |

## Hook Migration Order

1. `status-line`
   - Lowest risk.
   - Reads only local state JSON.
   - Replace shell script after byte-for-byte visual parity.

2. `session-start`
   - Compute session/dataset/cwd.
   - Write `~/.cognee-plugin/resolved.json`.
   - Emit same `hookSpecificOutput` shape.
   - Keep identity registration against Cognee API in Rust.

3. `context-lookup`
   - Replace Python `cognee.recall` with Rust read-model `recall`.
   - Preserve `additionalContext`, `systemMessage`, `last_recall.json`, and `recall-audit.log`.
   - Target latency: under 100ms from warm read model.

4. `store-user-prompt`, `store-tool-use`, `store-stop`
   - Parse hook payloads in Rust.
   - Store entries through Rust `remember_entry` support.
   - Preserve save counters and activity timestamps.

5. `pre-compact`
   - Generate markdown memory anchor from Rust recall, session state, and graph handles.
   - Must preserve useful context even when recall has no hits.

6. `session-end` and `idle-watcher`
   - Migrate last because these are the reliability layer.
   - Preserve detached shutdown behavior, stale lock recovery, and no-blocking Claude exit.

7. Remove Python scripts
   - Only after all hook parity tests pass on Windows and Linux.

## Required Rust Backend Additions

The Rust MCP needs more than the current read model for full plugin replacement:

| Capability | Needed for |
|---|---|
| `remember_entry` for QA/trace entries | prompt/tool/stop hooks |
| `ensure_agent_identity` | SessionStart |
| `ensure_dataset_ready` | SessionStart and remember |
| `session_cache` read/write model | context lookup and bridge |
| `sync_graph_context_to_session` equivalent | session-end/idle bridge |
| JSON hook payload fixtures | contract tests |
| lock file helper | idle/session-end mutual exclusion |
| detached process launcher | SessionEnd final sync |

## Contract Tests

Every migrated hook needs fixture tests:

| Test | Acceptance |
|---|---|
| Hook stdout purity | stdout is valid JSON or valid markdown, never logs |
| Payload compatibility | accepts real Claude hook payload fixtures |
| State compatibility | writes the same `~/.cognee-plugin/*.json` shapes |
| No Python import | migrated hook runs with Python unavailable |
| MCP recall path | explicit recall/search never calls `cognee-cli` |
| Latency | context lookup stays below 100ms warm |
| Shutdown safety | SessionEnd returns quickly and detached sync continues |
| Linux rebuild | `cargo build --release` recreates Claude and Codex MCP configs |

## Plugin File End State

Final `hooks/hooks.json` should call Rust:

```json
{
  "type": "command",
  "command": "cognee-mcp-rs hook context-lookup",
  "timeout": 15,
  "statusMessage": "Searching Cognee memory..."
}
```

Final plugin folder should keep:

- `.claude-plugin/plugin.json`
- `hooks/hooks.json`
- `agents/cognee-recall.md`
- `skills/*/SKILL.md`
- `AFTER_INSTALL_MCP.md`
- this migration plan

Final plugin folder should remove:

- Python hook scripts
- shell wrappers
- `cognee-cli` instructions

## Definition Of Done

The migration is done when:

- `claude plugin validate integrations/claude-code` passes
- every hook command runs through `cognee-mcp-rs`
- explicit agent memory actions use MCP tools
- the Haiku recall agent has MCP tool access
- no plugin doc tells agents to use `cognee-cli`
- Python is not required for the Claude Code plugin
- release build still auto-configures Claude and Codex MCP globally
- live recall/search/remember/sync works against the local Cognee server
