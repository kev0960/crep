use std::fs::File;
use std::io;
use std::io::BufWriter;
use std::io::Read;
use std::path::Path;

use ahash::AHashMap;
use bincode::serde as bserde;
use fst::Set;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use roaring::RoaringBitmap;
use serde::Deserialize;
use serde::Serialize;
use trigram_hash::trigram_hash::TrigramKey;

use super::document::Document;
use super::git_indexer::CommitIndex;
use super::git_indexer::FileId;
use super::git_indexer::GitIndexer;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitIndex {
    pub commit_index_to_commit_id: Vec<[u8; 20]>,
    pub commit_id_to_commit_index: AHashMap<[u8; 20], CommitIndex>,

    pub file_id_to_path: Vec<String>,

    pub file_id_to_document: AHashMap<FileId, Document>,
    pub word_to_file_id_ever_contained: AHashMap<TrigramKey, RoaringBitmap>,

    // Files that are not deleted at HEAD.
    pub not_deleted_files_head: RoaringBitmap,

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
        let not_deleted_files_head = RoaringBitmap::from_iter(
            indexer.file_id_to_document.iter().filter_map(|(k, v)| {
                if v.is_deleted {
                    return None;
                }

                Some(*k as u32)
            }),
        );

        Self {
            commit_index_to_commit_id: indexer.commit_index_to_commit_id,
            commit_id_to_commit_index: indexer.commit_id_to_commit_index,
            file_id_to_path: indexer.file_id_to_path,
            file_id_to_document: indexer.file_id_to_document,
            word_to_file_id_ever_contained: indexer
                .word_to_file_id_ever_contained,
            all_words,
            not_deleted_files_head,
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
        let file_size = file.metadata()?.len();

        let progress = ProgressBar::new(file_size);
        progress.set_style(ProgressStyle::default_bar().template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:60.cyan/blue}] {percent}%   {decimal_bytes:>7}/{decimal_total_bytes:7} {msg}"
                ).unwrap());

        let mut reader = ProgressFileReader {
            file,
            progress,
            bytes_read: 0,
        };

        let decoded = bserde::decode_from_std_read(
            &mut reader,
            bincode::config::standard(),
        )?;

        Ok(decoded)
    }
}

struct ProgressFileReader {
    file: File,
    progress: ProgressBar,
    bytes_read: usize,
}

impl Read for ProgressFileReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.file.read(buf)?;
        self.bytes_read += n;
        self.progress.set_position(self.bytes_read as u64);

        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use bincode::serde;

    use crate::index::document::WordKey;
    use crate::util::fst::test_util::test::convert_fst_to_string_vec;

    use super::*;

    #[test]
    fn test_serde() {
        let mut document_a = Document::new();
        document_a.add_words(
            1,
            AHashMap::from_iter(vec![
                ("a".into(), vec![1, 2, 3]),
                ("b".into(), vec![3, 4]),
            ]),
        );
        document_a
            .add_words(2, AHashMap::from_iter(vec![("b".into(), vec![5])]));
        document_a.remove_words(
            3,
            &[(
                "a".into(),
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
            commit_id_to_commit_index: AHashMap::new(),
            file_id_to_path: vec!["/a".to_owned(), "/b".to_owned()],
            file_id_to_document: AHashMap::from_iter(vec![(1, document_a)]),
            word_to_file_id_ever_contained: AHashMap::new(),
            all_words: Set::from_iter(["a", "ab", "abc"].iter()).unwrap(),
            not_deleted_files_head: RoaringBitmap::from_sorted_iter(0..2)
                .unwrap(),
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
