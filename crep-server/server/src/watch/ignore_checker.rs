use std::path::Path;

use ignore::WalkBuilder;

#[derive(PartialEq, Debug)]
struct IgnoreChecker {
    git_ignore_paths: Vec<String>,
    parent_index: Vec<Option<usize>>,
}

impl IgnoreChecker {
    pub fn new(repo_path: &str) -> Self {
        let walk = WalkBuilder::new(Path::new(repo_path)).build();

        let mut git_ignore_paths = Vec::new();
        for result in walk {
            match result {
                Ok(entry) => {
                    if entry.file_name() == ".gitignore" {
                        if let Some(parent) = entry.path().parent() {
                            if let Some(path) = parent.to_str() {
                                git_ignore_paths.push(path.to_owned());
                            }
                        }
                    }
                }
                Err(err) => panic!("{:?}", err),
            }
        }

        IgnoreChecker::from_gitignore_paths(git_ignore_paths)
    }

    fn from_gitignore_paths(mut paths: Vec<String>) -> Self {
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

        Self {
            git_ignore_paths: paths,
            parent_index,
        }
    }
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
            IgnoreChecker::from_gitignore_paths(vec![
                "/b".to_owned(),
                "/c".to_owned(),
                "/a".to_owned(),
                "/d".to_owned()
            ]),
            IgnoreChecker {
                git_ignore_paths: vec![
                    "/a".to_owned(),
                    "/b".to_owned(),
                    "/c".to_owned(),
                    "/d".to_owned()
                ],
                parent_index: vec![None, None, None, None]
            }
        );
    }

    #[test]
    fn nested_paths() {
        assert_eq!(
            IgnoreChecker::from_gitignore_paths(vec![
                "/".to_owned(),
                "/a".to_owned(),
                "/a/b".to_owned(),
                "/a/c".to_owned(),
                "/a/c/b".to_owned(),
                "/b".to_owned(),
                "/b/a".to_owned(),
            ]),
            IgnoreChecker {
                git_ignore_paths: vec![
                    "/".to_owned(),
                    "/a".to_owned(),
                    "/a/b".to_owned(),
                    "/a/c".to_owned(),
                    "/a/c/b".to_owned(),
                    "/b".to_owned(),
                    "/b/a".to_owned(),
                ],
                parent_index: vec![
                    None,
                    Some(0),
                    Some(1),
                    Some(1),
                    Some(3),
                    Some(0),
                    Some(5)
                ]
            }
        );
    }

    #[test]
    fn deeply_nested_path() {
        assert_eq!(
            IgnoreChecker::from_gitignore_paths(vec![
                "/".to_owned(),
                "/a".to_owned(),
                "/a/b/c/d".to_owned(),
                "/a/b/e".to_owned(),
                "/a/c".to_owned(),
                "/a/c/d/e/f".to_owned(),
            ]),
            IgnoreChecker {
                git_ignore_paths: vec![
                    "/".to_owned(),
                    "/a".to_owned(),
                    "/a/b/c/d".to_owned(),
                    "/a/b/e".to_owned(),
                    "/a/c".to_owned(),
                    "/a/c/d/e/f".to_owned(),
                ],
                parent_index: vec![
                    None,
                    Some(0),
                    Some(1),
                    Some(1),
                    Some(1),
                    Some(4)
                ]
            }
        );
    }

    #[test]
    fn no_ignores() {
        assert_eq!(
            IgnoreChecker::from_gitignore_paths(vec![]),
            IgnoreChecker {
                git_ignore_paths: vec![],
                parent_index: vec![]
            }
        );
    }
}
