use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crep_indexer::index::git_index::GitIndexRef;
use crep_indexer::index::git_indexer::GitIndexer;
use fst::Set;
use tokio::sync::OwnedRwLockReadGuard;

// Current git index status. The GitIndex is generated as a read only "view"
// so that we dont unnecessarily copy the data.
pub struct SearchIndex {
    indexer: GitIndexer,
    repo_path: PathBuf,
    all_words: Set<Vec<u8>>,
}

impl SearchIndex {
    pub fn new(indexer: GitIndexer, repo_path: &Path) -> Self {
        let all_words = build_all_words(&indexer);

        Self {
            indexer,
            all_words,
            repo_path: PathBuf::from(repo_path),
        }
    }

    pub fn as_index_ref(&self) -> GitIndexRef<'_> {
        GitIndexRef {
            commit_index_to_commit_id: &self.indexer.commit_index_to_commit_id,
            commit_id_to_commit_index: &self.indexer.commit_id_to_commit_index,
            file_id_to_path: &self.indexer.file_id_to_path,
            file_id_to_document: &self.indexer.file_id_to_document,
            word_to_file_id_ever_contained: &self
                .indexer
                .word_to_file_id_ever_contained,
            all_words: &self.all_words,
        }
    }

    pub fn do_incremental_index(
        &mut self,
        commit_id: &str,
    ) -> anyhow::Result<bool> {
        let mut commit_id_raw = [0u8; 20];
        hex::decode_to_slice(commit_id, &mut commit_id_raw)?;

        if self
            .indexer
            .commit_id_to_commit_index
            .get(&commit_id_raw)
            .is_some()
        {
            return Ok(false);
        }

        // This is the commit that is not seen yet. Let's re-index!

        let repo = git2::Repository::open(&self.repo_path)?;
        self.indexer.index_history(repo)?;
        self.all_words = build_all_words(&self.indexer);

        Ok(true)
    }
}

fn build_all_words(indexer: &GitIndexer) -> Set<Vec<u8>> {
    let mut keys = indexer
        .word_to_file_id_ever_contained
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();

    Set::from_iter(keys).expect("keys are sorted and deduplicated")
}

// Represents the immutable view over the GitIndex.
#[derive(Clone)]
pub struct SearchIndexGuard(pub Arc<OwnedRwLockReadGuard<SearchIndex>>);

impl SearchIndexGuard {
    pub fn as_index_ref(&self) -> GitIndexRef<'_> {
        self.0.as_index_ref()
    }
}

impl Deref for SearchIndexGuard {
    type Target = SearchIndex;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
