use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::path::Path;

use bincode::serde as bserde;
use fst::Set;
use roaring::RoaringBitmap;
use serde::Deserialize;
use serde::Serialize;

use super::document::Document;
use super::git_indexer::CommitIndex;
use super::git_indexer::FileId;
use super::git_indexer::GitIndexer;

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

    use crate::{
        index::document::WordKey,
        util::fst::test_util::test::convert_fst_to_string_vec,
    };

    use super::*;

    #[test]
    fn test_serde() {
        let mut document_a = Document::new();
        document_a.add_words(
            1,
            HashMap::from_iter(vec![("a", vec![1, 2, 3]), ("b", vec![3, 4])]),
        );
        document_a.add_words(2, HashMap::from_iter(vec![("b", vec![5])]));
        document_a.remove_words(
            3,
            &[(
                "a",
                vec![
                    WordKey {
                        commit_id: 1,
                        line: 1,
                    },
                    WordKey {
                        commit_id: 1,
                        line: 2,
                    },
                ],
            )],
        );

        let index = GitIndex {
            commit_index_to_commit_id: vec![[0; 20], [1; 20]],
            commit_id_to_commit_index: HashMap::new(),
            file_id_to_path: vec!["/a".to_owned(), "/b".to_owned()],
            file_id_to_document: HashMap::from_iter(vec![(1, document_a)]),
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
        let v: Vec<String> = convert_fst_to_string_vec(&decoded.all_words);

        assert_eq!(v, vec!["a", "ab", "abc"])
    }
}
