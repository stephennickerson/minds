---
name: cognee-search
description: Search Cognee memory. Session memory is automatically searched on every prompt via hooks. Use this skill explicitly for permanent knowledge graph search, filtered category search, or when you need more results than the automatic lookup provides.
---

# Cognee Memory Search

Search Cognee memory through the Rust MCP server, optionally filtered by dataset/category handles.

## Automatic session search

Session memory is searched **automatically on every user prompt** via the `UserPromptSubmit` hook. You do not need to run this skill to access current-session context.

## Data categories

Knowledge is organized into three categories via `node_set`:

| Category | Node set | Contains |
|----------|----------|----------|
| **user** | `user_context` | User preferences, corrections, personal facts |
| **project** | `project_docs` | Repository docs, code context, architecture decisions |
| **agent** | `agent_actions` | Tool call logs, reasoning traces, generated artifacts |

## Instructions

Call `mcp__cognee-mcp__recall`:

```json
{
  "query": "$ARGUMENTS",
  "datasets": ["${COGNEE_PLUGIN_DATASET:-claude_sessions}"],
  "top_k": 10,
  "llm_presummary": false
}
```

Call `mcp__cognee-mcp__search` when direct ranked evidence is better than a recall packet. Call `mcp__cognee-mcp__inspect_dataset` first when the dataset handle is unknown.

## Understanding results

The Rust MCP returns markdown packets with ranked evidence, source handles, graph relationships, coverage notes, and Navigate Next. Preserve handles in your answer.

## Decision table

| Signal | Action |
|--------|--------|
| Need current session context | Already automatic, no action needed |
| "what are my preferences" | Call `recall` and use returned source handles |
| "what does the codebase do" | Call `recall`; use `inspect_dataset` if handles are missing |
| "what did we do last time" | Call `recall` with `top_k: 10` |
| User explicitly says "search cognee" | Call `search` or `recall` through MCP |
| Auto context insufficient | Call `recall` with a sharper query |
