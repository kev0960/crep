use std::time::Instant;

use anyhow::anyhow;
use log::debug;
use log::info;
use log::trace;
use regex_syntax::hir::Hir;
use regex_syntax::hir::HirKind;
use roaring::RoaringBitmap;
use trigram_hash::trigram_hash::split_lines_to_token_set;

use crate::index::git_index::GitIndex;
use crate::index::git_indexer::FileId;
use crate::search::core::search_docs::find_all_words_containing_key;
use crate::search::core::search_docs::find_matching_commit_histories_in_doc;
use crate::search::core::search_docs::find_matching_commit_histories_in_doc_from_trigrams;
use crate::search::core::search_docs::find_matching_trigram;
use crate::search::permutation::PermutationIterator;
use crate::util::bitmap::utils::intersect_bitmap_vec;
use crate::util::bitmap::utils::intersect_bitmaps;
use crate::util::bitmap::utils::union_bitmaps;

use super::regex_search::RegexSearchCandidates;
use super::regex_search::SearchPartTrigram;
use super::regex_search::Trigram;

pub struct GitSearcher<'i> {
    index: &'i GitIndex,
}

#[derive(Default)]
pub struct SearchOption {
    pub max_num_to_find: Option<usize>,
}

impl<'i> GitSearcher<'i> {
    pub fn new(index: &'i GitIndex) -> Self {
        Self { index }
    }

    pub fn search(
        &self,
        query: &str,
        option: Option<SearchOption>,
    ) -> Vec<RawPerFileSearchResult> {
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

        self.find_overlapping_document(&documents_containing_each_word, option)
    }

    pub fn regex_search(
        &self,
        query: &str,
        option: Option<SearchOption>,
    ) -> Result<Vec<RawPerFileSearchResult>, String> {
        if query.is_empty() {
            return Ok(vec![]);
        }

        let parse_start = Instant::now();
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

        trace!("Candiates: {candidates:?}");

        let candidate_build_done = Instant::now();

        let mut search_result = vec![];
        let option = option.unwrap_or_default();

        for cand in candidates.candidates {
            trace!("Checking candidate: {cand:?}");

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
            trace!("Found candidate docs: {candidate_docs:?}");

            for doc_id in candidate_docs {
                let doc =
                    self.index.file_id_to_document.get(&(doc_id as FileId));

                if doc.is_none() {
                    continue;
                }

                // Find the matching commit histories.
                let matching_history =
                    find_matching_commit_histories_in_doc_from_trigrams(
                        doc.as_ref().unwrap(),
                        &trigrams,
                    )
                    .map_err(|e| e.to_string())?;

                if let Some(history) = matching_history {
                    search_result.push(RawPerFileSearchResult {
                        file_id: doc_id,
                        query: Query::Regex(query.to_owned()),
                        overlapped_commits: history,
                    });

                    if let Some(max_num_to_find) = option.max_num_to_find
                        && search_result.len() >= max_num_to_find
                    {
                        return Ok(search_result);
                    }
                }
            }
        }

        let raw_query_result_done = Instant::now();
        info!(
            "Candidate build: {}ms, Raw query result: {}ms",
            candidate_build_done.duration_since(parse_start).as_millis(),
            raw_query_result_done
                .duration_since(candidate_build_done)
                .as_millis()
        );

        Ok(search_result)
    }

    fn build_candidates_from_hir(
        &self,
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
        &self,
        word: &str,
    ) -> Option<(String, RoaringBitmap)> {
        let w = word.to_owned();

        if word.len() <= 2 {
            let docs =
                find_all_words_containing_key(word, &self.index.all_words)
                    .into_iter()
                    .filter_map(|word| {
                        self.index
                            .word_to_file_id_ever_contained
                            .get(&word.into())
                    })
                    .collect::<Vec<_>>();

            let overlaps = union_bitmaps(&docs).unwrap();
            return Some((w, overlaps));
        }

        let lines = vec![w.clone()];
        let trigrams = split_lines_to_token_set(&lines);

        // Find the document that contains all matching tokens.
        let bitmaps = trigrams
            .iter()
            .filter_map(|t| self.index.word_to_file_id_ever_contained.get(t))
            .collect::<Vec<_>>();

        if bitmaps.len() != trigrams.len() {
            return None;
        }

        Some((w, intersect_bitmaps(bitmaps.as_slice()).unwrap()))
    }

    fn find_overlapping_document(
        &self,
        bitmaps: &[(String, RoaringBitmap)],
        option: Option<SearchOption>,
    ) -> Vec<RawPerFileSearchResult> {
        let mut result = vec![];

        let docs_for_each_word =
            bitmaps.iter().map(|b| &b.1).collect::<Vec<_>>();
        let intersected_docs = intersect_bitmaps(&docs_for_each_word).unwrap();
        let option = option.unwrap_or_default();

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
                    find_matching_commit_histories_in_doc(document, word)
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
                let mut selected_bitmaps = vec![&document.doc_modified_commits];

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

                if let Some(max_num_to_find) = option.max_num_to_find
                    && result.len() >= max_num_to_find
                {
                    return result;
                }
            }
        }

        result
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
