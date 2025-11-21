use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;

use aho_corasick::AhoCorasick;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;

use crate::index::git_indexer::CommitIndex;
use crate::search::git_searcher::Query;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleCommitSearchResult {
    pub commit_id: CommitIndex,
    pub words_per_line: BTreeMap<usize, Vec<(String, usize)>>,
    pub lines: BTreeMap<usize, String>,
}

#[derive(Debug, Copy, Clone)]
pub struct MatchingWordPos {
    pub line_num: usize,
    pub col: usize,
}

impl SingleCommitSearchResult {
    pub fn new(
        query: &Query,
        commit_id: CommitIndex,
        file_content: &[&str],
    ) -> anyhow::Result<Option<Self>> {
        let matches = match query {
            Query::Words(words) => {
                Self::find_word_matches_in_document(words, file_content)?
                    .iter()
                    .map(|(k, v)| (*k, *v))
                    .collect::<Vec<_>>()
            }
            Query::Regex(regex) => {
                let r = Regex::new(regex)?;
                Self::find_regex_matches_in_document(&r, file_content)
            }
        };

        if let Query::Words(words) = query
            && matches.len() != words.len()
        {
            // Not every words are found in the document.
            return Ok(None);
        }

        if matches.is_empty() {
            return Ok(None);
        }

        let mut words_per_line: BTreeMap<usize, Vec<(String, usize)>> =
            BTreeMap::new();

        for (k, pos) in &matches {
            words_per_line
                .entry(pos.line_num)
                .or_default()
                .push((k.to_string(), pos.col));
        }

        for words in words_per_line.values_mut() {
            words.sort_by(|left, right| left.1.cmp(&right.1));
        }

        let lines = matches
            .iter()
            .flat_map(|(_, pos)| {
                (pos.line_num.saturating_sub(2)..pos.line_num.saturating_add(2))
                    .filter(|l| *l < file_content.len())
            })
            .collect::<HashSet<usize>>()
            .into_iter()
            .map(|line_num| (line_num, file_content[line_num].to_owned()))
            .collect::<BTreeMap<usize, String>>();

        Ok(Some(SingleCommitSearchResult {
            commit_id,
            words_per_line,
            lines,
        }))
    }

    fn find_word_matches_in_document<'w>(
        words: &'w [String],
        content: &[&str],
    ) -> anyhow::Result<HashMap<&'w str, MatchingWordPos>> {
        let ac = AhoCorasick::builder()
            .match_kind(aho_corasick::MatchKind::LeftmostFirst)
            .build(words)?;

        let mut word_pos_found: HashMap<usize, MatchingWordPos> =
            HashMap::with_capacity(words.len());

        for (line_num, line) in content.iter().enumerate() {
            for m in ac.find_iter(line) {
                if word_pos_found.contains_key(&m.pattern().as_usize()) {
                    continue;
                }

                word_pos_found.insert(
                    m.pattern().as_usize(),
                    MatchingWordPos {
                        line_num,
                        col: m.start(),
                    },
                );
            }
        }

        Ok(word_pos_found
            .into_iter()
            .map(|(k, v)| (words[k].as_str(), v))
            .collect())
    }

    fn find_regex_matches_in_document<'w>(
        regex: &Regex,
        content: &'w [&str],
    ) -> Vec<(&'w str, MatchingWordPos)> {
        let mut word_pos_found_lines = vec![];

        for (line_num, line) in content.iter().enumerate() {
            if let Some(m) = regex.find(line) {
                word_pos_found_lines.push((
                    m.as_str(),
                    MatchingWordPos {
                        line_num,
                        col: m.start(),
                    },
                ));
            }

            if word_pos_found_lines.len() > 10 {
                break;
            }
        }

        word_pos_found_lines
    }
}
