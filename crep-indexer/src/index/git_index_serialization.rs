use ahash::AHashMap;
use fst::Set;
use roaring::RoaringBitmap;
use serde::Deserialize;
use serde::Serialize;
use trigram_hash::trigram_hash::TrigramKey;

use crate::git::diff::FileDiffTracker;
use crate::index::document::Document;
use crate::index::git_index::GitIndex;
use crate::index::git_indexer::CommitIndex;
use crate::index::git_indexer::FileId;

#[derive(Serialize, Deserialize)]
pub struct GitIndexSerialization {
    pub commit_index_to_commit_id: Vec<[u8; 20]>,
    pub file_id_to_path: Vec<String>,
    pub file_id_to_document: AHashMap<FileId, Document>,
    pub word_to_file_id_ever_contained: AHashMap<TrigramKey, RoaringBitmap>,
    pub not_deleted_files_head: RoaringBitmap,
    pub diff_tracker: Vec<Option<FileDiffTracker>>,
}

impl From<GitIndexSerialization> for GitIndex {
    fn from(s: GitIndexSerialization) -> Self {
        let mut keys = s
            .word_to_file_id_ever_contained
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();

        let all_words = Set::from_iter(keys).unwrap();
        let commit_id_to_commit_index = s
            .commit_index_to_commit_id
            .iter()
            .enumerate()
            .map(|(i, k)| (*k, i))
            .collect::<AHashMap<[u8; 20], CommitIndex>>();

        Self {
            commit_index_to_commit_id: s.commit_index_to_commit_id,
            commit_id_to_commit_index,
            file_id_to_path: s.file_id_to_path,
            file_id_to_document: s.file_id_to_document,
            word_to_file_id_ever_contained: s.word_to_file_id_ever_contained,
            not_deleted_files_head: s.not_deleted_files_head,
            all_words,
            diff_tracker: s.diff_tracker,
        }
    }
}
