use roaring::RoaringBitmap;
use std::{collections::HashMap, path::Path};

use crate::index::Index;

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

        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.is_file() {
                Indexer::index_file(&mut word_to_bitmap, &path, num_indexed_files);

                files.push(path.to_string_lossy().to_string());
                num_indexed_files += 1;
            }

            if num_indexed_files > 1000 {
                break;
            }
        }

        Index::new(files, word_to_bitmap)
    }

    fn index_file(word_to_bitmap: &mut HashMap<String, RoaringBitmap>, path: &Path, file_id: i64) {
        let content = std::fs::read_to_string(path).unwrap();

        let mut word_to_lines = HashMap::<String, Vec<i32>>::new();
        for (line_num, line) in content.lines().enumerate() {
            let words = line.split_whitespace();
            for word in words {
                std::collections::hash_map::Entry::or_insert_with(
                    word_to_lines.entry(word.to_string()),
                    Vec::new,
                )
                .push(line_num as i32);
            }
        }

        for word in word_to_lines.keys() {
            let bitmap = word_to_bitmap.entry(word.to_string()).or_default();
            bitmap.insert(file_id as u32);
        }
    }
}
