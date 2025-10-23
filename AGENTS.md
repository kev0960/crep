# Workspace Guide

This workspace hosts a Git history search engine implemented as a Rust workspace with shared indexing/search logic and multiple front ends.

## Crates at a Glance
- `crep-indexer`: builds history-aware indices, persists them, and exposes search and result-viewer primitives. Updated architecture notes live in `crep-indexer/AGENTS.md`.
- `crep-indexer-cli`: terminal UI and raw REPL over the library. Handles index build/load, persistence, and interactive search. See `crep-indexer-cli/AGENTS.md`.
- `crep-server`: Axum + Leptos scaffold for a future web front end. Current wiring and extension advice is in `crep-server/AGENTS.md`.

## Typical Flow
1. Use `crep-indexer-cli` with `--path <repo>` to build a `GitIndex`, optionally `--save-path` to persist it, or `--load-path` to reuse an existing index.
2. Search from the ratatui UI (default) or `--debug` raw prompt to validate queries.
3. Integrate the same indexing/search crates into other applications (e.g., `crep-server`) by loading the saved `GitIndex` and constructing a `GitSearcher` per request.

Refer to the per-crate guides before extending modules or adding new entry points.
