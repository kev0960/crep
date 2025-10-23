# crep-indexer – Architecture Guide

`crep-indexer` houses the shared logic for turning a Git repository into a history-aware search index and executing queries against it.

## Module Map
- `lib.rs`: re-exports the `git`, `index`, and `search` modules for consumers.
- `git/`: diff utilities, primarily `FileDiffTracker`, that map libgit2 deltas to per-line add/delete bookkeeping.
- `index/`:
  - `git_indexer.rs`: orchestrates repo traversal, diff processing, tokenisation, and per-file state updates.
  - `document.rs`: owns the per-file model (`Document`, `WordIndex`, `WordKey`) with roaring bitmaps tracking commit inclusions and history.
  - `git_index.rs`: materialises a persistent `GitIndex` from the finished `GitIndexer`; handles `save`/`load`.
  - `git_index_debug.rs`: opt-in instrumentation summarising diff timings while indexing.
  - `check_binary.rs`: UTF-8 gatekeeping and binary detection helpers.
- `search/`:
  - `git_searcher.rs`: entry point that satisfies literal and regex searches using trigram/FST lookups, bitmap intersections, and LRU caching for word-to-document maps.
  - `regex_search.rs`: lowers regex HIR to `RegexSearchCandidates` and `Trigram` sets the searcher can probe without full-text scanning.
  - `permutation.rs`: iterator that enumerates candidate combinations of commit histories per matched token.
  - `search_result.rs`: converts `Query` + file content into highlighted contexts for first/last commit discovery.
  - `result_viewer.rs` & `line_formatter.rs`: reopen the repo to render highlighted snippets for CLI/UI consumers.
- `util/`: bitmap helpers (`intersect_bitmaps`, `union_bitmaps`), FST serde glue, and other shared utilities required across modules.

## Index Build Flow (`GitIndexer`)
1. `GitIndexer::new` configures progress output, branch selection, and UTF-8 policy via `GitIndexerConfig`.
2. `index_history` walks commits in topological order using libgit2, maintaining commit id/index maps and calling `index_tree` for the first snapshot or `index_diff` thereafter.
3. Each diff run leverages `FileDiffTracker` to translate adds/deletes into trigram updates. Text content is tokenised with `split_lines_to_tokens` before being merged into `Document`s.
4. When a document mutates, its `WordIndex` entries update `word_history` and `commit_inclutivity`. Deletions extend prior lifetimes so historical presence is preserved.
5. After traversal, `Document::finalize` seals each file by extending trailing intervals and building an FST of observed tokens.

## Persisted Index (`GitIndex`)
`GitIndex::build` consumes the finished indexer, exposing:
- commit lookup tables (`commit_index_to_commit_id`, inverse map),
- `file_id_to_path` and `file_id_to_document` maps,
- `word_to_file_id_ever_contained` for trigram-to-file membership, and
- a workspace-wide `all_words` FST for approximate lookups.
Indices can be serialised with `save`/`load` (bincode) using helpers from `util::serde`.

## Search Stack
- Literal queries: `GitSearcher::search` splits on whitespace, resolves each term to file bitmaps (with caching for short tokens), intersects per-file histories, and emits `RawPerFileSearchResult` entries containing overlapping commit ranges.
- Regex queries: `GitSearcher::regex_search` converts the regex HIR to trigram candidates (`RegexSearchCandidates`), prunes impossible paths, then intersects doc-level bitmaps for any candidate set of trigrams.
- Commit range reconciliation: `PermutationIterator` iterates candidate word histories, and `find_matching_commit_histories_in_doc*` intersect commit bitmaps (including `doc_modified_commits`) to identify uninterrupted spans where every token coexisted.

## Result Rendering
Consumers can use:
- `GitSearchResultViewer`: loads blobs for first matching commits, highlights matched ranges via `line_formatter::highlight_line_by_positions`, and prints truncated contexts.
- `SearchResult`: lightweight struct used by the CLI to obtain first/last sightings per file alongside per-line highlights.

## Extending the Crate
- New per-token metadata lives beside `WordIndex` and must be threaded through `GitIndexer`, `Document`, and `GitIndex` persistence.
- When altering diff handling, ensure `FileDiffTracker` stays consistent with commit lifetimes and `Document::remove_words` continues to seal gaps.
- The bitmap helpers assume dense roaring bitmaps; keep intersections cheap by minimising clone-heavy operations when adding new search paths.
