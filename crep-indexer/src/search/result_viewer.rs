use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use aho_corasick::AhoCorasick;
use anyhow::Result;
use git2::Oid;
use git2::Repository;
use owo_colors::OwoColorize;
use regex::Regex;

use crate::index::git_index::GitIndex;
use crate::index::git_indexer::CommitIndex;

use super::git_searcher::Query;
use super::git_searcher::RawPerFileSearchResult;
use super::line_formatter::highlight_line_by_positions;

static MAX_SEARCH_RESULT: usize = 10;

pub struct GitSearchResultViewer<'i> {
    repo: Repository,
    index: &'i GitIndex,
}

impl<'i> GitSearchResultViewer<'i> {
    pub fn new(path: &str, index: &'i GitIndex) -> Self {
        Self {
            repo: git2::Repository::open(Path::new(path)).unwrap(),
            index,
        }
    }

    pub fn show_results(
        &self,
        results: &[RawPerFileSearchResult],
    ) -> Result<()> {
        let mut index = 0;
        for result in results {
            let result = self.show_result(index + 1, result);
            if let Some(result) = result? {
                println!("{result}\n\n");
                index += 1;
            }

            if index >= MAX_SEARCH_RESULT {
                println!("Too many results.. return");
                break;
            }
        }

        Ok(())
    }

    fn show_result(
        &self,
        index: usize,
        result: &RawPerFileSearchResult,
    ) -> Result<Option<String>> {
        let first_commit_introduced = result.overlapped_commits.min();

        if first_commit_introduced.is_none() {
            return Ok(None);
        }

        let file_content = self
            .read_file_at_commit(
                result.file_id as usize,
                first_commit_introduced.unwrap() as usize,
            )?
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<String>>();

        let matches = match &result.query {
            Query::Words(words) => self
                .find_word_matches_in_document(words, &file_content)?
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect::<Vec<_>>(),
            Query::Regex(regex) => {
                let r = Regex::new(regex)?;
                self.find_regex_matches_in_document(&r, &file_content)
            }
        };

        if let Query::Words(ref words) = result.query {
            if matches.len() != words.len() {
                // Not every words are found in the document.
                return Ok(None);
            }
        }

        let mut line_to_words: HashMap<usize, Vec<(&str, usize)>> =
            HashMap::new();

        for (k, (line_num, col_num)) in &matches {
            line_to_words
                .entry(*line_num)
                .or_default()
                .push((k, *col_num));
        }

        for words in line_to_words.values_mut() {
            words.sort_by(|left, right| left.1.cmp(&right.1));
        }

        let lines_to_show = matches
            .iter()
            .flat_map(|(_, (line, _))| {
                (line.saturating_sub(2)..line.saturating_add(2))
                    .filter(|l| *l < file_content.len())
            })
            .collect::<BTreeSet<usize>>();

        if lines_to_show.is_empty() {
            return Ok(None);
        }

        let mut result = format!(
            "{index}. Found words {} at {}\n",
            matches
                .iter()
                .map(|(k, _)| *k)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(",")
                .red(),
            self.index.file_id_to_path[result.file_id as usize].yellow()
        );

        let mut prev_line_num = None;
        for line_num in lines_to_show {
            if let Some(prev_line_num) = prev_line_num
                && prev_line_num < line_num - 1
            {
                result.push_str("...\n\n")
            }

            // If the line contains the matched word, then we should highlight it.
            // Otherwise just show the line.
            if let Some(words) = line_to_words.get(&line_num) {
                // highlight words.
                result.push_str(&format!(
                    "{:>6}| {}\n",
                    line_num,
                    highlight_line_by_positions(&file_content[line_num], words)
                ));
            } else {
                result.push_str(&format!(
                    "{:>6}| {}\n",
                    line_num, file_content[line_num]
                ));
            }

            prev_line_num = Some(line_num);
        }

        Ok(Some(result))
    }

    fn read_file_at_commit(
        &self,
        file_id: usize,
        commit_index: CommitIndex,
    ) -> Result<String> {
        let commit_id = Oid::from_bytes(
            &self.index.commit_index_to_commit_id[commit_index],
        )?;

        let commit = self.repo.find_commit(commit_id)?;
        let tree = commit.tree()?;

        let file_path = &self.index.file_id_to_path[file_id];
        let entry = tree.get_path(Path::new(file_path))?;

        let object = entry.to_object(&self.repo)?;
        if let Some(blob) = object.as_blob() {
            Ok(String::from_utf8_lossy(blob.content()).to_string())
        } else {
            anyhow::bail!("Path is not a blob file {file_path}");
        }
    }

    fn find_word_matches_in_document<'w>(
        &self,
        words: &'w [String],
        content: &[String],
    ) -> Result<HashMap<&'w str, (usize, usize)>> {
        let ac = AhoCorasick::builder()
            .match_kind(aho_corasick::MatchKind::LeftmostFirst)
            .build(words)?;

        let mut word_pos_found: HashMap<usize, (usize, usize)> =
            HashMap::with_capacity(words.len());

        for (line_num, line) in content.iter().enumerate() {
            for m in ac.find_iter(line) {
                if word_pos_found.contains_key(&m.pattern().as_usize()) {
                    continue;
                }

                word_pos_found
                    .insert(m.pattern().as_usize(), (line_num, m.start()));
            }
        }

        Ok(word_pos_found
            .into_iter()
            .map(|(k, v)| return (words[k].as_str(), v))
            .collect())
    }

    fn find_regex_matches_in_document<'w>(
        &self,
        regex: &Regex,
        content: &'w [String],
    ) -> Vec<(&'w str, (usize, usize))> {
        let mut word_pos_found_lines = vec![];

        for (line_num, line) in content.iter().enumerate() {
            if let Some(m) = regex.find(line) {
                word_pos_found_lines.push((m.as_str(), (line_num, m.start())));
            }

            if word_pos_found_lines.len() > 10 {
                break;
            }
        }

        word_pos_found_lines
    }
}
