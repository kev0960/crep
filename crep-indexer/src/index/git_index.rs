use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

use bincode::serde as bserde;
use fst::Set;
use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};

use super::{
    document::Document,
    git_indexer::{CommitIndex, FileId, GitIndexer},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct GitIndex {
    pub commit_index_to_commit_id: Vec<[u8; 20]>,
    pub commit_id_to_commit_index: HashMap<[u8; 20], CommitIndex>,

    pub file_id_to_path: Vec<String>,

    pub file_id_to_document: HashMap<FileId, Document>,
    pub word_to_file_id_ever_contained: HashMap<String, RoaringBitmap>,

    #[serde(with = "crate::util::serde::fst::fst_set_to_vec")]
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

    pub fn save(&self, file_path: &Path) -> anyhow::Result<()> {
        let file = File::create(file_path)?;

        let mut writer = BufWriter::new(file);
        bserde::encode_into_std_write(
            self,
            &mut writer,
            bincode::config::standard(),
        )?;

        Ok(())
    }

    pub fn load(file_path: &Path) -> anyhow::Result<Self> {
        let file = File::open(file_path)?;

        let mut reader = BufReader::new(file);

        let decoded = bserde::decode_from_std_read(
            &mut reader,
            bincode::config::standard(),
        )?;

        Ok(decoded)
    }
}

#[cfg(test)]
mod tests {
    use bincode::serde;

    use super::*;

    #[test]
    fn test_serde() {
        let index = GitIndex {
            commit_index_to_commit_id: vec![],
            commit_id_to_commit_index: HashMap::new(),
            file_id_to_path: vec![],
            file_id_to_document: HashMap::new(),
            word_to_file_id_ever_contained: HashMap::new(),
            all_words: Set::from_iter(["a", "ab", "abc"].iter()).unwrap(),
        };

        let encoded = serde::encode_to_vec(index, bincode::config::standard());
        assert!(encoded.is_ok());

        let (decoded, _): (GitIndex, usize) = serde::decode_from_slice(
            encoded.unwrap().as_slice(),
            bincode::config::standard(),
        )
        .unwrap();

        assert_eq!(decoded.all_words.len(), 3);
        let v: Vec<String> = decoded
            .all_words
            .stream()
            .into_bytes()
            .into_iter()
            .map(|v| str::from_utf8(v.as_slice()).unwrap().to_owned())
            .collect();

        assert_eq!(v, vec!["a", "ab", "abc"])
    }
}
