use ahash::AHashMap;
use fst::Set;
use roaring::RoaringBitmap;
use trigram_hash::trigram_hash::TrigramKey;

use crate::index::git_indexer::CommitIndex;

use super::document::Document;
use super::git_indexer::FileId;
use super::git_indexer::GitIndexer;

#[derive(Debug)]
pub struct GitIndex {
    pub commit_index_to_commit_id: Vec<[u8; 20]>,
    pub commit_id_to_commit_index: AHashMap<[u8; 20], CommitIndex>,

    pub file_id_to_path: Vec<String>,

    pub file_id_to_document: AHashMap<FileId, Document>,
    pub word_to_file_id_ever_contained: AHashMap<TrigramKey, RoaringBitmap>,

    pub all_words: Set<Vec<u8>>,
}

pub struct GitIndexRef<'a> {
    pub commit_index_to_commit_id: &'a Vec<[u8; 20]>,
    pub commit_id_to_commit_index: &'a AHashMap<[u8; 20], CommitIndex>,

    pub file_id_to_path: &'a Vec<String>,

    pub file_id_to_document: &'a AHashMap<FileId, Document>,
    pub word_to_file_id_ever_contained: &'a AHashMap<TrigramKey, RoaringBitmap>,

    pub all_words: &'a Set<Vec<u8>>,
}

impl<'a> From<&'a GitIndex> for GitIndexRef<'a> {
    fn from(v: &'a GitIndex) -> Self {
        Self {
            commit_index_to_commit_id: &v.commit_index_to_commit_id,
            commit_id_to_commit_index: &v.commit_id_to_commit_index,
            file_id_to_path: &v.file_id_to_path,
            file_id_to_document: &v.file_id_to_document,
            word_to_file_id_ever_contained: &v.word_to_file_id_ever_contained,
            all_words: &v.all_words,
        }
    }
}

impl From<GitIndexer> for GitIndex {
    fn from(indexer: GitIndexer) -> Self {
        let mut keys = indexer
            .word_to_file_id_ever_contained
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();

        let all_words = Set::from_iter(keys).unwrap();

        /*
                let not_deleted_files_head = RoaringBitmap::from_iter(
                    indexer.file_id_to_document.iter().filter_map(|(k, v)| {
                        if v.is_deleted {
                            return None;
                        }

                        Some(*k as u32)
                    }),
                );
        */

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
