use std::num::NonZeroUsize;
use std::sync::Mutex;

use ahash::AHashMap;
use crep_indexer::search::git_searcher::Query;
use crep_indexer::search::git_searcher::RawPerFileSearchResult;
use crep_indexer::search::result::search_result::SearchResult;
use tracing::info;

pub struct SearchCache {
    results: Mutex<lru::LruCache<Query, CachedSearchResults>>,
}

impl SearchCache {
    pub fn new(cache_size: NonZeroUsize) -> Self {
        Self {
            results: Mutex::new(lru::LruCache::new(cache_size)),
        }
    }

    pub fn find<I>(
        &self,
        q: &Query,
        result_index: I,
    ) -> Option<Vec<CacheResult>>
    where
        I: IntoIterator<Item = usize>,
    {
        if let Some(c) = self.results.lock().unwrap().get(q) {
            info!("Match found for {:?}; raw size: {}", q, c.raw_result.len());

            let mut cache_result = vec![];
            for index in result_index {
                if index >= c.raw_result.len() {
                    cache_result.push(CacheResult::NotExist);
                } else if let Some(search_result) =
                    c.raw_index_to_search_result.get(&index)
                {
                    match search_result {
                        Some(search_result) => cache_result
                            .push(CacheResult::Hit(search_result.clone())),
                        None => cache_result.push(CacheResult::NotExist),
                    }
                } else {
                    cache_result.push(CacheResult::Miss(
                        c.raw_result.get(index).unwrap().clone(),
                    ));
                }
            }

            Some(cache_result)
        } else {
            None
        }
    }

    pub fn put_raw_result(
        &self,
        q: &Query,
        raw_result: Vec<RawPerFileSearchResult>,
    ) {
        info!(
            "Added {:?}'s raw results to the cache (num results: {})",
            q,
            raw_result.len()
        );

        let mut results = self.results.lock().unwrap();

        let entry = results
            .try_get_or_insert_mut(q.clone(), || -> Result<_, ()> {
                Ok(CachedSearchResults::default())
            })
            .unwrap();

        entry.raw_result = raw_result;
    }

    pub fn put_search_results<I>(&self, q: &Query, results: I)
    where
        I: IntoIterator<Item = (usize, Option<SearchResult>)>,
    {
        let mut iter = results.into_iter().peekable();

        // Early return if it's empty
        if iter.peek().is_none() {
            return;
        }

        let mut cache = self.results.lock().unwrap();
        let entry = cache
            .try_get_or_insert_mut(q.clone(), || -> Result<_, ()> {
                Ok(CachedSearchResults::default())
            })
            .unwrap();

        for (index, result) in iter {
            entry.raw_index_to_search_result.insert(index, result);
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
