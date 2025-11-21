use std::path::Path;

use git2::Oid;
use git2::Repository;

use crate::index::git_indexer::CommitIndex;
use crate::index::git_indexer::FileId;
use crate::search::result::search_result::RepoReader;

pub struct SimpleRepoReader<'r, 'i> {
    pub repo: &'r Repository,
    pub file_id_to_path: &'i [String],
    pub commit_index_to_commit_id: &'i [[u8; 20]],
}

impl<'r, 'i> RepoReader for SimpleRepoReader<'r, 'i> {
    fn read_file_at_commit(
        &self,
        commit_id: CommitIndex,
        file_id: FileId,
    ) -> anyhow::Result<Option<(/*file path*/ String, /*content*/ String)>>
    {
        let file_path = self.file_id_to_path.get(file_id).unwrap();
        let commit =
            Oid::from_bytes(&self.commit_index_to_commit_id[commit_id])?;

        let commit = self.repo.find_commit(commit)?;
        let tree = commit.tree()?;

        let entry = tree.get_path(Path::new(&file_path))?;
        let object = entry.to_object(self.repo)?;
        if let Some(blob) = object.as_blob() {
            Ok(Some((
                file_path.to_owned(),
                String::from_utf8_lossy(blob.content()).to_string(),
            )))
        } else {
            Ok(None)
        }
    }
}
