use std::num::NonZeroUsize;

use ahash::AHashMap;
use crep_indexer::search::git_searcher::RawPerFileSearchResult;
use crep_indexer::search::result::search_result::SearchResult;

use crate::search::search::Query;

pub struct SearchCache {
    results: lru::LruCache<Query, CachedSearchResults>,
}

impl SearchCache {
    pub fn new(cache_size: NonZeroUsize) -> Self {
        Self {
            results: lru::LruCache::new(cache_size),
        }
    }

    pub fn find(
        &mut self,
        q: &Query,
        result_index: &[usize],
    ) -> Option<Vec<CacheResult>> {
        if let Some(c) = self.results.get(q) {
            let mut cache_result = vec![];
            for index in result_index {
                if *index >= c.raw_result.len() {
                    cache_result.push(CacheResult::NotExist);
                } else if let Some(search_result) =
                    c.raw_index_to_search_result.get(index)
                {
                    match search_result {
                        Some(search_result) => cache_result
                            .push(CacheResult::Hit(search_result.clone())),
                        None => cache_result.push(CacheResult::NotExist),
                    }
                } else {
                    cache_result.push(CacheResult::Miss(
                        c.raw_result.get(*index).unwrap().clone(),
                    ));
                }
            }

            Some(cache_result)
        } else {
            None
        }
    }

    pub fn put_raw_result(
        &mut self,
        q: &Query,
        raw_result: Vec<RawPerFileSearchResult>,
    ) {
        let entry = self
            .results
            .try_get_or_insert_mut(q.clone(), || -> Result<_, ()> {
                Ok(CachedSearchResults::default())
            })
            .unwrap();

        entry.raw_result = raw_result;
    }

    pub fn put_search_results(
        &mut self,
        q: &Query,
        results: &[(usize, Option<SearchResult>)],
    ) {
        let entry = self
            .results
            .try_get_or_insert_mut(q.clone(), || -> Result<_, ()> {
                Ok(CachedSearchResults::default())
            })
            .unwrap();

        for (index, result) in results {
            entry
                .raw_index_to_search_result
                .insert(*index, result.clone());
        }
    }
}

#[derive(Default)]
struct CachedSearchResults {
    raw_result: Vec<RawPerFileSearchResult>,
    raw_index_to_search_result: AHashMap<usize, Option<SearchResult>>,
}

pub enum CacheResult {
    Hit(SearchResult),
    Miss(RawPerFileSearchResult),
    NotExist,
}
