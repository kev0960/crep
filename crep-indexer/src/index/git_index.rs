use std::collections::HashMap;

use fst::Set;
use roaring::RoaringBitmap;

use super::{
    document::Document,
    git_indexer::{CommitIndex, FileId, GitIndexer},
};

pub struct GitIndex {
    pub commit_index_to_commit_id: Vec<[u8; 20]>,
    pub commit_id_to_commit_index: HashMap<[u8; 20], CommitIndex>,

    pub file_id_to_path: Vec<String>,

    pub file_id_to_document: HashMap<FileId, Document>,
    pub word_to_file_id_ever_contained: HashMap<String, RoaringBitmap>,

    pub all_words: Set<Vec<u8>>,
}

impl GitIndex {
    // Build the finalized index.
    pub fn build(indexer: GitIndexer) -> Self {
        let mut keys = indexer
            .word_to_file_id_ever_contained
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();

        let all_words = Set::from_iter(keys).unwrap();

        Self {
            commit_index_to_commit_id: indexer.commit_index_to_commit_id,
            commit_id_to_commit_index: indexer.commit_id_to_commit_index,
            file_id_to_path: indexer.file_id_to_path,
            file_id_to_document: indexer.file_id_to_document,
            word_to_file_id_ever_contained: indexer
                .word_to_file_id_ever_contained,
            all_words,
        }
    }
}
