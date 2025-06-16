use roaring::RoaringBitmap;
use std::{collections::HashMap, path::Path};
use walkdir::WalkDir;

use crate::{
    index::{FileToWordPos, Index},
    tokenizer::Tokenizer,
};

pub struct Indexer {
    git_root: String,
}

impl Indexer {
    pub fn new(git_root: &str) -> Self {
        Indexer {
            git_root: git_root.to_string(),
        }
    }

    pub fn index_directory(&mut self) -> Index {
        let path = Path::new(&self.git_root);

        let mut num_indexed_files: i64 = 0;

        let mut files = vec![];

        let mut word_to_bitmap: HashMap<String, RoaringBitmap> = HashMap::new();
        let mut file_to_word_pos: FileToWordPos = HashMap::new();

        for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|entry| {
                let file_name = entry.file_name().to_string_lossy();
                if file_name.starts_with(".") {
                    return false;
                }

                if file_name == "target" {
                    return false;
                }

                true
            })
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() {
                if Indexer::index_file(
                    &mut word_to_bitmap,
                    &mut file_to_word_pos,
                    entry.path(),
                    num_indexed_files,
                )
                .is_err()
                {
                    continue;
                }

                files.push(entry.path().to_string_lossy().to_string());
                num_indexed_files += 1;
            }

            if num_indexed_files > 10000 {
                break;
            }
        }

        Index::new(files, word_to_bitmap, file_to_word_pos)
    }

    fn index_file(
        word_to_bitmap: &mut HashMap<String, RoaringBitmap>,
        file_to_word_pos: &mut FileToWordPos,
        path: &Path,
        file_id: i64,
    ) -> Result<(), std::io::Error> {
        let content = std::fs::read_to_string(path)?;

        let tokenizer_result = Tokenizer::split_to_words(&content);
        for word in tokenizer_result.total_words {
            let bitmap = word_to_bitmap.entry(word.to_string()).or_default();
            bitmap.insert(file_id as u32);
        }

        file_to_word_pos.insert(
            file_id as usize,
            tokenizer_result
                .word_pos
                .into_iter()
                .map(|(word, positions)| (word.to_string(), positions))
                .collect(),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn append_test_dir_path(p: &str) -> String {
        env::current_dir()
            .unwrap()
            .join(p)
            .to_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn test_indexing_directory() {
        let mut indexer = Indexer::new(&append_test_dir_path("test_data/indexer"));
        let index = indexer.index_directory();

        let mut files_indexed = index.files.clone();
        files_indexed.sort();

        assert_eq!(
            files_indexed,
            vec![
                append_test_dir_path("test_data/indexer/sub_dir/test3.js"),
                append_test_dir_path("test_data/indexer/test1.js"),
                append_test_dir_path("test_data/indexer/test2.js"),
            ]
        );

        pretty_assertions::assert_eq!(
            index.word_to_bitmap,
            HashMap::from_iter(vec![
                ("a".to_owned(), RoaringBitmap::from_iter(vec![0, 2])),
                ("function".to_owned(), RoaringBitmap::from_iter(vec![0])),
                ("export".to_owned(), RoaringBitmap::from_iter(vec![1, 2])),
                ("3".to_owned(), RoaringBitmap::from_iter(vec![2])),
                ("const".to_owned(), RoaringBitmap::from_iter(vec![2])),
            ])
        );

        pretty_assertions::assert_eq!(
            index.file_to_word_pos,
            HashMap::from_iter(vec![
                (
                    0,
                    HashMap::from_iter(vec![
                        ("a".to_owned(), vec![(0, 9)]),
                        ("function".to_owned(), vec![(0, 0)])
                    ])
                ),
                (
                    1,
                    HashMap::from_iter(vec![("export".to_owned(), vec![(0, 0)]),])
                ),
                (
                    2,
                    HashMap::from_iter(vec![
                        ("a".to_owned(), vec![(0, 13)]),
                        ("export".to_owned(), vec![(0, 0)]),
                        ("const".to_owned(), vec![(0, 7)]),
                        ("3".to_owned(), vec![(0, 17)]),
                    ])
                )
            ])
        );
    }
}
