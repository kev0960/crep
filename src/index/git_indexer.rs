use std::{cell::RefCell, collections::HashMap};

use crate::{
    git::diff::{FileDiffTracker, LineDeleteResult},
    tokenizer::{Tokenizer, WordPosition},
};
use anyhow::Result;
use git2::{Delta, ObjectType, Repository, Sort, Tree, TreeWalkResult};
use roaring::RoaringBitmap;

use super::document::{Document, WordKey};

pub type CommitIndex = usize;
pub type FileId = usize;

pub struct GitIndexer {
    pub commit_index_to_commit_id: Vec<[u8; 20]>,
    pub commit_id_to_commit_index: HashMap<[u8; 20], CommitIndex>,

    file_name_to_id: HashMap<String, FileId>,
    pub file_id_to_name: Vec<String>,
    file_id_to_diff_tracker: HashMap<FileId, FileDiffTracker>,

    pub file_id_to_document: HashMap<FileId, Document>,

    // RoaringBitmap is set if the corresponding file id contains the word.
    pub word_to_file_id_ever_contained: HashMap<String, RoaringBitmap>,
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
            file_id_to_name: Vec::new(),
            file_id_to_diff_tracker: HashMap::new(),
            file_id_to_document: HashMap::new(),
            word_to_file_id_ever_contained: HashMap::new(),
        }
    }

    fn get_file_id_insert_if_missing(
        &mut self,
        file_full_path: &str,
    ) -> FileId {
        match self.file_name_to_id.get(file_full_path) {
            Some(id) => *id,
            None => {
                let file_id = self.file_name_to_id.len();
                self.file_name_to_id
                    .insert(file_full_path.to_owned(), file_id);
                self.file_id_to_name.push(file_full_path.to_owned());
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

            for (file_id, diff_tracker) in self.file_id_to_diff_tracker.iter() {
                println!(
                    "File name: {:?} {:?}",
                    self.file_id_to_name.get(*file_id),
                    diff_tracker
                );
            }
        }

        Ok(())
    }

    fn index_diff(
        &mut self,
        current_tree: &Tree,
        prev_tree: &Tree,
        repo: &Repository,
        commit_index: &CommitIndex,
    ) -> Result<()> {
        let mut opts = git2::DiffOptions::new();
        opts.context_lines(0);

        let diff = repo.diff_tree_to_tree(
            Some(prev_tree),
            Some(current_tree),
            Some(&mut opts),
        )?;

        let current_diff_file: RefCell<Option<CurrentGitDiffFile>> =
            RefCell::new(None);

        let file_delta = RefCell::new(Vec::<GitDelta>::new());

        diff.foreach(
            &mut |delta, _| {
                let mut file_delta = file_delta.borrow_mut();
                let mut current_diff_file = current_diff_file.borrow_mut();

                if !file_delta.is_empty() {
                    self.index_git_delta(
                        &current_diff_file,
                        &file_delta,
                        commit_index,
                    )
                    .unwrap();
                    file_delta.clear();
                }

                match delta.status() {
                    Delta::Modified => {
                        if let Some(path) = delta.old_file().path() {
                            let file_id = self.get_file_id_insert_if_missing(
                                path.to_str().unwrap(),
                            );

                            *current_diff_file = Some(CurrentGitDiffFile {
                                current_file_id: file_id,
                                status: delta.status(),
                            });
                        }
                    }
                    Delta::Deleted => {
                        if let Some(path) = delta.old_file().path() {
                            let file_id = self.get_file_id_insert_if_missing(
                                path.to_str().unwrap(),
                            );

                            *current_diff_file = Some(CurrentGitDiffFile {
                                current_file_id: file_id,
                                status: delta.status(),
                            });
                        }
                    }
                    Delta::Added => {
                        if let Some(path) = delta.new_file().path() {
                            let file_id = self.get_file_id_insert_if_missing(
                                path.to_str().unwrap(),
                            );

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
                file_delta.borrow_mut().push(GitDelta {
                    prev_line_start_num: hunk.old_start(),
                    prev_line_count: hunk.old_lines(),
                    new_line_start_num: hunk.new_start(),
                    new_line_count: hunk.new_lines(),
                    added_lines: Vec::with_capacity(hunk.new_lines() as usize),
                    deleted_lines: Vec::with_capacity(hunk.old_lines() as usize),
                });

                true
            }),
            Some(&mut |_, _, line| {
                let current_diff_file = current_diff_file.borrow();

                // No need to handle Delte::Removed case.
                if let Some(current_diff_file) = current_diff_file.as_ref()
                    && !(current_diff_file.status == Delta::Modified
                        || current_diff_file.status == Delta::Added) {
                    return true;
                }

                let mut file_delta = file_delta.borrow_mut();
                if line.origin() == '+' {
                    file_delta.last_mut().unwrap().added_lines.push(
                        std::str::from_utf8(line.content())
                            .unwrap_or("<invalid utf8>")
                            .to_owned(),
                    );
                } else if line.origin() == '-' {
                    file_delta.last_mut().unwrap().deleted_lines.push(
                        std::str::from_utf8(line.content())
                            .unwrap_or("<invalid utf8>")
                            .to_owned(),
                    );
                }

                true
            }),
        )?;

        let file_delta = file_delta.borrow_mut();
        if !file_delta.is_empty() {
            self.index_git_delta(
                &current_diff_file.borrow_mut(),
                &file_delta,
                commit_index,
            )
            .unwrap();
        }

        Ok(())
    }

    fn index_tree(
        &mut self,
        commit_index: &CommitIndex,
        tree: &Tree,
        repo: &Repository,
    ) -> Result<()> {
        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if entry.kind() == Some(ObjectType::Blob) {
                if let Some(name) = entry.name() {
                    let object_id = entry.id();
                    let blob = match repo.find_blob(object_id) {
                        Ok(blob) => blob,
                        Err(_) => return TreeWalkResult::Ok,
                    };

                    let content = std::str::from_utf8(blob.content()).unwrap();
                    let file_name = &format!("{root}{name}");

                    let file_id = self.get_file_id_insert_if_missing(file_name);
                    self.add_new_lines(
                        *commit_index,
                        file_id,
                        /*prev_line_start=*/ 0,
                        /*new_line_start=*/ 0,
                        &content
                            .lines()
                            .map(|line| line.to_string())
                            .collect::<Vec<String>>(),
                    );
                }
            }

            TreeWalkResult::Ok
        })?;

        Ok(())
    }

    fn index_git_delta(
        &mut self,
        file: &Option<CurrentGitDiffFile>,
        hunks: &[GitDelta],
        commit_index: &CommitIndex,
    ) -> Result<(), String> {
        if file.is_none() {
            return Err("file should not be empty".to_owned());
        }

        let file_id = file.as_ref().unwrap().current_file_id;
        let status = file.as_ref().unwrap().status;

        match status {
            Delta::Modified => {
                for hunk in hunks.iter().rev() {
                    if hunk.prev_line_start_num == 0 {
                        assert!(hunk.prev_line_count == 0);
                    }

                    if hunk.new_line_start_num == 0 {
                        assert!(hunk.new_line_count == 0);
                    }

                    if hunk.prev_line_count > 0 {
                        // Hunk uses 1-based line number and the deleted line
                        // number "includes" the deleted line. E.g. if
                        // prev_line_start_num is 1, then it means the first
                        // line is removed.
                        //
                        // Hence we have to deduct 1 since the indexes are zero
                        // based.
                        self.delete_lines(
                            *commit_index,
                            file_id,
                            hunk.prev_line_start_num as usize - 1,
                            &hunk.deleted_lines,
                        );

                        if hunk.new_line_count > 0 {
                            // diff_tracker.add_lines(
                            //     hunk.prev_line_start_num as usize - 1,
                            //     hunk.new_line_count as usize,
                            //     (*commit_index, (hunk.new_line_start_num - 1) as usize),
                            // );

                            self.add_new_lines(
                                *commit_index,
                                file_id,
                                (hunk.prev_line_start_num - 1) as usize,
                                (hunk.new_line_start_num - 1) as usize,
                                &hunk.added_lines,
                            );
                        }
                    } else {
                        // If the "delete" is not happening, then it means only
                        // new line is added. Since the line is added "after"
                        // the 1-based line number, we do not need to deduct 1.
                        self.add_new_lines(
                            *commit_index,
                            file_id,
                            hunk.prev_line_start_num as usize,
                            (hunk.new_line_start_num - 1) as usize,
                            &hunk.added_lines,
                        );
                    }
                }
            }
            Delta::Added => {
                if hunks.len() != 1 {
                    return Err(format!(
                        "new file should have one hunk - {file:?} {hunks:?}",
                    ));
                }

                if hunks[0].new_line_start_num != 1 {
                    return Err(format!(
                        "new file hunk does not start with 1? - {file:?} {hunks:?}",
                    ));
                }

                if hunks[0].prev_line_start_num != 0 {
                    return Err(format!(
                        "prev file hunk does not start with 0? - {file:?} {hunks:?}",
                    ));
                }

                self.add_new_lines(
                    *commit_index,
                    file_id,
                    /*prev_line_start_num=*/ 0,
                    0,
                    &hunks[0].added_lines,
                );
            }
            Delta::Deleted => {
                if hunks.len() != 1 {
                    return Err(format!(
                        "deleted file should have one hunk - {file:?} {hunks:?}",
                    ));
                }

                if hunks[0].new_line_start_num != 0 {
                    return Err(format!(
                        "delete file hunk does not start with 0? - {file:?} {hunks:?}",
                    ));
                }

                self.delete_entire_file(*commit_index, file_id);
            }
            _ => {}
        }

        Ok(())
    }

    // Add a new line at "prev_line_start".
    //
    // New lines are copied from (new_line_start, new_line_count) from the new file.
    fn add_new_lines(
        &mut self,
        commit_index: CommitIndex,
        file_id: FileId,
        prev_line_start: usize,
        new_line_start: usize,
        lines: &[String],
    ) {
        let diff_tracker = self.file_id_to_diff_tracker.get_mut(&file_id);
        if let Some(tracker) = diff_tracker {
            tracker.add_lines(
                prev_line_start,
                lines.len(),
                (commit_index, new_line_start),
            );
        } else {
            self.file_id_to_diff_tracker.insert(
                file_id,
                FileDiffTracker::new(commit_index, lines.len()),
            );
        }

        // Now index those new lines.
        let tokens =
            Tokenizer::split_lines_to_word_line_only(lines, new_line_start)
                .word_pos;
        let word_to_lines = match tokens {
            WordPosition::LineNumOnlyWithDedup(word_to_lines) => word_to_lines,
            _ => panic!(),
        };

        let document = self
            .file_id_to_document
            .entry(file_id)
            .or_insert(Document::new());

        for word in word_to_lines.keys() {
            self.word_to_file_id_ever_contained
                .entry(word.to_string())
                .or_default()
                .insert(file_id as u32);
        }

        document.add_words(commit_index, word_to_lines);
    }

    fn delete_lines(
        &mut self,
        commit_index: CommitIndex,
        file_id: FileId,
        delete_line_start: usize,
        lines: &[String],
    ) {
        let diff_tracker = self.file_id_to_diff_tracker.get_mut(&file_id);
        assert!(diff_tracker.is_some());

        let diff_tracker = diff_tracker.unwrap();
        let delete_result =
            diff_tracker.delete_lines(delete_line_start, lines.len());

        let word_key_for_each_deleted_line =
            flatten_delete_result(&delete_result);

        let document = self.file_id_to_document.get_mut(&file_id);
        assert!(document.is_some());

        let document = document.unwrap();

        let tokens = Tokenizer::split_lines_to_word_line_only(
            lines, /*new_line_start=*/ 0,
        )
        .word_pos;

        let word_to_lines = match tokens {
            WordPosition::LineNumOnlyWithDedup(word_to_lines) => word_to_lines,
            _ => panic!(),
        }
        .into_iter()
        .map(|(word, lines)| {
            (
                word,
                lines
                    .into_iter()
                    .map(|line| word_key_for_each_deleted_line[line])
                    .collect::<Vec<WordKey>>(),
            )
        })
        .collect::<Vec<(&str, Vec<WordKey>)>>();

        document.remove_words(commit_index, &word_to_lines);
    }

    fn delete_entire_file(
        &mut self,
        commit_index: CommitIndex,
        file_id: FileId,
    ) {
        let diff_tracker = self.file_id_to_diff_tracker.get_mut(&file_id);
        assert!(diff_tracker.is_some());

        let diff_tracker = diff_tracker.unwrap();
        diff_tracker.delete_all();

        let document = self.file_id_to_document.get_mut(&file_id);
        assert!(document.is_some());

        let document = document.unwrap();
        document.remove_document(commit_index);
    }
}

fn flatten_delete_result(delete_results: &[LineDeleteResult]) -> Vec<WordKey> {
    let mut delete_result_per_line = Vec::<WordKey>::with_capacity(
        delete_results
            .iter()
            .map(|r| r.start_and_end.1 - r.start_and_end.0)
            .sum(),
    );

    for delete_result in delete_results {
        let (start, end) = delete_result.start_and_end;
        for i in start..end {
            delete_result_per_line.push(WordKey {
                commit_id: delete_result.commit_id,
                line: i,
            })
        }
    }

    delete_result_per_line
}

#[derive(Debug)]
struct GitDelta {
    prev_line_start_num: u32,
    prev_line_count: u32,

    new_line_start_num: u32,
    new_line_count: u32,

    added_lines: Vec<String>,
    deleted_lines: Vec<String>,
}
