use std::collections::HashSet;

use anyhow::anyhow;
use fst::IntoStreamer;
use fst::Set;
use log::debug;
use lru::LruCache;
use regex_automata::dense;
use regex_syntax::hir::Hir;
use regex_syntax::hir::HirKind;
use roaring::RoaringBitmap;

use crate::index::document::Document;
use crate::index::git_index::GitIndex;
use crate::index::git_indexer::FileId;
use crate::search::permutation::PermutationIterator;
use crate::tokenizer::Tokenizer;
use crate::tokenizer::TokenizerMethod;
use crate::util::bitmap::utils::intersect_bitmap_vec;
use crate::util::bitmap::utils::intersect_bitmaps;
use crate::util::bitmap::utils::union_bitmaps;

use super::regex_search::RegexOrString;
use super::regex_search::RegexSearchCandidates;
use super::regex_search::SearchPartTrigram;
use super::regex_search::Trigram;

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
        if query.is_empty() {
            return vec![];
        }

        let words = query.split_whitespace();
        let mut documents_containing_each_word: Vec<(String, RoaringBitmap)> =
            vec![];

        for word in words {
            let results = self.get_document_bitmap_containing_word(word);
            if results.is_none() {
                return vec![];
            }

            documents_containing_each_word.push(results.unwrap());
        }

        self.find_overlapping_document(&documents_containing_each_word)
    }

    pub fn regex_search(
        &mut self,
        query: &str,
    ) -> Result<Vec<RawPerFileSearchResult>, String> {
        if query.is_empty() {
            return Ok(vec![]);
        }

        let hir = regex_syntax::parse(query);

        if hir.is_err() {
            return Err(format!(
                "Failed to parse regex {query}. Error: {:?}",
                hir.err()
            ));
        }

        let hir = hir.unwrap();
        debug!("Hir : {hir:?}");

        let candidates = self
            .build_candidates_from_hir(&hir)
            .map_err(|e| format!("Error building candidates {e:?}"))?;

        debug!("Candiates: {candidates:?}");

        let mut search_result = vec![];
        for cand in candidates.candidates {
            let trigrams = cand.trigrams;

            if trigrams.is_empty() {
                continue;
            }

            // Now find all the docs with the matching trigram.
            let mut docs_bitmaps = vec![];

            for trigram in &trigrams {
                let fetched_trigrams =
                    find_matching_trigram(trigram, &self.index.all_words)
                        .map_err(|e| e.to_string())?;

                let docs_that_contained_matching_trigrams = fetched_trigrams
                    .iter()
                    .filter_map(|t| {
                        self.index.word_to_file_id_ever_contained.get(t)
                    })
                    .collect::<Vec<_>>();

                if docs_that_contained_matching_trigrams.is_empty() {
                    // Clear the docs_bitmap since there is no match.
                    docs_bitmaps.clear();
                    break;
                }

                docs_bitmaps.push(
                    union_bitmaps(&docs_that_contained_matching_trigrams)
                        .unwrap(),
                );

                if docs_bitmaps.last().is_some_and(|b| b.is_empty()) {
                    // No need to continue as there will be no matching docs.
                    docs_bitmaps.clear();
                    break;
                }
            }

            // Now get the doc that contained all of the matching trigrams :)
            if docs_bitmaps.is_empty() {
                continue;
            }

            let candidate_docs = intersect_bitmap_vec(docs_bitmaps).unwrap();
            for doc_id in candidate_docs {
                let doc =
                    self.index.file_id_to_document.get(&(doc_id as FileId));

                if doc.is_none() {
                    continue;
                }

                // Find the matching commit histories.
                let matching_history = self
                    .find_matching_commit_histories_in_doc_from_trigrams(
                        doc.as_ref().unwrap(),
                        &trigrams,
                    )
                    .map_err(|e| e.to_string())?;

                if let Some(history) = matching_history {
                    search_result.push(RawPerFileSearchResult {
                        file_id: doc_id,
                        query: Query::Regex(query.to_owned()),
                        overlapped_commits: history,
                    })
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
                if let Some(bitmap) =
                    self.get_document_bitmap_containing_word(literal)
                // Only include if there is a doc that contains all of the literals.
                    && !bitmap.1.is_empty()
                {
                    let trigrams = Trigram::from_long_string(literal);
                    Ok(RegexSearchCandidates {
                        candidates: vec![SearchPartTrigram { trigrams }],
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
            HirKind::Class(class) => Ok(RegexSearchCandidates {
                candidates: vec![SearchPartTrigram {
                    trigrams: vec![Trigram::from(class)],
                }],
            }),
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

            debug!("commit histories {commit_histories_per_word:?}");
            let permutations = PermutationIterator::new(
                &commit_histories_per_word
                    .iter()
                    .map(|m| m.len() as u32)
                    .collect::<Vec<u32>>(),
            );

            for permutation in permutations {
                debug!("Permutations {permutation:?}");

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
    ) -> anyhow::Result<Option<RoaringBitmap>> {
        if trigrams.is_empty() {
            return Ok(None);
        }

        let mut commit_bitmaps = vec![];

        for trigram in trigrams {
            if doc.all_words.is_none() {
                return Ok(None);
            }

            let matching_trigram = find_matching_trigram(
                trigram,
                doc.all_words.as_ref().unwrap(),
            )?;

            let commit_histories_that_contains_word = matching_trigram
                .iter()
                .filter_map(|t| doc.words.get(t).map(|i| &i.commit_inclutivity))
                .collect::<Vec<_>>();

            if commit_histories_that_contains_word.is_empty() {
                return Ok(None);
            }

            commit_bitmaps.push(
                union_bitmaps(&commit_histories_that_contains_word).unwrap(),
            )
        }

        Ok(intersect_bitmap_vec(commit_bitmaps))
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

fn find_matching_trigram(
    key: &Trigram,
    all_words: &Set<Vec<u8>>,
) -> anyhow::Result<Vec<String>> {
    let matching_regex_or_string = key.create_matching_regex_or_string();
    match matching_regex_or_string {
        RegexOrString::String(s) => Ok(vec![s]),
        RegexOrString::Regex(r) => {
            let dfa = dense::Builder::new().build(&r)?;
            let strs = all_words.search(dfa).into_stream().into_strs()?;

            Ok(strs)
        }
    }
}

#[cfg(test)]
mod regex_build_hir {}
