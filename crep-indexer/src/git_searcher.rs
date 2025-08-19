use std::collections::HashMap;

use fst::{IntoStreamer, automaton::Levenshtein};
use roaring::RoaringBitmap;

use crate::{
    index::git_index::GitIndex, search::permutation::PermutationIterator,
    searcher::intersect_bitmaps,
};

pub struct GitSearcher<'i> {
    index: &'i GitIndex,
}

impl<'i> GitSearcher<'i> {
    pub fn new(index: &'i GitIndex) -> Self {
        Self { index }
    }

    pub fn search(&self, query: &str) -> Vec<RawPerFileSearchResult> {
        let words = query.split_whitespace();
        let documents_containing_each_word: Vec<Vec<(String, &RoaringBitmap)>> =
            words
                .filter_map(|w| {
                    let results = self.get_document_bitmap_containing_word(w);
                    if results.is_empty() {
                        return None;
                    }

                    println!("{w} ==> {results:?}");

                    Some(results)
                })
                .collect();

        let raw_result =
            self.find_overlapping_document(&documents_containing_each_word);
        println!("Raw: {raw_result:?}");

        let mut search_result = vec![];
        for result in &raw_result {
            let overlapping_commits =
                self.find_overlapping_commit_history(result);

            for (file_id, commit_overlap) in overlapping_commits {
                search_result.push(RawPerFileSearchResult {
                    file_id,
                    words: result
                        .words
                        .iter()
                        .map(|w| w.to_string())
                        .collect::<Vec<String>>(),
                    overapped_commits: commit_overlap,
                })
            }
        }

        search_result
    }

    pub fn get_document_bitmap_containing_word(
        &self,
        word: &str,
    ) -> Vec<(String, &RoaringBitmap)> {
        if let Some(bitmap) =
            self.index.word_to_file_id_ever_contained.get(word)
        {
            return vec![(word.to_owned(), bitmap)];
        }

        // Otherwise, try to find the closer words.
        let lev = Levenshtein::new(word, 1).unwrap();

        self.index
            .all_words
            .search(lev)
            .into_stream()
            .into_strs()
            .unwrap()
            .into_iter()
            .filter_map(|word| {
                if let Some(bitmap) =
                    self.index.word_to_file_id_ever_contained.get(&word)
                {
                    Some((word, bitmap))
                } else {
                    None
                }
            })
            .collect()
    }

    fn find_overlapping_document(
        &self,
        bitmaps: &'i Vec<Vec<(String, &RoaringBitmap)>>,
    ) -> Vec<RawSearchResult<'i>> {
        let permutations = PermutationIterator::new(
            &bitmaps.iter().map(|m| m.len() as u32).collect::<Vec<u32>>(),
        );

        let mut result = vec![];

        for permutation in permutations {
            println!("Permutations {permutation:?}");
            let mut selected_bitmaps = vec![];
            let mut selected_words = vec![];

            for (index, perm_idx) in permutation.iter().enumerate() {
                let current_list = bitmaps.get(index).unwrap();

                selected_words
                    .push(current_list[*perm_idx as usize].0.as_str());
                selected_bitmaps.push(current_list[*perm_idx as usize].1);
            }

            let intersection = intersect_bitmaps(selected_bitmaps.as_slice());
            if intersection.is_none() {
                continue;
            }

            result.push(RawSearchResult {
                file_ids: intersection.unwrap().iter().collect(),
                words: selected_words,
            })
        }

        result
    }

    fn find_overlapping_commit_history(
        &self,
        result: &RawSearchResult,
    ) -> HashMap<u32, RoaringBitmap> {
        let mut file_id_to_commit_history_bitmap = HashMap::new();

        for file_id in &result.file_ids {
            let file_id_uz = *file_id as usize;

            let document = self.index.file_id_to_document.get(&file_id_uz);
            if document.is_none() {
                continue;
            }

            let document = document.unwrap();
            let word_indexs = result
                .words
                .iter()
                .map(|word| document.words.get(*word))
                .collect::<Vec<_>>();

            if word_indexs.iter().any(|i| i.is_none()) {
                continue;
            }

            // Now overlap all commits.
            let commit_inclutivity_bitmaps = word_indexs
                .into_iter()
                .map(|index| &index.unwrap().commit_inclutivity)
                .collect::<Vec<_>>();

            println!("commit inclutivity {commit_inclutivity_bitmaps:?}");

            let commit_overlaps =
                intersect_bitmaps(&commit_inclutivity_bitmaps);

            if let Some(bitmap) = commit_overlaps
                && !bitmap.is_empty()
            {
                file_id_to_commit_history_bitmap.insert(*file_id, bitmap);
            }
        }

        file_id_to_commit_history_bitmap
    }
}

#[derive(Debug)]
struct RawSearchResult<'s> {
    pub words: Vec<&'s str>,
    pub file_ids: Vec<u32>,
}

#[derive(Debug)]
pub struct RawPerFileSearchResult {
    pub words: Vec<String>,
    pub file_id: u32,
    pub overapped_commits: RoaringBitmap,
}
