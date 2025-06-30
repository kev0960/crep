use indicatif::{ProgressBar, ProgressStyle};
use roaring::RoaringBitmap;
use std::{collections::HashMap, path::Path};
use walkdir::WalkDir;

use crate::{
    git_indexer::GitIndexer,
    index::{FileToWordPos, Index},
    tokenizer::Tokenizer,
};

pub struct Indexer {
    root_dir: String,
}

impl Indexer {
    pub fn new(root_dir: &str) -> Self {
        Indexer {
            root_dir: root_dir.to_string(),
        }
    }

    pub fn index(&self) {
        match git2::Repository::open(Path::new(&self.root_dir)) {
            Ok(repo) => GitIndexer::new(repo).index_history().unwrap(),
            Err(_) => {
                println!("Non Git-directory. Only indexing the current directory.");
                self.index_directory();
            }
        }
    }

    pub fn index_directory(&self) -> Index {
        let path = Path::new(&self.root_dir);

        let mut num_indexed_files: i64 = 0;

        let mut files = vec![];

        let mut word_to_bitmap: HashMap<String, RoaringBitmap> = HashMap::new();
        let mut file_to_word_pos: FileToWordPos = HashMap::new();

        let bar = ProgressBar::new(10000);
        bar.set_style(ProgressStyle::default_bar().template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg}",
        ).unwrap());

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
                bar.set_message(entry.path().to_str().unwrap().to_owned());

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

                bar.inc(1);
            }

            if num_indexed_files > 10000 {
                break;
            }
        }

        bar.finish();

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
