# Crep

Crep (Code gREP) is a fast code search tool that indexes the full git history of
the repository.

It aims to provide the sub second response time for the moderately sized
repository.

## Development Plan

- [] Indexing server that incrementally updates the index
- [] Better format to store the index (e.g. compression)
- [] Ranking of the indexed documents (e.g. Show the recently modified docs first)
- [] SIMD based indexing
- [] Multi-threaded indexing
- [] Reducing the index memory footprint.

- [x] Single threaded indexing of the repository
- [x] Basic CLI tool for the code search
- [x] Basic browser based code search interface
