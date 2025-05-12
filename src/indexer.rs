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

            if num_indexed_files > 1000 {
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

        let (all_words, word_pos_map) = Tokenizer::split_to_words(&content);
        for word in all_words {
            let bitmap = word_to_bitmap.entry(word.to_string()).or_default();
            bitmap.insert(file_id as u32);
        }

        file_to_word_pos.insert(
            file_id as usize,
            word_pos_map
                .into_iter()
                .map(|(word, positions)| (word.to_string(), positions))
                .collect(),
        );

        Ok(())
    }
}
