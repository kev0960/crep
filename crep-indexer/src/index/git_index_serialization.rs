use std::fs::File;
use std::io;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::path::Path;

use ahash::AHashMap;
use ahash::AHashSet;
use bincode::serde as bserde;
use indicatif::ProgressBar;
use indicatif::ProgressDrawTarget;
use indicatif::ProgressStyle;
use roaring::RoaringBitmap;
use serde::Deserialize;
use serde::Serialize;
use trigram_hash::trigram_hash::TrigramKey;

use crate::git::diff::FileDiffTracker;
use crate::index::document::Document;
use crate::index::git_indexer::FileId;
use crate::index::git_indexer::GitIndexer;

#[derive(Serialize, Deserialize)]
pub struct GitIndexSerialization {
    pub commit_index_to_commit_id: Vec<[u8; 20]>,

    pub file_id_to_path: Vec<String>,
    pub file_id_to_document: AHashMap<FileId, Document>,
    pub file_id_to_diff_tracker: AHashMap<FileId, FileDiffTracker>,

    pub word_to_file_id_ever_contained: AHashMap<TrigramKey, RoaringBitmap>,

    pub ignored_non_utf8_file_path_set: AHashSet<String>,
}

// Borrowed view over a [`GitIndexer`] used purely for serialization.
#[derive(Serialize)]
pub struct GitIndexSerializationRef<'a> {
    pub commit_index_to_commit_id: &'a Vec<[u8; 20]>,

    pub file_id_to_path: &'a Vec<String>,
    pub file_id_to_document: &'a AHashMap<FileId, Document>,
    pub file_id_to_diff_tracker: &'a AHashMap<FileId, FileDiffTracker>,

    pub word_to_file_id_ever_contained: &'a AHashMap<TrigramKey, RoaringBitmap>,

    pub ignored_non_utf8_file_path_set: &'a AHashSet<String>,
}

fn encode_to_file<T: Serialize>(
    value: &T,
    file_path: &Path,
) -> anyhow::Result<()> {
    let file = File::create(file_path)?;

    let mut writer = BufWriter::new(file);
    bserde::encode_into_std_write(
        value,
        &mut writer,
        bincode::config::standard(),
    )?;

    Ok(())
}

impl GitIndexSerializationRef<'_> {
    pub fn save(&self, file_path: &Path) -> anyhow::Result<()> {
        encode_to_file(self, file_path)
    }
}

impl GitIndexSerialization {
    pub fn load(file_path: &Path) -> anyhow::Result<Self> {
        let file = File::open(file_path)?;

        let file_size = file.metadata()?.len();

        let progress = ProgressBar::new(file_size);
        progress.set_style(ProgressStyle::default_bar().template(
                            "{spinner:.green} [{elapsed_precise}] [{bar:60.cyan/blue}] {percent}%   {decimal_bytes:>7}/{decimal_total_bytes:7} {msg}"
                        ).unwrap());
        progress.set_draw_target(ProgressDrawTarget::stderr_with_hz(5));

        let mut reader = ProgressFileReader {
            inner: BufReader::new(file),
            progress,
            bytes_read: 0,
            pending_bytes: 0,
        };

        let decoded = bserde::decode_from_std_read(
            &mut reader,
            bincode::config::standard(),
        )?;

        Ok(decoded)
    }
}

impl<'a> From<&'a GitIndexer> for GitIndexSerializationRef<'a> {
    fn from(index: &'a GitIndexer) -> Self {
        Self {
            commit_index_to_commit_id: &index.commit_index_to_commit_id,
            file_id_to_path: &index.file_id_to_path,
            file_id_to_document: &index.file_id_to_document,
            file_id_to_diff_tracker: &index.file_id_to_diff_tracker,
            word_to_file_id_ever_contained: &index
                .word_to_file_id_ever_contained,
            ignored_non_utf8_file_path_set: &index
                .ignored_non_utf8_file_path_set,
        }
    }
}

