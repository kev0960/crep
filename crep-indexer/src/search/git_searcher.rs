use std::collections::HashSet;

use fst::{IntoStreamer, Set};
use lru::LruCache;
use regex_automata::dense;
use roaring::RoaringBitmap;

use crate::{
    index::{document::Document, git_index::GitIndex, git_indexer::FileId},
    search::permutation::PermutationIterator,
    tokenizer::{Tokenizer, TokenizerMethod},
    util::bitmap::utils::{intersect_bitmaps, union_bitmaps},
};

pub struct GitSearcher<'i> {
    index: &'i GitIndex,
    word_to_docs_cache: LruCache<
        String,
        Option<(String, RoaringBitmap, /*should_split_trigram=*/ bool)>,
    >,
}

impl<'i> GitSearcher<'i> {
    pub fn new(index: &'i GitIndex) -> Self {
        Self {
            index,
            word_to_docs_cache: LruCache::new(
                std::num::NonZeroUsize::new(4096).unwrap(),
            ),
        }
    }

    pub fn search(&mut self, query: &str) -> Vec<RawPerFileSearchResult> {
        let words = query.split_whitespace();
        let mut documents_containing_each_word: Vec<(
            String,
            RoaringBitmap,
            /*should_split_trigram=*/ bool,
        )> = vec![];

        for word in words {
            let results = self.get_document_bitmap_containing_word(word);
            if results.is_none() {
                return vec![];
            }

            println!("{word} ==> {results:?}");
            documents_containing_each_word.push(results.unwrap());
        }

        self.find_overlapping_document(&documents_containing_each_word)
    }

    pub fn get_document_bitmap_containing_word(
        &mut self,
        word: &str,
    ) -> Option<(String, RoaringBitmap, /*should_split_trigram=*/ bool)> {
        if let Some(docs) = self.word_to_docs_cache.get(word) {
            return docs.clone();
        }

        let w = word.to_owned();

        if word.len() <= 2 {
            let docs =
                find_all_words_containing_key(word, &self.index.all_words)
                    .into_iter()
                    .filter_map(|word| {
                        self.index.word_to_file_id_ever_contained.get(&word)
                    })
                    .collect::<Vec<_>>();

            let overlaps = union_bitmaps(&docs).unwrap();

            self.word_to_docs_cache
                .put(w.clone(), Some((w.clone(), overlaps.clone(), false)));

            return Some((w, overlaps, false));
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
            return None;
        }

        self.word_to_docs_cache.put(
            w.clone(),
            Some((
                w.clone(),
                intersect_bitmaps(bitmaps.as_slice()).unwrap(),
                true,
            )),
        );

        Some((w, intersect_bitmaps(bitmaps.as_slice()).unwrap(), true))
    }

    fn find_overlapping_document(
        &self,
        bitmaps: &'i Vec<(String, RoaringBitmap, bool)>,
    ) -> Vec<RawPerFileSearchResult> {
        let mut result = vec![];

        let docs_for_each_word =
            bitmaps.iter().map(|b| &b.1).collect::<Vec<_>>();
        let intersected_docs = intersect_bitmaps(&docs_for_each_word).unwrap();

        for file_id in intersected_docs {
            let document =
                self.index.file_id_to_document.get(&(file_id as FileId));

            if document.is_none() {
                continue;
            }

            let document = document.unwrap();
            let commit_histories_per_word = bitmaps
                .iter()
                .map(|(word, _, should_split_trigram)| {
                    self.find_matching_commit_histories_in_doc(
                        document,
                        word,
                        *should_split_trigram,
                    )
                })
                .collect::<Vec<_>>();

            println!("commit histories {commit_histories_per_word:?}");
            let permutations = PermutationIterator::new(
                &commit_histories_per_word
                    .iter()
                    .map(|m| m.len() as u32)
                    .collect::<Vec<u32>>(),
            );

            for permutation in permutations {
                println!("Permutations {permutation:?}");

                let mut selected_words = vec![];
                let mut selected_bitmaps = vec![];

                for (index, perm_idx) in permutation.iter().enumerate() {
                    selected_words.push(
                        commit_histories_per_word[index][*perm_idx as usize]
                            .0
                            .clone(),
                    );

                    selected_bitmaps.push(
                        &commit_histories_per_word[index][*perm_idx as usize].1,
                    );
                }

                let overlapped_commits =
                    intersect_bitmaps(&selected_bitmaps).unwrap();

                if overlapped_commits.is_empty() {
                    continue;
                }

                result.push(RawPerFileSearchResult {
                    words: selected_words,
                    file_id,
                    overlapped_commits,
                });
            }
        }

        result
    }

    fn find_matching_commit_histories_in_doc(
        &self,
        doc: &Document,
        word: &str,
        should_split_trigram: bool,
    ) -> Vec<(String, RoaringBitmap)> {
        if !should_split_trigram {
            let words_to_find = find_all_words_containing_key(
                word,
                doc.all_words.as_ref().unwrap(),
            );
            return words_to_find
                .into_iter()
                .filter_map(|w| {
                    doc.words
                        .get(&w)
                        .map(|index| (w, index.commit_inclutivity.clone()))
                })
                .collect::<Vec<_>>();
        }

        let lines = vec![word.to_owned()];
        let trigrams = Tokenizer::split_lines_to_tokens(
            &lines,
            0,
            TokenizerMethod::Trigram,
        );

        let mut commit_bitmaps = vec![];
        for w in trigrams.total_words {
            if w.len() < 3 {
                continue;
            }

            if let Some(b) = doc.words.get(w) {
                commit_bitmaps.push(&b.commit_inclutivity);
            } else {
                return vec![];
            }
        }

        vec![(word.to_owned(), intersect_bitmaps(&commit_bitmaps).unwrap())]
    }
}

#[derive(Debug)]
pub struct RawPerFileSearchResult {
    pub words: Vec<String>,
    pub file_id: u32,
    pub overlapped_commits: RoaringBitmap,
}

fn find_all_words_containing_key(
    key: &str,
    all_words: &Set<Vec<u8>>,
) -> Vec<String> {
    let escaped_word = regex::escape(key);
    let pattern = match key.len() {
        2 => format!("{escaped_word}.|.{escaped_word}"),
        1 => format!("{escaped_word}..|.{escaped_word}.|..{escaped_word}"),
        _ => panic!("Should not happen {key}"),
    };

    let dfa = dense::Builder::new().build(&pattern).unwrap();
    all_words.search(dfa).into_stream().into_strs().unwrap()
}
