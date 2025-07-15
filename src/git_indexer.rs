use std::{cell::RefCell, collections::HashMap};

use anyhow::Result;
use git2::{Delta, ObjectType, Repository, Sort, Tree, TreeWalkResult};
use roaring::RoaringBitmap;

use crate::{
    git::diff::{self, FileDiffTracker},
    tokenizer::Tokenizer,
};

pub type CommitIndex = usize;
type FileId = usize;

pub struct GitIndexer {
    commit_index_to_commit_id: Vec<[u8; 20]>,
    commit_id_to_commit_index: HashMap<[u8; 20], CommitIndex>,
    file_name_to_id: HashMap<String, FileId>,
    file_id_to_diff_tracker: HashMap<FileId, FileDiffTracker>,
}

#[derive(Debug)]
struct CurrentGitDiffFile {
    current_file_id: FileId,
    status: Delta,
}

impl GitIndexer {
    pub fn new() -> Self {
        Self {
            commit_index_to_commit_id: Vec::new(),
            commit_id_to_commit_index: HashMap::new(),
            file_name_to_id: HashMap::new(),
            file_id_to_diff_tracker: HashMap::new(),
        }
    }

    fn get_file_id_insert_if_missing(&mut self, file_full_path: &str) -> FileId {
        match self.file_name_to_id.get(file_full_path) {
            Some(id) => *id,
            None => {
                let file_id = self.file_name_to_id.len();
                self.file_name_to_id
                    .insert(file_full_path.to_owned(), file_id);
                file_id
            }
        }
    }

    pub fn index_history(&mut self, repo: Repository) -> Result<()> {
        let mut revwalk = repo.revwalk()?;

        revwalk.push_head()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;

        let mut last_tree: Option<Tree> = None;

        for old_result in revwalk {
            let old = old_result?;
            let commit = repo.find_commit(old)?;

            println!("Commit {}", commit.id());

            let mut commit_id = [0u8; 20];
            commit_id.copy_from_slice(commit.id().as_bytes());

            self.commit_index_to_commit_id.push(commit_id);

            let commit_index = self.commit_index_to_commit_id.len() - 1;
            self.commit_id_to_commit_index
                .insert(commit_id, commit_index);

            let tree = commit.tree()?;

            let mut opts = git2::DiffOptions::new();
            opts.context_lines(0);

            if let Some(ref prev_tree) = last_tree {
                self.index_diff(&tree, prev_tree, &repo, &commit_index)?;
                last_tree = Some(tree);
            } else {
                last_tree = Some(tree);
                self.index_tree(&0, last_tree.as_ref().unwrap(), &repo)?;
            }
        }

        let file_id_to_name = self
            .file_name_to_id
            .iter()
            .map(|(k, v)| (*v, k))
            .collect::<HashMap<_, _>>();

        for (file_id, diff_tracker) in self.file_id_to_diff_tracker.iter() {
            println!(
                "File name: {:?} {:?}",
                file_id_to_name.get(file_id),
                diff_tracker
            );
        }

        Ok(())
    }

    fn index_diff(
        &mut self,
        current_tree: &Tree,
        prev_tree: &Tree,
        repo: &Repository,
        commit_id: &CommitIndex,
    ) -> Result<()> {
        let mut opts = git2::DiffOptions::new();
        opts.context_lines(0);

        let diff = repo.diff_tree_to_tree(Some(prev_tree), Some(current_tree), Some(&mut opts))?;

        let current_diff_file: RefCell<Option<CurrentGitDiffFile>> = RefCell::new(None);

        // TODO: Generate GitDelta struct. Store Deltas to this vec (from for_each)
        // We need to process Hunks in reverse to handle prefix sum properly.
        //
        let file_delta = RefCell::new(Vec::<GitDelta>::new());

        diff.foreach(
            &mut |delta, _| {
                let mut file_delta = file_delta.borrow_mut();
                let mut current_diff_file = current_diff_file.borrow_mut();

                if !file_delta.is_empty() {
                    println!("{:?}", file_delta);
                    self.index_git_delta(&current_diff_file, &file_delta, commit_id)
                        .unwrap();
                    file_delta.clear();
                }

                match delta.status() {
                    Delta::Modified => {
                        if let Some(path) = delta.old_file().path() {
                            let file_id =
                                self.get_file_id_insert_if_missing(path.to_str().unwrap());

                            *current_diff_file = Some(CurrentGitDiffFile {
                                current_file_id: file_id,
                                status: delta.status(),
                            });
                        }
                    }
                    Delta::Deleted => {
                        if let Some(path) = delta.old_file().path() {
                            let file_id =
                                self.get_file_id_insert_if_missing(path.to_str().unwrap());

                            *current_diff_file = Some(CurrentGitDiffFile {
                                current_file_id: file_id,
                                status: delta.status(),
                            });
                        }
                    }
                    Delta::Added => {
                        if let Some(path) = delta.new_file().path() {
                            let file_id =
                                self.get_file_id_insert_if_missing(path.to_str().unwrap());

                            *current_diff_file = Some(CurrentGitDiffFile {
                                current_file_id: file_id,
                                status: delta.status(),
                            });
                        }
                    }
                    _ => {}
                }

                true
            },
            None,
            Some(&mut |_delta, hunk| {
                if current_diff_file.borrow().as_ref().unwrap().status == Delta::Modified {
                    println!("Hunk: {:?}", String::from_utf8_lossy(hunk.header()));
                }

                file_delta.borrow_mut().push(GitDelta {
                    prev_line_start_num: hunk.old_start(),
                    prev_line_count: hunk.old_lines(),
                    new_line_start_num: hunk.new_start(),
                    new_line_count: hunk.new_lines(),
                });

                true
            }),
            None,
        )?;

        let file_delta = file_delta.borrow_mut();
        if !file_delta.is_empty() {
            self.index_git_delta(&current_diff_file.borrow_mut(), &file_delta, commit_id)
                .unwrap();
        }

        Ok(())
    }

