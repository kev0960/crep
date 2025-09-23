# Workspace Overview

This workspace provides a Git history–aware code search engine built as a Rust workspace.

## Crates
- `crep-indexer`: library crate that builds/loads search indices and evaluates queries. See `crep-indexer/AGENTS.md` for details.
- `crep-indexer-cli`: CLI wrapper that runs indexing jobs, persists indices, and offers an interactive search REPL. See `crep-indexer-cli/AGENTS.md`.
- `crep-server`: Axum/Leptos application scaffold intended to host a web UI over the shared library. See `crep-server/AGENTS.md`.

## Typical Usage
1. Use `crep-indexer-cli` to build or load a `GitIndex` for a repository.
2. Run queries through the CLI (or integrate the same library logic into the server to provide an API/UI).

Consult each crate’s `AGENTS.md` for module internals and extension guidelines.
