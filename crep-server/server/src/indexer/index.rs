use std::ops::Deref;
use std::sync::Arc;

use crep_indexer::index::git_index::GitIndexRef;
use crep_indexer::index::git_indexer::GitIndexer;
use fst::Set;
use tokio::sync::OwnedRwLockReadGuard;

// Current git index status. The GitIndex is generated as a read only "view"
// so that we dont unnecessarily copy the data.
pub struct SearchIndex {
    indexer: GitIndexer,
    all_words: Set<Vec<u8>>,
}

impl SearchIndex {
    pub fn new(indexer: GitIndexer) -> Self {
        let all_words = build_all_words(&indexer);

        Self { indexer, all_words }
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

    pub fn refresh_all_words(&mut self) {
        self.all_words = build_all_words(&self.indexer);
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
