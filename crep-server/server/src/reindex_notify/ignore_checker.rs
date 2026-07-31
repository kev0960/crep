use std::path::Path;

use ignore::WalkBuilder;
use ignore::gitignore::Gitignore;
use ignore::gitignore::GitignoreBuilder;

#[derive(Debug)]
pub struct IgnoreChecker {
    git_ignores: Vec<Gitignore>,
    git_ignore_paths: Vec<String>,
}

impl IgnoreChecker {
    pub fn new(repo_path: &str) -> Self {
        let walk = WalkBuilder::new(Path::new(repo_path)).hidden(false).build();

        let mut git_ignore_paths = Vec::new();
        for result in walk {
            match result {
                Ok(entry) => {
                    if entry.file_name() == ".gitignore"
                        && let Some(parent) = entry.path().parent()
                        && let Some(path) = parent.to_str()
                    {
                        git_ignore_paths.push(path.to_owned());
                    }
                }
                Err(err) => panic!("{:?}", err),
            }
        }

        let (git_ignore_paths, parent_index) =
            build_gitignore_parent_index_array(git_ignore_paths);

        let mut git_ignores = vec![];

        // Now based on the parent index, create the pre-built git ignores.
        for (index, _) in git_ignore_paths.iter().enumerate() {
            let mut ignore_paths = vec![];

            let mut current_index = Some(index);
            while current_index.is_some() {
                let current = current_index.unwrap();
                ignore_paths.push(git_ignore_paths[current].as_str());
                current_index = parent_index[current];
            }

            ignore_paths.reverse();

            let mut builder = GitignoreBuilder::new(repo_path);
            for path in ignore_paths {
                if let Some(err) =
                    builder.add(Path::new(path).join(".gitignore"))
                {
                    panic!("Failed to add a gitignore {}", err);
                }
            }

            git_ignores.push(builder.build().unwrap());
        }

        Self {
            git_ignore_paths,
            git_ignores,
        }
    }

    pub fn is_ignored<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();
        let matching_path = self
            .git_ignore_paths
            .binary_search_by(|probe| cmp_paths(probe, path.to_str().unwrap()));

        match matching_path {
            Ok(res) => self
                .git_ignores
                .get(res)
                .unwrap()
                .matched_path_or_any_parents(path, path.is_dir())
                .is_ignore(),
            Err(res) => {
                if res == 0 {
                    return false;
                }

                self.git_ignores
                    .get(res - 1)
                    .unwrap()
                    .matched_path_or_any_parents(path, path.is_dir())
                    .is_ignore()
            }
        }
    }
}

// Returns the .gitignore paths (Vec<String>) and the index to the parent (Vec<Option<usize>>) for
// each path. For example, if there is a gitignore file on "/a" and "/a/b", then "/a" is the
// "parent" of "/a/b" in terms of searching for the ignore matches.
fn build_gitignore_parent_index_array(
    mut paths: Vec<String>,
) -> (Vec<String>, Vec<Option<usize>>) {
    paths.sort_by(|a, b| cmp_paths(a, b));

    // Now construct the search table.
    let mut parent_index: Vec<Option<usize>> = vec![None; paths.len()];

    for (index, path) in paths.iter().enumerate() {
        if index == 0 {
            continue;
        }

        // Find the "parent"
        let mut current = Path::new(path).parent().unwrap();

        let parent_idx = loop {
            let parent_idx = &paths[0..index].binary_search_by(|probe| {
                cmp_paths(probe, current.to_str().unwrap())
            });

            match parent_idx {
                Ok(idx) => break Some(*idx),
                Err(_) => {
                    let parent = current.parent();
                    if parent.is_none() {
                        break None;
                    }

                    current = parent.unwrap();
                    continue;
                }
            }
        };

        parent_index[index] = parent_idx;
    }

    (paths, parent_index)
}

fn cmp_paths(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;

    let mut ita = a.bytes();
    let mut itb = b.bytes();

    loop {
        match (ita.next(), itb.next()) {
            (None, None) => return Equal,
            (None, Some(_)) => return Less,
            (Some(_), None) => return Greater,
            (Some(ba), Some(bb)) => {
                if ba == bb {
                    continue;
                }

                // custom rule: '/' is *greater* than all other characters
                let ra = if ba == b'/' { u8::MAX } else { ba };
                let rb = if bb == b'/' { u8::MAX } else { bb };

                return ra.cmp(&rb);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn no_nested_paths() {
        assert_eq!(
            build_gitignore_parent_index_array(vec![
                "/b".to_owned(),
                "/c".to_owned(),
                "/a".to_owned(),
                "/d".to_owned()
            ]),
            (
                vec![
                    "/a".to_owned(),
                    "/b".to_owned(),
                    "/c".to_owned(),
                    "/d".to_owned()
                ],
                vec![None, None, None, None],
            )
        );
    }

    #[test]
    fn nested_paths() {
        assert_eq!(
            build_gitignore_parent_index_array(vec![
                "/".to_owned(),
                "/a".to_owned(),
                "/a/b".to_owned(),
                "/a/c".to_owned(),
                "/a/c/b".to_owned(),
                "/b".to_owned(),
                "/b/a".to_owned(),
            ]),
            (
                vec![
                    "/".to_owned(),
                    "/a".to_owned(),
                    "/a/b".to_owned(),
                    "/a/c".to_owned(),
                    "/a/c/b".to_owned(),
                    "/b".to_owned(),
                    "/b/a".to_owned(),
                ],
                vec![
                    None,
                    Some(0),
                    Some(1),
                    Some(1),
                    Some(3),
                    Some(0),
                    Some(5)
                ],
            )
        );
    }

    #[test]
    fn deeply_nested_path() {
        assert_eq!(
            build_gitignore_parent_index_array(vec![
                "/".to_owned(),
                "/a".to_owned(),
                "/a/b/c/d".to_owned(),
                "/a/b/e".to_owned(),
                "/a/c".to_owned(),
                "/a/c/d/e/f".to_owned(),
            ]),
            (
                vec![
                    "/".to_owned(),
                    "/a".to_owned(),
                    "/a/b/c/d".to_owned(),
                    "/a/b/e".to_owned(),
                    "/a/c".to_owned(),
                    "/a/c/d/e/f".to_owned(),
                ],
                vec![None, Some(0), Some(1), Some(1), Some(1), Some(4)],
            )
        );
    }

    #[test]
    fn no_ignores() {
        assert_eq!(
            build_gitignore_parent_index_array(vec![]),
            (vec![], vec![],)
        );
    }
}
