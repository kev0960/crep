# crep-indexer-cli – Usage & Flow

`crep-indexer-cli` wraps the shared library with both a ratatui-powered interface and a lightweight debug REPL.

## Binary Entry Point (`src/main.rs`)
- Parses arguments with `clap` (`Args`):
  - `--path/-p`: repository to open (required).
  - `--main-branch/-m`: overrides the branch used during history traversal (default `main`).
  - `--load-path/-l`: load a previously persisted `GitIndex` instead of rebuilding.
  - `--save-path/-s`: persist the freshly built index after indexing.
  - `--save-only`: exit immediately after saving; skip launching any UI.
  - `--debug`: run the text REPL (`raw_searcher`) instead of the TUI.
  - `--log`: initialise the file logger (`logger::init_file_logger`) at `LevelFilter::Debug`.
- `build_index` either deserialises (`GitIndex::load`) or builds a new index:
  1. Construct `GitIndexer` with `show_index_progress = true`, UTF-8 filtering enabled, branch hint, and libgit2 repository handle.
  2. Call `index_history`, build a `GitIndex`, optionally `save`, and return it.
- Unless `--save-only` is set, the index is handed to `Searcher::new` together with the repo path.
- Execution mode:
  - `--debug`: calls `raw_searcher::handle_query` for a blocking prompt.
  - default: bootstraps a ratatui terminal, clears the screen, and runs the event-driven `App`.

## TUI Workflow (`src/app.rs`)
- `App` holds shared state: an `Arc<Mutex<Searcher>>`, input buffer (`tui_input::Input`), UI/search channels, and a rolling log.
- Two background threads are spawned:
  1. UI thread publishes crossterm `Event`s into the app channel.
  2. Search worker receives `SearchMessage::SearchRequest(Query)` messages, executes `Searcher::handle_query`, and streams back `Message::SearchResults` + log entries.
- `State` toggles between `Control` (global shortcuts), `Input(QueryType)`, and `Terminate`. Users can switch regex vs literal entry and exit via ESC.
- Rendering is built with ratatui layouts (input, results, log panes). Results display the first/last commit where the query matched and preview highlighted lines from `SearchResult`.

## Searcher Bridge (`src/searcher.rs`)
- Maintains a libgit2 `Repository`, a borrowed `GitIndex`, and an owned `GitSearcher`.
- `Query` enum supports `Regex(String)` and `RawString(String)`; the latter feeds into `GitSearcher::search` (literal word split).
- `handle_query` converts each `RawPerFileSearchResult` into `FirstAndLastFound`, reopening blobs at specific commits and re-running `SearchResult::new` to validate matches.
- This logic powers both the TUI and debug REPL so behaviour stays consistent across modes.

## Debug REPL (`src/raw_searcher.rs`)
- Simple blocking loop used when `--debug` is passed.
- Interprets input prefixes: `q:` forces literal search, `r:` forces regex, otherwise treats input as regex.
- Prints first/last sightings with ANSI highlights using `get_highlighted_line` (mirrors the TUI formatting helpers).

## Logging (`src/logger.rs`)
- Optional file logger writing timestamped lines via a custom `log::Log` implementation guarded by a `Mutex<File>`.

## Extending the CLI
- Add new CLI flags in `Args` and plumb them into `build_index`/`App` to expose library features.
- UI enhancements should reuse `Searcher` to avoid duplicating Git access; prefer sending richer `SearchMessage`s rather than adding more shared state.
- Keep the search worker responsive by debouncing `SearchMessage`s like the existing `recv`/`try_recv` loop when adding new request types.
