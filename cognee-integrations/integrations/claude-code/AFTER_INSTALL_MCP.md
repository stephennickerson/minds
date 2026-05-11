# After Install: Rust MCP Wiring

Run this after pulling and building `cognee-mcp` with `cargo build --release`.

## LLM Command

```text
You are updating the Cognee Claude Code integration after the Rust MCP install.

Goal: every explicit agent memory action must use the Rust `cognee-mcp` MCP server. Automatic Claude Code hooks may keep using the existing hook commands until the Rust hook migration lands.

Do these edits in `integrations/claude-code`:

1. Verify `cognee-mcp` is registered:
   - `claude mcp get cognee-mcp`
   - `codex mcp get cognee-mcp`

2. Update `agents/cognee-recall.md`:
   - keep `model: haiku`
   - add MCP tool access for `mcp__cognee-mcp__recall`, `mcp__cognee-mcp__search`, `mcp__cognee-mcp__inspect_dataset`, `mcp__cognee-mcp__inspect_graph`, `mcp__cognee-mcp__get_status`, and `mcp__cognee-mcp__sync_read_model`
   - remove all `cognee-cli recall` and `scripts/cognee-search.sh` instructions
   - instruct the agent to use `recall` with `llm_presummary=false` by default
   - require returned answers to preserve source handles

3. Update skills:
   - `skills/cognee-search/SKILL.md` must call `mcp__cognee-mcp__recall` or `mcp__cognee-mcp__search`
   - `skills/cognee-remember/SKILL.md` must call `mcp__cognee-mcp__remember`, then `mcp__cognee-mcp__sync_read_model` when immediate recall visibility is needed
   - `skills/cognee-sync/SKILL.md` must call `mcp__cognee-mcp__sync_read_model`

4. Do not rewrite `hooks/hooks.json` yet unless the Rust binary already exposes hook-compatible commands for every hook event. The hooks are the magic and must only be migrated behind parity tests.

5. Validate:
   - `claude plugin validate <absolute path to integrations/claude-code>`
   - open `agents/cognee-recall.md` and confirm there are no `cognee-cli` calls
   - open the three skills and confirm explicit manual actions use MCP tools
```

## Current Decision

Manual agent actions are Rust MCP now:

- recall
- search
- inspect dataset
- inspect graph
- remember
- sync read model
- forget

Automatic lifecycle behavior stays in hooks until the Rust plugin migration is complete:

- SessionStart
- UserPromptSubmit context injection
- UserPromptSubmit prompt storage
- PostToolUse trace storage
- Stop answer storage
- PreCompact memory anchor
- SessionEnd / idle watcher sync
