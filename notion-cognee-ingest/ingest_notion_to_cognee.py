"""Ingest Notion content into the local Cognee repository."""

from __future__ import annotations

import argparse
import asyncio
import os
import sys
from pathlib import Path
from typing import Any

from dotenv import load_dotenv

ROOT = Path(__file__).resolve().parents[1]
COGNEE_REPO = ROOT / "cognee"
WORKDIR = Path(__file__).resolve().parent

sys.path.insert(0, str(COGNEE_REPO))
os.chdir(WORKDIR)

load_dotenv(COGNEE_REPO / ".env")
load_dotenv(WORKDIR / ".env")

import cognee  # noqa: E402
from notion import notion_databases, notion_pages  # noqa: E402


def _notion_api_key() -> str | None:
    return os.getenv("NOTION_API_KEY") or os.getenv("NOTION_TOKEN")


def _database_ids(values: list[str]) -> list[dict[str, str]] | None:
    if not values:
        return None

    parsed: list[dict[str, str]] = []
    for value in values:
        raw = value.strip()
        if not raw:
            continue
        if "=" in raw:
            name, database_id = raw.split("=", 1)
            parsed.append({"id": database_id.strip(), "use_name": name.strip()})
        else:
            parsed.append({"id": raw})
    return parsed or None


async def _add_source(source: Any, dataset: str, label: str, background: bool) -> None:
    result = await cognee.add(
        source,
        dataset_name=dataset,
        run_in_background=background,
    )
    print(f"added {label}: {result}")


async def run() -> None:
    parser = argparse.ArgumentParser(description="Load Notion into Cognee.")
    parser.add_argument("--dataset", default="notion_data")
    parser.add_argument("--database-id", action="append", default=[])
    parser.add_argument("--page-id", action="append", default=[])
    parser.add_argument("--skip-databases", action="store_true")
    parser.add_argument("--skip-pages", action="store_true")
    parser.add_argument("--cognify", action="store_true")
    parser.add_argument("--background", action="store_true")
    args = parser.parse_args()

    api_key = _notion_api_key()
    source_kwargs = {"api_key": api_key} if api_key else {}

    if args.skip_databases and args.skip_pages:
        raise SystemExit("Nothing selected. Remove one skip flag.")

    if not args.skip_databases:
        databases = notion_databases(
            database_ids=_database_ids(args.database_id),
            **source_kwargs,
        )
        await _add_source(databases, args.dataset, "Notion databases", args.background)

    if not args.skip_pages:
        pages = notion_pages(page_ids=args.page_id or None, **source_kwargs)
        await _add_source(pages, args.dataset, "Notion pages", args.background)

    if args.cognify:
        if not os.getenv("LLM_API_KEY") and not os.getenv("OPENAI_API_KEY"):
            raise SystemExit("Set LLM_API_KEY or OPENAI_API_KEY before --cognify.")
        result = await cognee.cognify(args.dataset, run_in_background=args.background)
        print(f"cognified {args.dataset}: {result}")


if __name__ == "__main__":
    asyncio.run(run())
