use std::collections::{HashMap, HashSet};

use fst::IntoStreamer;
use regex_automata::dense;
use roaring::RoaringBitmap;

use crate::{
    index::{git_index::GitIndex, git_indexer::FileId},
    search::permutation::PermutationIterator,
    tokenizer::{Tokenizer, TokenizerMethod},
    util::bitmap::intersect::intersect_bitmaps,
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
        let documents_containing_each_word: Vec<
            Vec<(String, RoaringBitmap, /*should_split_trigram=*/ bool)>,
        > = words
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
                        .map(|(w, _)| w.to_string())
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
    ) -> Vec<(String, RoaringBitmap, /*should_split_trigram=*/ bool)> {
        let w = word.to_owned();

        if word.len() <= 2 {
            // Then no need to split the word into trigrams.
            let escaped_word = regex::escape(&word);
            let pattern = format!("{}.|.{}", escaped_word, escaped_word);

            let dfa = dense::Builder::new().build(&pattern).unwrap();
            return self
                .index
                .all_words
                .search(dfa)
                .into_stream()
                .into_strs()
                .unwrap()
                .into_iter()
                .filter_map(|word| {
                    if let Some(bitmap) =
                        self.index.word_to_file_id_ever_contained.get(&word)
                    {
                        Some((word, bitmap.clone(), false))
                    } else {
                        None
                    }
                })
                .collect();
        }

        let lines = vec![w.clone()];
        let trigrams = Tokenizer::split_lines_to_tokens(
            &lines,
            0,
            TokenizerMethod::Trigram,
        );

        let tokens = trigrams.total_words;
        let trigrams: HashSet<&str> =
            tokens.into_iter().filter(|w| w.len() >= 3).collect();

        // Find the document that contains all matching tokens.
        let bitmaps = trigrams
            .iter()
            .filter_map(|t| self.index.word_to_file_id_ever_contained.get(*t))
            .collect::<Vec<_>>();

        if bitmaps.len() != trigrams.len() {
            return vec![];
        }

        vec![(w, intersect_bitmaps(bitmaps.as_slice()).unwrap(), true)]
    }

    fn find_overlapping_document(
        &self,
        bitmaps: &'i Vec<Vec<(String, RoaringBitmap, bool)>>,
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

                selected_words.push((
                    current_list[*perm_idx as usize].0.as_str(),
                    current_list[*perm_idx as usize].2,
                ));
                selected_bitmaps.push(&current_list[*perm_idx as usize].1);
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
            let file_id_uz = *file_id as FileId;

            let commit_inclutivity_bitmaps = result
                .words
                .iter()
                .map(|(w, should_spolit_trigram)| {
                    return self.find_overlapping_commit_in_document(
                        file_id_uz,
                        w,
                        *should_spolit_trigram,
                    );
                })
                .collect::<Vec<_>>();

            if commit_inclutivity_bitmaps.contains(&None) {
                continue;
            }
            let commit_inclutivity_bitmaps = commit_inclutivity_bitmaps
                .into_iter()
                .map(|b| b.unwrap())
                .collect::<Vec<_>>();

            let commit_overlaps =
                intersect(commit_inclutivity_bitmaps.as_slice());

            if let Some(bitmap) = commit_overlaps
                && !bitmap.is_empty()
            {
                file_id_to_commit_history_bitmap.insert(*file_id, bitmap);
            }
        }

        file_id_to_commit_history_bitmap
    }

    fn find_overlapping_commit_in_document(
        &self,
        file_id: FileId,
        word: &str,
        split_trigram: bool,
    ) -> Option<RoaringBitmap> {
        let document = self.index.file_id_to_document.get(&file_id);

        let document = document?;
        if !split_trigram {
            if let Some(index) = document.words.get(word) {
                return Some(index.commit_inclutivity.clone());
            } else {
                return None;
            }
        }

        let word = [word.to_owned()];
        let trigrams = Tokenizer::split_lines_to_tokens(
            &word,
            0,
            TokenizerMethod::Trigram,
        )
        .total_words
        .into_iter()
        .filter(|w| w.len() >= 3)
        .collect::<Vec<&str>>();

        let bitmaps = trigrams
            .iter()
            .map(|t| &document.words.get(*t).unwrap().commit_inclutivity)
            .collect::<Vec<_>>();

        intersect_bitmaps(&bitmaps)
    }
}

#[derive(Debug)]
struct RawSearchResult<'s> {
    pub words: Vec<(&'s str, /*should_split_trigram=*/ bool)>,
    pub file_ids: Vec<u32>,
}

#[derive(Debug)]
pub struct RawPerFileSearchResult {
    pub words: Vec<String>,
    pub file_id: u32,
    pub overapped_commits: RoaringBitmap,
}

fn intersect(bitmaps: &[RoaringBitmap]) -> Option<RoaringBitmap> {
    let mut iter = bitmaps.iter();
    let first = (*iter.next()?).clone();

    let result = iter.fold(first, |mut total, bitmap| {
        total &= bitmap;
        total
    });

    Some(result)
}
