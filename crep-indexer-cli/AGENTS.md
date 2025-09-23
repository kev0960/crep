# crep-indexer-cli – Usage & Flow

`crep-indexer-cli` provides a command-line interface over the `crep-indexer` library.

## Binary Entry Point (`src/main.rs`)
- Parses arguments with `clap` (`Args`):
  - `--path/-p`: repository root to index or search.
  - `--main-branch/-m`: optional main branch name (defaults to `main`).
  - `--load-path/-l`: optional file to deserialize a previously saved `GitIndex`.
  - `--save-path/-s`: optional destination to persist the freshly built index.
- Control flow:
  1. If `--load-path` is provided, call `GitIndex::load`.
  2. Otherwise construct an `Indexer` with the supplied options, call `Indexer::index`, expect a `GitIndex`, and optionally `save` it.
  3. Hand the index to `handle_query`.

## Interactive Loop (`handle_query`)
- Wraps the `GitIndex` with a `GitSearcher` and a `GitSearchResultViewer`.
- Repeatedly prompts for a query (empty line exits).
- Executes `regex_search` (queries are treated as regexes) and streams highlighted results to stdout using the viewer.

## Integrating with Other Tools
- For scripted indexing, call the binary with `--save-path` to materialise an index artifact.
- For experiments, pipe commands into the REPL (`printf 'pattern
' | cargo run -p crep-indexer-cli -- --path ...`).
- The CLI is intentionally thin: add new flags here when exposing additional functionality from `crep-indexer` (e.g., switching between literal and regex modes, toggling result limits, etc.).
