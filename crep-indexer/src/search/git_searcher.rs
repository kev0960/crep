use std::collections::HashSet;
use std::fmt::Write;

use anyhow::anyhow;
use fst::{IntoStreamer, Set};
use lru::LruCache;
use regex_automata::dense;
use regex_syntax::hir::{Class, Hir, HirKind};
use roaring::RoaringBitmap;

use crate::{
    index::{document::Document, git_index::GitIndex, git_indexer::FileId},
    search::permutation::PermutationIterator,
    tokenizer::{Tokenizer, TokenizerMethod},
    util::bitmap::utils::{intersect_bitmaps, union_bitmaps},
};

use super::regex_search::{RegexSearchCandidates, SearchPartTrigram, Trigram};

pub struct GitSearcher<'i> {
    index: &'i GitIndex,
    word_to_docs_cache: LruCache<String, Option<(String, RoaringBitmap)>>,
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
        let mut documents_containing_each_word: Vec<(String, RoaringBitmap)> =
            vec![];

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

    pub fn regex_search(
        &mut self,
        query: &str,
    ) -> Result<Vec<RawPerFileSearchResult>, String> {
        let hir = regex_syntax::parse(query);

        if hir.is_err() {
            return Err(format!(
                "Failed to parse regex {query}. Error: {:?}",
                hir.err()
            ));
        }

        let hir = hir.unwrap();
        println!("Hir : {hir:?}");

        let candidates = self
            .build_candidates_from_hir(&hir)
            .map_err(|e| format!("Error building candidates {e:?}"))?;

        let mut search_result = vec![];
        for cand in candidates.candidates {
            let trigrams = cand.trigrams;

            if trigrams.is_empty() {
                continue;
            }

            // Trigrams to look for and the list of documents.
            for doc_id in cand.docs_to_check {
                let doc =
                    self.index.file_id_to_document.get(&(doc_id as FileId));

                if let Some(doc) = doc {
                    let commit_history = self
                        .find_matching_commit_histories_in_doc_from_trigrams(
                            doc, &trigrams,
                        );

                    if commit_history.as_ref().is_some_and(|c| !c.is_empty()) {
                        search_result.push(RawPerFileSearchResult {
                            query: Query::Regex(query.to_owned()),
                            file_id: doc_id,
                            overlapped_commits: commit_history.unwrap(),
                        });
                    }
                }
            }
        }

        Ok(search_result)
    }

    fn build_candidates_from_hir(
        &mut self,
        hir: &Hir,
    ) -> anyhow::Result<RegexSearchCandidates> {
        match hir.kind() {
            HirKind::Empty => Ok(RegexSearchCandidates { candidates: vec![] }),
            HirKind::Literal(literal) => {
                let literal = std::str::from_utf8(&literal.0)?;
                if let Some((_, docs)) =
                    self.get_document_bitmap_containing_word(literal)
                {
                    let trigrams = Trigram::from_long_string(literal);
                    Ok(RegexSearchCandidates {
                        candidates: vec![SearchPartTrigram {
                            trigrams,
                            docs_to_check: docs,
                        }],
                    })
                } else {
                    Ok(RegexSearchCandidates { candidates: vec![] })
                }
            }
            HirKind::Repetition(repetition) => {
                let candidate =
                    self.build_candidates_from_hir(&repetition.sub)?;
                Ok(RegexSearchCandidates::repeat(
                    &candidate,
                    repetition.min,
                    repetition.max,
                ))
            }
            HirKind::Concat(hirs) => {
                let candidates: Vec<RegexSearchCandidates> = hirs
                    .iter()
                    .map(|hir| self.build_candidates_from_hir(hir))
                    .collect::<Result<_, _>>()?;

                Ok(RegexSearchCandidates::concat(&candidates))
            }
            HirKind::Alternation(hirs) => {
                let candidates: Vec<RegexSearchCandidates> = hirs
                    .iter()
                    .map(|hir| self.build_candidates_from_hir(hir))
                    .collect::<Result<_, _>>()?;

                Ok(RegexSearchCandidates::alternation(&candidates))
            }
            HirKind::Class(class) => {
                let pattern = match class {
                    Class::Unicode(unicode) => {
                        let mut pattern = String::from(".*[");
                        for range in unicode.ranges() {
                            write!(
                                &mut pattern,
                                r"\u{{{:X}}}-\u{{{:X}}}",
                                range.start() as u32,
                                range.end() as u32
                            )?;
                        }

                        pattern.push_str("].*");
                        pattern
                    }
                    Class::Bytes(bytes) => {
                        let mut pattern = String::from(".*[");
                        for range in bytes.ranges() {
                            write!(
                                &mut pattern,
                                r"\x{:02X}-\x{:02X}",
                                range.start(),
                                range.end()
                            )?;
                        }

                        pattern.push_str("].*");
                        pattern
                    }
                };

                let dfa = dense::Builder::new().build(&pattern)?;
                let all_matched_trigrams = self
                    .index
                    .all_words
                    .search(dfa)
                    .into_stream()
                    .into_strs()?;

                Ok(RegexSearchCandidates {
                    candidates: all_matched_trigrams
                        .iter()
                        .map(|t| SearchPartTrigram {
                            trigrams: vec![Trigram::new(t)],
                            docs_to_check: self
                                .index
                                .word_to_file_id_ever_contained
                                .get(t)
                                .unwrap()
                                .clone(),
                        })
                        .collect(),
                })
            }
            HirKind::Capture(_) | HirKind::Look(_) => {
                Err(anyhow!("Do not use capture or look"))
            }
        }
    }

    fn get_document_bitmap_containing_word(
        &mut self,
        word: &str,
    ) -> Option<(String, RoaringBitmap)> {
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
                .put(w.clone(), Some((w.clone(), overlaps.clone())));

            return Some((w, overlaps));
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
            Some((w.clone(), intersect_bitmaps(bitmaps.as_slice()).unwrap())),
        );

        Some((w, intersect_bitmaps(bitmaps.as_slice()).unwrap()))
    }

    fn find_overlapping_document(
        &self,
        bitmaps: &'i Vec<(String, RoaringBitmap)>,
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
                .map(|(word, _)| {
                    self.find_matching_commit_histories_in_doc(document, word)
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
                    query: Query::Words(selected_words),
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
    ) -> Vec<(String, RoaringBitmap)> {
        if word.len() < 3 {
            let words_to_find = find_all_words_containing_key(
                word,
                doc.all_words.as_ref().unwrap(),
            );

            let bitmap = intersect_bitmaps(
                &words_to_find
                    .into_iter()
                    .filter_map(|w| {
                        doc.words.get(&w).map(|index| &index.commit_inclutivity)
                    })
                    .collect::<Vec<_>>(),
            );

            return vec![(word.to_string(), bitmap.unwrap())];
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

    fn find_matching_commit_histories_in_doc_from_trigrams(
        &self,
        doc: &Document,
        trigrams: &[Trigram],
    ) -> Option<RoaringBitmap> {
        if trigrams.is_empty() {
            return None;
        }

        let mut commit_bitmaps = vec![];

        let first = &trigrams[0];
        if first.is_trigram() {
            for t in trigrams {
                if let Some(b) = doc.words.get(&t.to_string()) {
                    commit_bitmaps.push(&b.commit_inclutivity);
                } else {
                    return None;
                }
            }
        } else {
            assert!(trigrams.len() == 1);

            let words_to_find = find_all_words_containing_key(
                &first.to_string(),
                doc.all_words.as_ref().unwrap(),
            );

            commit_bitmaps = words_to_find
                .into_iter()
                .filter_map(|w| {
                    doc.words.get(&w).map(|index| &index.commit_inclutivity)
                })
                .collect::<Vec<_>>();
        }

        intersect_bitmaps(&commit_bitmaps)
    }
}

#[derive(Debug)]
pub enum Query {
    Words(Vec<String>),
    Regex(String),
}

#[derive(Debug)]
pub struct RawPerFileSearchResult {
    pub query: Query,
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

#[cfg(test)]
mod regex_build_hir {}
