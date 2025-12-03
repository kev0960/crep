use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use ahash::AHashSet;
use anyhow::anyhow;
use git2::{Repository, Status, StatusOptions};

pub struct NotCommitedFilesIndexer {
    created_files: AHashSet<String>,
    modified_files: AHashSet<String>,
    removed_files: AHashSet<String>,

    root_path: PathBuf,
    repo: Arc<Mutex<Repository>>,
}

impl NotCommitedFilesIndexer {
    pub fn new(root_path: &Path) -> anyhow::Result<Self> {
        let repo = Repository::open(root_path)?;

        Ok(Self {
            created_files: AHashSet::new(),
            modified_files: AHashSet::new(),
            removed_files: AHashSet::new(),
            root_path: PathBuf::from(root_path),
            repo: Arc::new(Mutex::new(repo)),
        })
    }

    pub fn reindex_files(&mut self, path: &[PathBuf]) -> anyhow::Result<()> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);

        let mut interested_paths = AHashSet::from_iter(path);
        let repo = self.repo.lock().unwrap();
        let status = repo.statuses(Some(&mut opts))?;

        for entry in status.iter() {
            let path = PathBuf::from(entry.path().unwrap());
            if !interested_paths.contains(&path) {
                continue;
            }

            interested_paths.remove(&path);

            let index_verdict = classify_change(entry.status());
            let path_str = path
                .to_str()
                .ok_or(anyhow!("Path is not UTF-8"))?
                .to_owned();

            match index_verdict {
                IndexVerdict::New => {
                    self.created_files.insert(path_str);
                }
                IndexVerdict::Modify => {
                    self.modified_files.insert(path_str);
                }
                IndexVerdict::Remove => {
                    self.removed_files.insert(path_str);
                }
                IndexVerdict::Ignore => {
                    continue;
                }
            }
        }

        // If there is some path but not showing up on the status, then it means that the file is
        // no longer modified.
        for path in interested_paths {
            let path_str = path
                .to_str()
                .ok_or(anyhow!("Path is not UTF-8"))?
                .to_owned();

            self.created_files.remove(&path_str);
            self.modified_files.remove(&path_str);
            self.removed_files.remove(&path_str);
        }

        Ok(())
    }

    pub fn get_file_content(
        &self,
        file_path: &str,
    ) -> anyhow::Result<FileContent> {
        let full_path = Path::new(&self.root_path).join(file_path);

        if self.removed_files.contains(file_path) {
            return Ok(FileContent::Deleted);
        }

        if self.created_files.contains(file_path) {
            Ok(FileContent::Created(read_file(&full_path)?))
        } else if self.modified_files.contains(file_path) {
            Ok(FileContent::Modified(read_file(&full_path)?))
        } else {
            Ok(FileContent::NotChanged)
        }
    }
}

fn read_file(file_path: &Path) -> anyhow::Result<String> {
    let mut file = File::open(file_path)?;

    let mut content = String::new();
    file.read_to_string(&mut content)?;

    Ok(content)
}

pub enum FileContent {
    Created(String),
    Modified(String),
    Deleted,
    NotChanged,
}

pub enum IndexVerdict {
    New,
    Modify,
    Remove,
    Ignore,
}

fn classify_change(status: Status) -> IndexVerdict {
    if status.is_conflicted() || status.is_ignored() {
        return IndexVerdict::Ignore;
    }

    // Untracked or added
    if status.is_wt_new() || status.is_index_new() {
        return IndexVerdict::New;
    }

    // Removed (either staged or unstaged delete)
    if status.is_index_deleted() || status.is_wt_deleted() {
        return IndexVerdict::Remove;
    }

    // Modified (index or worktree)
    if status.is_index_modified()
        || status.is_wt_modified()
        || status.is_index_renamed()
        || status.is_wt_renamed()
        || status.is_index_typechange()
        || status.is_wt_typechange()
    {
        return IndexVerdict::Modify;
    }

    // Do not index rest of cases.
    IndexVerdict::Ignore
}
