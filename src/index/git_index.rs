use std::collections::HashMap;

use roaring::RoaringBitmap;

use crate::git::diff::FileDiffTracker;

use super::{
    document::Document,
    git_indexer::{CommitIndex, FileId},
};

pub struct GitIndex {
    commit_index_to_commit_id: Vec<[u8; 20]>,
    commit_id_to_commit_index: HashMap<[u8; 20], CommitIndex>,

    file_name_to_id: HashMap<String, FileId>,
    file_id_to_name: Vec<String>,
    file_id_to_diff_tracker: HashMap<FileId, FileDiffTracker>,

    file_id_to_document: HashMap<FileId, Document>,
}