    fn index_tree(
        &mut self,
        commit_id: &CommitIndex,
        tree: &Tree,
        repo: &Repository,
    ) -> Result<()> {
        let mut word_to_bitmap = HashMap::new();
        let mut file_to_word_pos = HashMap::new();

        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if entry.kind() == Some(ObjectType::Blob) {
                if let Some(name) = entry.name() {
                    let object_id = entry.id();
                    let blob = match repo.find_blob(object_id) {
                        Ok(blob) => blob,
                        Err(_) => return TreeWalkResult::Ok,
                    };

                    let content = std::str::from_utf8(blob.content()).unwrap();
                    let file_name = &format!("{}{}", root, name);
                    println!("FILE NAME {}", file_name);

                    let file_id = match self.file_name_to_id.get(file_name) {
                        Some(id) => *id,
                        None => {
                            let file_id = self.file_name_to_id.len();
                            self.file_name_to_id.insert(file_name.to_owned(), file_id);
                            file_id
                        }
                    };

                    self.index_file(
                        commit_id,
                        file_id,
                        &mut word_to_bitmap,
                        &mut file_to_word_pos,
                        content,
                    );
                }
            }

            TreeWalkResult::Ok
        })?;

        println!("{:?}", self.file_id_to_diff_tracker);

        Ok(())
    }

    fn index_file(
        &mut self,
        commit_id: &CommitIndex,
        file_id: FileId,
        word_to_bitmap: &mut HashMap<String, RoaringBitmap>,
        file_to_word_pos: &mut HashMap<usize, Document>,
        content: &str,
    ) {
        let tokenizer_result = Tokenizer::split_to_words(content);

        for word in tokenizer_result.total_words {
            let bitmap = word_to_bitmap.entry(word.to_string()).or_default();
            bitmap.insert(file_id as u32);
        }

        file_to_word_pos.insert(
            file_id,
            Document::new(*commit_id, tokenizer_result.word_pos),
        );

        self.file_id_to_diff_tracker.insert(
            file_id,
            FileDiffTracker::new(*commit_id, content.lines().count()),
        );
    }

    fn index_git_delta(
        &mut self,
        file: &Option<CurrentGitDiffFile>,
        hunks: &[GitDelta],
        commit_id: &CommitIndex,
    ) -> Result<(), String> {
        if file.is_none() {
            return Err("file should not be empty".to_owned());
        }

        let file_id = file.as_ref().unwrap().current_file_id;
        let status = file.as_ref().unwrap().status;

        match status {
            Delta::Modified => {
                let diff_tracker = self.file_id_to_diff_tracker.get_mut(&file_id);
                if diff_tracker.is_none() {
                    return Err(format!(""));
                }

                let diff_tracker = diff_tracker.unwrap();
                for hunk in hunks.iter().rev() {
                    if hunk.prev_line_start_num == 0 {
                        assert!(hunk.prev_line_count == 0);
                    }

                    if hunk.prev_line_start_num > 0 {
                        diff_tracker.delete_lines(
                            hunk.prev_line_start_num as usize - 1,
                            hunk.prev_line_count as usize,
                        );
                    }

                    diff_tracker.add_lines(
                        hunk.prev_line_start_num as usize,
                        hunk.new_line_count as usize,
                        *commit_id,
                    );
                }
            }
            Delta::Added => {
                if hunks.len() != 1 {
                    return Err(format!(
                        "new file should have one hunk - {:?} {:?}",
                        file, hunks
                    ));
                }

                if hunks[0].new_line_start_num != 1 {
                    return Err(format!(
                        "new file hunk does not start with 1? - {:?} {:?}",
                        file, hunks
                    ));
                }

                if hunks[0].prev_line_start_num != 0 {
                    return Err(format!(
                        "prev file hunk does not start with 0? - {:?} {:?}",
                        file, hunks
                    ));
                }

                self.file_id_to_diff_tracker.insert(
                    file_id,
                    FileDiffTracker::new(*commit_id, hunks[0].new_line_count as usize),
                );
            }
            Delta::Deleted => {
                if hunks.len() != 1 {
                    return Err(format!(
                        "deleted file should have one hunk - {:?} {:?}",
                        file, hunks
                    ));
                }

                if hunks[0].new_line_start_num != 0 {
                    return Err(format!(
                        "delete file hunk does not start with 0? - {:?} {:?}",
                        file, hunks
                    ));
                }

                self.file_id_to_diff_tracker.remove(&file_id);
            }
            _ => {}
        }

        Ok(())
    }
}

#[derive(Debug)]
struct GitDelta {
    prev_line_start_num: u32,
    prev_line_count: u32,

    new_line_start_num: u32,
    new_line_count: u32,
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
