use fst::{
    IntoStreamer,
    automaton::{Levenshtein, Str},
};
use roaring::RoaringBitmap;

use crate::{
    index::index::Index, result_viewer::SearchResult,
    search::permutation::PermutationIterator,
    util::bitmap::utils::intersect_bitmaps,
};

pub struct Searcher<'i> {
    index: &'i Index,
}

impl<'i> Searcher<'i> {
    pub fn new(index: &'i Index) -> Self {
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
                git_commit_range: None,
            });
        }

        search_result
    }

    fn combine_search_result(
        word_search_results: &'i Vec<Vec<(String, RoaringBitmap)>>,
    ) -> Vec<RawSearchResult<'i>> {
        let permutations = PermutationIterator::new(
            &word_search_results
                .iter()
                .map(|m| m.len() as u32)
                .collect::<Vec<u32>>(),
        );

        let mut search_result = vec![];

        for permutation in permutations {
            let mut selected_bitmaps = vec![];
            let mut selected_words = vec![];

            for (index, perm_idx) in permutation.iter().enumerate() {
                let current_list = &word_search_results[index];

                selected_words
                    .push(current_list[*perm_idx as usize].0.as_str());
                selected_bitmaps.push(&current_list[*perm_idx as usize].1);
            }

            let intersection = intersect_bitmaps(selected_bitmaps.as_slice());
            if intersection.is_none() {
                continue;
            }

            search_result.push(RawSearchResult {
                file_indexes: intersection.unwrap().iter().collect(),
                words: selected_words,
            });
        }

        search_result
    }

    fn search_word(&self, word: &str) -> Vec<(String, RoaringBitmap)> {
        let matcher = Str::new(word);
        let mut matched_words = self
            .index
            .words
            .search(matcher)
            .into_stream()
            .into_strs()
            .unwrap();

        if matched_words.is_empty() {
            let lev = Levenshtein::new(word, 1).unwrap();
            matched_words = self
                .index
                .words
                .search(lev)
                .into_stream()
                .into_strs()
                .unwrap();
        }

        // Now find the files that contains the matched words.

        matched_words
            .into_iter()
            .filter_map(|word| {
                let bitmap = self.index.word_to_bitmap.get(&word)?;

                if bitmap.is_empty() {
                    return None;
                }

                Some((word.to_string(), bitmap.clone()))
            })
            .collect()
    }
}

struct RawSearchResult<'s> {
    pub words: Vec<&'s str>,
    pub file_indexes: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_search_single_letter() {
        let index = Index::new(
            /*files=*/ vec![],
            /*word_to_bitmap=*/
            HashMap::from_iter(vec![
                ("a".to_owned(), RoaringBitmap::from_iter(vec![1])),
                ("b".to_owned(), RoaringBitmap::from_iter(vec![1])),
                ("ab".to_owned(), RoaringBitmap::from_iter(vec![1])),
            ]),
            /*file_to_word_pos=*/ HashMap::new(),
        );
        let searcher = Searcher::new(&index);

        assert_eq!(
            searcher.search_word("a"),
            vec![("a".to_owned(), RoaringBitmap::from_iter(vec![1]))]
        );
    }

    #[test]
    fn test_search_multi_letter() {
        let index = Index::new(
            /*files=*/ vec![],
            /*word_to_bitmap=*/
            HashMap::from_iter(vec![
                ("foo".to_owned(), RoaringBitmap::from_iter(vec![1])),
                ("foob".to_owned(), RoaringBitmap::from_iter(vec![2])),
                ("boo".to_owned(), RoaringBitmap::from_iter(vec![3])),
                ("far".to_owned(), RoaringBitmap::from_iter(vec![4])),
            ]),
            /*file_to_word_pos=*/ HashMap::new(),
        );
        let searcher = Searcher::new(&index);

        assert_eq!(
            searcher.search_word("foo"),
            vec![("foo".to_owned(), RoaringBitmap::from_iter(vec![1])),]
        );
    }

    #[test]
    fn test_search_with_levenshtein() {
        let index = Index::new(
            /*files=*/ vec![],
            /*word_to_bitmap=*/
            HashMap::from_iter(vec![
                ("foooooo".to_owned(), RoaringBitmap::from_iter(vec![1])),
                ("boooooo".to_owned(), RoaringBitmap::from_iter(vec![2])),
                ("booooor".to_owned(), RoaringBitmap::from_iter(vec![3])),
                ("faooooo".to_owned(), RoaringBitmap::from_iter(vec![4])),
            ]),
            /*file_to_word_pos=*/ HashMap::new(),
        );
        let searcher = Searcher::new(&index);

        assert_eq!(
            searcher.search_word("boooook"),
            vec![
                ("boooooo".to_owned(), RoaringBitmap::from_iter(vec![2])),
                ("booooor".to_owned(), RoaringBitmap::from_iter(vec![3])),
            ]
        );
    }
}
