# Indexer

## GitIndexer

Represents the running indexer.

## GitIndex

Represents the generated result from the `GitIndexer`. It only contains the
constructs that are needed for the search.

Searcher reads the `GitIndex`.

## GitIndexSerialization

Serialized output of the `GitIndexer` for the future indexing (e.g. to continue
indexing when the repo gets updated).

GitIndexSerialization <---> GitIndexer <---> GitIndex
For storage For search
