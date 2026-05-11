---
name: cognee-remember
description: Store data permanently in the Cognee knowledge graph. Accepts a data category (user, project, or agent) to tag the data with the correct node_set for filtered retrieval.
---

# Cognee Permanent Memory Storage

Store data permanently in the Cognee knowledge graph through the Rust MCP server with category tagging.

## Data categories

Cognee organizes knowledge into three categories via `node_set` tagging:

| Category | Node set | What belongs here |
|----------|----------|-------------------|
| **user** | `user_context` | User preferences, corrections, personal facts, communication style |
| **project** | `project_docs` | Repository docs, code context, architecture decisions, company data |
| **agent** | `agent_actions` | Tool call logs, reasoning traces, generated artifacts (auto-captured by hooks) |

## Instructions

Determine the category from the user's intent, then call `mcp__cognee-mcp__remember`.

**User data** (preferences, corrections, personal context):
```json
{
  "data": ["$ARGUMENTS"],
  "dataset_name": "${COGNEE_PLUGIN_DATASET:-claude_sessions}",
  "node_set": ["user_context"],
  "run_in_background": false
}
```

**Project data** (docs, code, company knowledge):
```json
{
  "data": ["$ARGUMENTS"],
  "dataset_name": "${COGNEE_PLUGIN_DATASET:-claude_sessions}",
  "node_set": ["project_docs"],
  "run_in_background": false
}
```

**Agent data** (explicit agent notes — routine tool logs are automatic):
```json
{
  "data": ["$ARGUMENTS"],
  "dataset_name": "${COGNEE_PLUGIN_DATASET:-claude_sessions}",
  "node_set": ["agent_actions"],
  "run_in_background": false
}
```

If the category is unclear, default to **project**.

After `remember`, call `mcp__cognee-mcp__sync_read_model` for the same dataset if you need the new data visible to fast recall immediately.

## When to use

- User says "remember this" or "save this" → category **user**
- User says "remember this about the project/codebase" → category **project**
- You want to persist your own findings or conclusions → category **agent**
- NOT for routine tool call logging (that's automatic via hooks with `agent_actions` tagging)

## Category routing guide

| Signal | Category |
|--------|----------|
| "remember my preference for..." | user |
| "I always want..." / "I prefer..." | user |
| "remember this about the codebase" | project |
| "save these docs" / "index this file" | project |
| "note that this API works like..." | project |
| "remember what we discovered" | agent |
| Routine tool calls | agent (automatic, no action needed) |
