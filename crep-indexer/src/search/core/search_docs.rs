use fst::IntoStreamer;
use fst::Set;
use log::trace;
use regex_automata::dense;
use roaring::RoaringBitmap;
use trigram_hash::trigram_hash::TrigramKey;
use trigram_hash::trigram_hash::split_lines_to_token_set;

use crate::index::document::Document;
use crate::index::git_indexer::CommitIndex;
use crate::search::regex_search::RegexOrString;
use crate::search::regex_search::Trigram;
use crate::util::bitmap::utils::intersect_bitmap_vec;
use crate::util::bitmap::utils::intersect_bitmaps;
use crate::util::bitmap::utils::union_bitmaps;

pub fn find_matching_commit_histories_in_doc_from_trigrams(
    doc: &Document,
    trigrams: &[Trigram],
    head_commit_index: CommitIndex,
) -> anyhow::Result<Option<RoaringBitmap>> {
    if trigrams.is_empty() {
        return Ok(None);
    }

    let mut commit_bitmaps = vec![doc.doc_modified_commits.clone()];
    if !doc.is_deleted {
        commit_bitmaps
            .last_mut()
            .unwrap()
            .insert(head_commit_index as u32);
    }

    for trigram in trigrams {
        if doc.all_words.is_none() {
            return Ok(None);
        }

        let matching_trigram =
            find_matching_trigram(trigram, doc.all_words.as_ref().unwrap())?;

        trace!("Matching trigrams : {matching_trigram:?}");

        let commit_histories_that_contains_word = matching_trigram
            .iter()
            .filter_map(|t| doc.words.get(t).map(|i| &i.commit_inclutivity))
            .collect::<Vec<_>>();

        if commit_histories_that_contains_word.is_empty() {
            return Ok(None);
        }

        commit_bitmaps
            .push(union_bitmaps(&commit_histories_that_contains_word).unwrap());

        trace!("Commit bitmaps: {:?}", commit_bitmaps.last());
    }

    Ok(intersect_bitmap_vec(commit_bitmaps))
}

pub fn find_matching_trigram(
    key: &Trigram,
    all_words: &Set<Vec<u8>>,
) -> anyhow::Result<Vec<TrigramKey>> {
    let matching_regex_or_string = key.create_matching_regex_or_string();
    match matching_regex_or_string {
        RegexOrString::String(s) => Ok(vec![s.into()]),
        RegexOrString::Regex(r) => {
            let dfa = dense::Builder::new().build(&r)?;
            Ok(all_words
                .search(dfa)
                .into_stream()
                .into_strs()?
                .into_iter()
                .map(|f| TrigramKey::from_utf8(&f))
                .collect())
        }
    }
}

pub fn find_matching_commit_histories_in_doc(
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
                    doc.words
                        .get(&w.into())
                        .map(|index| &index.commit_inclutivity)
                })
                .collect::<Vec<_>>(),
        );

        return vec![(word.to_string(), bitmap.unwrap())];
    }

    let lines = vec![word.to_owned()];
    let trigrams = split_lines_to_token_set(&lines);

    let mut commit_bitmaps = vec![];
    for w in trigrams {
        if let Some(b) = doc.words.get(&w) {
            commit_bitmaps.push(&b.commit_inclutivity);
        } else {
            return vec![];
        }
    }

    vec![(word.to_owned(), intersect_bitmaps(&commit_bitmaps).unwrap())]
}

pub fn find_all_words_containing_key(
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
