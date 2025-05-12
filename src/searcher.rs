use crate::{index::Index, result_viewer::SearchResult};
use fst::{
    Automaton, IntoStreamer,
    automaton::{Levenshtein, Str},
};
use roaring::RoaringBitmap;

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
        let raw_search_results = Searcher::combine_search_result(&word_search_results);

        let mut search_result = vec![];
        for result in raw_search_results {
            let files = result
                .file_indexes
                .iter()
                .map(|index| self.index.files[*index as usize].clone())
                .collect();

            search_result.push(SearchResult {
                files,
                words: result.words.iter().map(|s| s.to_string()).collect(),
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

                selected_words.push(current_list[*perm_idx as usize].0.as_str());
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
        let matched_words = match word.len() {
            1 | 2 => {
                let matcher = Str::new(word).starts_with();
                self.index
                    .words
                    .search(matcher)
                    .into_stream()
                    .into_strs()
                    .unwrap()
            }
            _ => {
                let lev = Levenshtein::new(word, 1).unwrap();
                self.index
                    .words
                    .search(lev)
                    .into_stream()
                    .into_strs()
                    .unwrap()
            }
        };

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

struct PermutationIterator {
    limit: Vec<u32>,
    pub current: Option<Vec<u32>>,
}

impl PermutationIterator {
    fn new(limit: &[u32]) -> Self {
        let current = vec![0; limit.len()];

        if limit.iter().any(|v| v == &0) {
            panic!("Limit cannot contain zero");
        }

        Self {
            limit: limit.to_vec(),
            current: Some(current),
        }
    }
}

impl Iterator for PermutationIterator {
    type Item = Vec<u32>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current.clone()?;
        let next = self.current.as_mut().unwrap();

        for i in (0..next.len()).rev() {
            if next[i] < self.limit[i] - 1 {
                next[i] += 1;

                next[i + 1..].fill(0);

                return Some(current);
            }

            next[i] = 0;
        }

        self.current = None;
        Some(current)
    }
}

fn intersect_bitmaps(bitmaps: &[&RoaringBitmap]) -> Option<RoaringBitmap> {
    let mut iter = bitmaps.iter();
    let first = (*iter.next()?).clone();

    let result = iter.fold(first, |mut total, bitmap| {
        total &= *bitmap;
        total
    });

    Some(result)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_permutation() {
        let mut itr = PermutationIterator::new(&[1, 2, 3]);

        assert_eq!(itr.next(), Some(vec![0, 0, 0]));
        assert_eq!(itr.next(), Some(vec![0, 0, 1]));
        assert_eq!(itr.next(), Some(vec![0, 0, 2]));
        assert_eq!(itr.next(), Some(vec![0, 1, 0]));
        assert_eq!(itr.next(), Some(vec![0, 1, 1]));
        assert_eq!(itr.next(), Some(vec![0, 1, 2]));
        assert_eq!(itr.next(), None);
    }

    #[test]
    fn test_permutation_all_ones() {
        let mut itr = PermutationIterator::new(&[1, 1, 1]);

        assert_eq!(itr.next(), Some(vec![0, 0, 0]));
        assert_eq!(itr.next(), None);
    }

    #[test]
    fn test_permutation_binary() {
        let mut itr = PermutationIterator::new(&[2, 2, 2]);

        assert_eq!(itr.next(), Some(vec![0, 0, 0]));
        assert_eq!(itr.next(), Some(vec![0, 0, 1]));
        assert_eq!(itr.next(), Some(vec![0, 1, 0]));
        assert_eq!(itr.next(), Some(vec![0, 1, 1]));
        assert_eq!(itr.next(), Some(vec![1, 0, 0]));
        assert_eq!(itr.next(), Some(vec![1, 0, 1]));
        assert_eq!(itr.next(), Some(vec![1, 1, 0]));
        assert_eq!(itr.next(), Some(vec![1, 1, 1]));
        assert_eq!(itr.next(), None);
    }

    #[test]
    fn test_search_word() {
        let index = Index::new(
            /*files=*/ vec![],
            /*word_to_bitmap=*/
            HashMap::from_iter(vec![("a".to_owned(), RoaringBitmap::from_iter(vec![1]))]),
            /*file_to_word_pos=*/ HashMap::new(),
        );
        let searcher = Searcher::new(&index);

        assert_eq!(
            searcher.search_word("a"),
            vec![RoaringBitmap::from_iter(vec![1])]
        );
    }
}
