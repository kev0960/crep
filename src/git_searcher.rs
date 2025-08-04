use crate::index::git_indexer::GitIndexer;

pub struct GitSearcher<'i> {
    index: &'i GitIndexer,
}

impl<'i> GitSearcher<'i> {
    pub fn new(index: &'i GitIndexer) -> Self {
        Self { index }
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let words = query.split_whitespace();
        let word_search_results = words
            .filter_map(|w| {
                let results = self.search_word(w);
                if results.is_empty() {
                    return None;
                }

                Some(results)
            })
            .collect();

        // Now select the document that contains
        let raw_search_results =
            Searcher::combine_search_result(&word_search_results);

        let mut search_result = vec![];
        for result in raw_search_results {
            let files = result
                .file_indexes
                .iter()
                .map(|index| {
                    (self.index.files[*index as usize].clone(), *index as usize)
                })
                .collect();

            search_result.push(SearchResult {
                files,
                words: result.words.iter().map(|s| s.to_string()).collect(),
            });
        }

        search_result
    }
}
