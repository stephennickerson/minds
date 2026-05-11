# Notion to Cognee

This folder is the local ingestion bridge for loading Notion into Cognee.

## Required secrets

Set one Notion token and one LLM key before building the graph:

```powershell
$env:NOTION_API_KEY = "secret_..."
$env:LLM_API_KEY = "sk-..."
```

The Notion integration must be shared with the pages and databases you want Cognee to see.

## Load Notion

Add everything visible to the Notion integration:

```powershell
..\\cognee\\.venv\\Scripts\\python.exe .\\ingest_notion_to_cognee.py --dataset notion_data
```

Build the graph after the raw load:

```powershell
..\\cognee\\.venv\\Scripts\\python.exe .\\ingest_notion_to_cognee.py --dataset notion_data --cognify
```

Limit scope when needed:

```powershell
..\\cognee\\.venv\\Scripts\\python.exe .\\ingest_notion_to_cognee.py --dataset notion_data --database-id "Projects=38163b40-3538-4b0d-9e2b-bf9e5f0797cc" --skip-pages --cognify
```

## Keep It Updated

Rerun the same command on a schedule. The source reads current Notion state, Cognee ingests incrementally, and `--cognify` refreshes the graph.
