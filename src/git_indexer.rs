use std::{cell::Cell, collections::HashMap};

use git2::{Delta, ObjectType, Repository, Sort, Tree, TreeWalkResult};
use roaring::RoaringBitmap;

use crate::tokenizer::Tokenizer;

pub type CommitIndex = usize;

pub struct GitIndexer {
    repo: Repository,
    commit_index_to_commit_id: Vec<[u8; 20]>,
    commit_id_to_commit_index: HashMap<[u8; 20], CommitIndex>,
}

impl GitIndexer {
    pub fn new(repo: Repository) -> Self {
        Self {
            repo,
            commit_index_to_commit_id: Vec::new(),
            commit_id_to_commit_index: HashMap::new(),
        }
    }

    pub fn index_history(&mut self) -> Result<(), git2::Error> {
        let mut revwalk = self.repo.revwalk()?;

        revwalk.push_head()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;

        let mut last_tree: Option<Tree> = None;
        let mut file_name_to_id = HashMap::new();

        for old_result in revwalk {
            let old = old_result?;
            let commit = self.repo.find_commit(old)?;

            println!("Commit {}", commit.id());

            let mut commit_id = [0u8; 20];
            commit_id.copy_from_slice(commit.id().as_bytes());

            self.commit_index_to_commit_id.push(commit_id);
            self.commit_id_to_commit_index
                .insert(commit_id, self.commit_index_to_commit_id.len() - 1);

            let tree = commit.tree()?;

            let mut added_files = Vec::new();
            let mut deleted_files = Vec::new();

            if let Some(ref prev_tree) = last_tree {
                let diff = self
                    .repo
                    .diff_tree_to_tree(Some(prev_tree), Some(&tree), None)?;

                let skip_file = Cell::new(false);
                let result = diff.foreach(
                    &mut |delta, _| {
                        match delta.status() {
                            Delta::Added => {
                                if let Some(path) = delta.new_file().path() {
                                    println!("Added {}", path.display());
                                    added_files.push(path.to_owned());
                                    skip_file.set(true);
                                }
                            }
                            Delta::Deleted => {
                                if let Some(path) = delta.old_file().path() {
                                    println!("Delted {}", path.display());
                                    deleted_files.push(path.to_owned());
                                    skip_file.set(true);
                                }
                            }
                            _ => {
                                skip_file.set(false);
                            }
                        }
                        true
                    },
                    Some(&mut |_delta, _binary| true),
                    Some(&mut |_delta, _hunk| true),
                    None,
                );
                println!("{:?}", result);

                last_tree = Some(tree);
            } else {
                last_tree = Some(tree);
                self.index_tree(&0, last_tree.as_ref().unwrap(), &mut file_name_to_id)
                    .unwrap();
            }
        }

        Ok(())
    }

    fn index_tree(
        &self,
        commit_id: &CommitIndex,
        tree: &Tree,
        file_name_to_id: &mut HashMap<String, usize>,
    ) -> Result<(), git2::Error> {
        let mut word_to_bitmap = HashMap::new();
        let mut file_to_word_pos = HashMap::new();

        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if entry.kind() == Some(ObjectType::Blob) {
                if let Some(name) = entry.name() {
                    let object_id = entry.id();
                    let blob = match self.repo.find_blob(object_id) {
                        Ok(blob) => blob,
                        Err(_) => return TreeWalkResult::Ok,
                    };

                    let content = std::str::from_utf8(blob.content()).unwrap();
                    GitIndexer::index_file(
                        commit_id,
                        &mut word_to_bitmap,
                        &mut file_to_word_pos,
                        file_name_to_id,
                        &format!("{}{}", root, name),
                        content,
                    );
                }
            }

            TreeWalkResult::Ok
        })
    }

    fn index_file(
        commit_id: &CommitIndex,
        word_to_bitmap: &mut HashMap<String, RoaringBitmap>,
        file_to_word_pos: &mut HashMap<usize, Document>,
        file_name_to_id: &mut HashMap<String, usize>,
        file_name: &str,
        content: &str,
    ) {
        let tokenizer_result = Tokenizer::split_to_words(content);

        let file_id = match file_name_to_id.get(file_name) {
            Some(id) => *id,
            None => {
                let file_id = file_name_to_id.len();
                file_name_to_id.insert(file_name.to_owned(), file_id);
                file_id
            }
        };

        for word in tokenizer_result.total_words {
            let bitmap = word_to_bitmap.entry(word.to_string()).or_default();
            bitmap.insert(file_id as u32);
        }

        file_to_word_pos.insert(
            file_id,
            Document::new(*commit_id, tokenizer_result.word_pos),
        );
    }
}

pub struct WordIndex {
    line_number: usize,
    col_number: usize,
    commit_start: CommitIndex,
    commit_end: Option<CommitIndex>,
}

pub struct Document {
    words: HashMap<String, Vec<WordIndex>>,

    // For each word, we track whether the specific word was included in the specific commit.
    word_commit_inclutivity: HashMap<String, RoaringBitmap>,

    // Whether the word_commit_inclutivity is updated or not.
    should_update_inclutivity: bool,
}

impl Document {
    fn new(commit_index: CommitIndex, words: HashMap<&str, Vec<(usize, usize)>>) -> Self {
        Self {
            words: words
                .into_iter()
                .map(|(k, v)| {
                    (
                        k.to_owned(),
                        v.into_iter()
                            .map(|(line_num, col_num)| WordIndex {
                                line_number: line_num,
                                col_number: col_num,
                                commit_start: commit_index,
                                commit_end: None,
                            })
                            .collect(),
                    )
                })
                .collect(),
            word_commit_inclutivity: HashMap::new(),
            should_update_inclutivity: true,
        }
    }
}
