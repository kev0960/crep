# crep-indexer – Architecture Guide

`crep-indexer` is the shared library that knows how to index a Git repository’s full history and answer code-search queries. The crate is organised into a few key modules.

## Index Entry Points (`index/indexer.rs`)
- `Indexer` chooses between Git-aware indexing (`GitIndexer`) and a filesystem-only fallback (`Index`).
- `IndexerConfig` captures root path, default main branch, and UTF-8 handling policy.
- `index_directory` scans non-Git trees via `WalkDir`, tokenises text files, and produces an `Index` (files, word bitmaps, word positions) mainly for local experiments.

## Git-Aware Index Build (`index/git_indexer.rs`)
- `GitIndexer::index_history` walks commits topologically using libgit2, collecting `commit_index_to_commit_id` and a reverse map.
- Per commit it either:
  - runs `index_tree` for the first snapshot; or
  - diffs against the previous tree (`index_diff`), translating each hunk (`GitDelta`) into add/delete operations.
- `Utf8FileChecker` (`index/check_binary.rs`) filters obvious binary blobs and honours the `ignore_utf8_error` flag.
- `FileDiffTracker` (`git/diff.rs`) maps live line ranges to the commit that last touched them, enabling precise removal bookkeeping.
- `add_new_lines` tokenises added lines (trigrams) and updates document state; it also records file-level membership in `word_to_file_id_ever_contained`.
- `delete_lines`/`delete_entire_file` consult `FileDiffTracker`, flatten removals into `WordKey`s, and mark tokens as inactive from the relevant commit onward.

## Per-File State (`index/document.rs`)
- `Document` keeps every observed token for a file.
  - `word_history`: `PriorityQueue<WordKey, CommitEndPriority>` recording when each token instance started and when it stopped existing.
  - `commit_inclutivity`: `RoaringBitmap` flagging commits where the token is present.
  - `all_words`: FST (`fst::Set`) built at `finalize` time for fast trigram lookups.
- `add_words`, `remove_words`, and `remove_document` update the queue and bitmaps in response to diff events.
- `finalize` extends trailing token lifetimes to the tip commit and materialises the per-file FST.

## Persisted Index (`index/git_index.rs`)
- `GitIndex::build` consumes a finished `GitIndexer`, packaging:
  - commit lookup tables,
  - `file_id_to_path`,
  - `file_id_to_document`,
  - global `word_to_file_id_ever_contained`, and
  - a workspace-wide `all_words` FST.
- `save`/`load` serialise via `bincode` using helpers under `util/serde/fst`.

## Search Components (`search/*`)
- `GitSearcher` (`search/git_searcher.rs`) performs both literal and regex queries over a `GitIndex`.
  - Token-level bitmap lookups and intersections determine candidate files.
  - `PermutationIterator` explores combinations when multiple occurrences may satisfy the query.
  - `find_matching_commit_histories_in_doc`/`_from_trigrams` collapse per-token `RoaringBitmap`s into commit ranges where all parts coexisted.
  - Results are emitted as `RawPerFileSearchResult` (file id + commit bitmap + query description).
- Regex handling (`search/regex_search.rs`) lowers a regex HIR into minimal trigram requirements (`RegexSearchCandidates`, `Trigram`) so the searcher can probe FSTs instead of full-text scanning.
- `GitSearchResultViewer` (`search/result_viewer.rs`) re-opens the Git repo, materialises file content from the earliest matching commit, and highlights hits using Aho-Corasick plus stored byte offsets.

## Tokenisation (`tokenizer.rs`)
- `Tokenizer::split_lines_to_tokens` emits deduplicated line numbers for words or trigrams, designed for indexing commits.
- `Tokenizer::split_to_words_with_col` retains byte offsets for presentation.
- `TokenizerMethod` selects between word mode and trigram mode; lines and positions are stored in hash maps/BTreeSets for deterministic output.

## Shared Utilities
- `util/bitmap::utils` exposes `union_bitmaps`/`intersect_bitmaps` helpers for `RoaringBitmap` operations.
- `util/serde` and `util/fst` provide serialization glue for FST-backed sets.

## Extending the Crate
- To add new per-token metadata, extend `WordIndex` in `document.rs` and update `GitIndex` serialization.
- To refine diff handling, update `FileDiffTracker` and ensure `delete_lines` still maps deletions back to the correct originating commits.
- To parallelize indexing, isolate tokenisation workloads while keeping ordered updates on shared `Document`s to avoid race conditions.