impl<'a> From<&'a GitIndexSerialization> for GitIndexSerializationRef<'a> {
    fn from(index: &'a GitIndexSerialization) -> Self {
        Self {
            commit_index_to_commit_id: &index.commit_index_to_commit_id,
            file_id_to_path: &index.file_id_to_path,
            file_id_to_document: &index.file_id_to_document,
            file_id_to_diff_tracker: &index.file_id_to_diff_tracker,
            word_to_file_id_ever_contained: &index
                .word_to_file_id_ever_contained,
            ignored_non_utf8_file_path_set: &index
                .ignored_non_utf8_file_path_set,
        }
    }
}

struct ProgressFileReader<R> {
    inner: R,
    progress: ProgressBar,
    bytes_read: usize,
    pending_bytes: usize,
}

impl<R: Read> Read for ProgressFileReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.bytes_read += n;
        self.pending_bytes += n;

        // Only update the ProgressBar every 10MiB read.
        if self.pending_bytes >= 10 * 1024 * 1024 {
            self.progress.set_position(self.bytes_read as u64);
            self.pending_bytes = 0;
        }

        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use bincode::serde;

    use crate::index::document::WordKey;

    use super::*;

    /// Builds a representative index that exercises every field of
    /// [`GitIndexSerialization`].
    fn sample_index() -> GitIndexSerialization {
        let mut document_a = Document::new();
        document_a.add_words(
            1,
            AHashMap::from_iter(vec![
                ("abc".into(), vec![1, 2, 3]),
                ("bcd".into(), vec![3, 4]),
            ]),
        );
        document_a
            .add_words(2, AHashMap::from_iter(vec![("bcd".into(), vec![5])]));
        document_a.remove_words(
            3,
            &[(
                "abc".into(),
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

        let mut diff_tracker = FileDiffTracker::new(1, 10);
        diff_tracker.add_lines(3, 5, (2, 0));

        let mut ever_contained = RoaringBitmap::new();
        ever_contained.insert(1);
        ever_contained.insert(2);

        GitIndexSerialization {
            commit_index_to_commit_id: vec![[0; 20], [1; 20], [2; 20]],
            file_id_to_path: vec![
                "/a".to_owned(),
                "/b".to_owned(),
                "/deleted".to_owned(),
            ],
            file_id_to_diff_tracker: AHashMap::from_iter(vec![(
                0,
                diff_tracker,
            )]),
            file_id_to_document: AHashMap::from_iter(vec![(1, document_a)]),
            word_to_file_id_ever_contained: AHashMap::from_iter(vec![(
                "abc".into(),
                ever_contained,
            )]),
            ignored_non_utf8_file_path_set: AHashSet::from_iter(vec![
                "/deleted".to_owned(),
            ]),
        }
    }

    impl PartialEq for GitIndexSerialization {
        fn eq(&self, other: &Self) -> bool {
            self.commit_index_to_commit_id == other.commit_index_to_commit_id
                && self.file_id_to_path == other.file_id_to_path
                && self.file_id_to_document == other.file_id_to_document
                && self.file_id_to_diff_tracker == other.file_id_to_diff_tracker
                && self.word_to_file_id_ever_contained
                    == other.word_to_file_id_ever_contained
                && self.ignored_non_utf8_file_path_set
                    == other.ignored_non_utf8_file_path_set
        }
    }

    #[test]
    fn test_serde() {
        let index = sample_index();

        let encoded =
            serde::encode_to_vec(&index, bincode::config::standard()).unwrap();

        let (decoded, _): (GitIndexSerialization, usize) =
            serde::decode_from_slice(
                encoded.as_slice(),
                bincode::config::standard(),
            )
            .unwrap();

        assert!(index == decoded);
    }

    #[test]
    fn test_save_and_load() {
        let index = sample_index();
        let index_ref = GitIndexSerializationRef::from(&index);

        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("index.bin");

        index_ref.save(&file_path).expect("save should succeed");
        assert!(file_path.exists());

        let loaded = GitIndexSerialization::load(&file_path)
            .expect("load should succeed");

        assert!(index == loaded);
    }

    #[test]
    fn test_load_missing_file_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("does_not_exist.bin");

        assert!(GitIndexSerialization::load(&missing).is_err());
    }
}
