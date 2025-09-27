use std::path::Path;

use crep_indexer::{
    index::{
        git_index::GitIndex,
        indexer::{IndexResult, Indexer, IndexerConfig},
    },
    search::{
        git_searcher::{GitSearcher, RawPerFileSearchResult},
        search_result::SearchResult,
    },
};
use git2::{Oid, Repository};

pub struct Searcher<'a> {
    repo: Repository,
    index: &'a GitIndex,
    searcher: GitSearcher<'a>,
}

pub enum Query<'a> {
    Regex(&'a str),
    Key(&'a str),
}

impl<'a> Searcher<'a> {
    pub fn new(index: &'a GitIndex, path: &str) -> Self {
        Self {
            repo: Repository::open(Path::new(path)).unwrap(),
            index,
            searcher: GitSearcher::new(&index),
        }
    }

    pub fn handle_query(
        &self,
        query: &Query,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let search_results = vec![];

        let raw_results = match query {
            Query::Regex(regex) => self.searcher.regex_search(regex),
            Query::Key(key) => Ok(self.searcher.search(key)),
        }?;

        for result in raw_results {
            for commit_id in result.overlapped_commits {
                let (file_path, content) = self.read_file_at_commit(
                    result.file_id as usize,
                    commit_id as usize,
                )?;

                search_results.push(SearchResult::new(
                    &result,
                    file_path,
                    &content.lines().collect::<Vec<&str>>(),
                )?);
            }
        }

        Ok(search_results)
    }

    fn read_file_at_commit(
        &self,
        file_id: usize,
        commit_index: usize,
    ) -> anyhow::Result<(&str, String)> {
        let commit_id = Oid::from_bytes(
            &self.index.commit_index_to_commit_id[commit_index],
        )?;

        let commit = self.repo.find_commit(commit_id)?;
        let tree = commit.tree()?;

        let file_path = &self.index.file_id_to_path[file_id];
        let entry = tree.get_path(Path::new(file_path))?;

        let object = entry.to_object(&self.repo)?;
        if let Some(blob) = object.as_blob() {
            Ok((
                file_path,
                String::from_utf8_lossy(blob.content()).to_string(),
            ))
        } else {
            anyhow::bail!("Path is not a blob file {file_path}");
        }
    }
}

fn handle_query(index: GitIndex, path: &str) {
    let mut searcher = GitSearcher::new(&index);

    loop {
        print!("Query :: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let input = input.trim();

        if input.is_empty() {
            break;
        }

        let results = searcher.regex_search(input).unwrap();
        viewer.show_results(&results).unwrap();
    }
}
