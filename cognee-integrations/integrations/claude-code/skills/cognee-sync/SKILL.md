---
name: cognee-sync
description: Sync session cache entries into the permanent Cognee knowledge graph. Run this to make session memory searchable, or it runs automatically at session end.
---

# Sync Session to Permanent Graph

Bridge session cache entries into the permanent knowledge graph.

## Instructions

Call `mcp__cognee-mcp__sync_read_model`:

```json
{
  "dataset_name": "${COGNEE_PLUGIN_DATASET:-claude_sessions}"
}
```

## What this does

1. Refreshes the Rust read model from Cognee backend exports
2. Updates dataset, data, node, edge, and raw source handles
3. Makes new graph data visible to Rust `recall`, `search`, `inspect_dataset`, and `inspect_graph`

After this, session entries become searchable via `/cognee-memory:cognee-search`, and the graph knowledge is automatically included in session completion prompts.

## When to use

- Before searching for session content that hasn't been synced yet
- When you want to force an early sync without waiting for session end
- Automatic session-end bridge still runs through hooks; explicit agent sync uses Rust MCP
