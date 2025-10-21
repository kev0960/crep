use ahash::AHashMap;
use ahash::AHashSet;
use fst::Set;
use serde::Deserialize;
use serde::Serialize;
use trigram_hash::trigram_hash::TrigramKey;

use roaring::RoaringBitmap;

use super::git_indexer::CommitIndex;

#[derive(Debug, Eq, Hash, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub struct WordKey {
    pub commit_id: CommitIndex,

    // Line within the commit when the word was first introduced.
    pub line: usize,
}

#[derive(Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WordIndex {
    pub word_history: AHashSet<WordKey>,

    // Whether the specific word is included in a given commit.
    pub commit_inclutivity: RoaringBitmap,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Document {
    pub words: AHashMap<TrigramKey, WordIndex>,

    #[serde(with = "crate::util::serde::fst::fst_set_to_vec::option")]
    pub all_words: Option<Set<Vec<u8>>>,

    pub doc_modified_commits: RoaringBitmap,
}

impl Document {
    pub fn new() -> Self {
        Self {
            words: AHashMap::new(),
            all_words: None,
            doc_modified_commits: RoaringBitmap::new(),
        }
    }

    pub fn add_words(
        &mut self,
        commit_index: CommitIndex,
        words: AHashMap<TrigramKey, Vec<usize>>,
    ) {
        for (word, lines) in words {
            let word_index = self.words.entry(word).or_default();

            for line in lines {
                word_index.word_history.insert(WordKey {
                    commit_id: commit_index,
                    line,
                });
            }

            word_index.commit_inclutivity.insert(commit_index as u32);
        }
        self.doc_modified_commits.insert(commit_index as u32);
    }

    pub fn remove_words(
        &mut self,
        commit_index: CommitIndex,
        words: &[(TrigramKey, Vec<WordKey>)],
    ) {
        for (word, word_keys) in words {
            debug_assert!(!word_keys.is_empty());

            let word_index = self.words.get_mut(word);
            if let Some(word_index) = word_index {
                for word_key in word_keys {
                    word_index.word_history.remove(word_key);
                }
            }
        }

        let modified_words: AHashSet<TrigramKey> =
            words.iter().map(|(word, _)| *word).collect();
        for word in modified_words {
            self.update_commit_inclutivity_after_removal(commit_index, word);
        }

        self.doc_modified_commits.insert(commit_index as u32);
    }

    pub fn remove_document(&mut self, commit_index: CommitIndex) {
        for word_index in self.words.values_mut() {
            if !word_index.word_history.is_empty() {
                let last_enabled_commit = word_index.commit_inclutivity.max();
                word_index.commit_inclutivity.insert_range(
                    last_enabled_commit.unwrap()..((commit_index - 1) as u32),
                );

                word_index.word_history.clear();
            }
        }

        self.doc_modified_commits.insert(commit_index as u32);
    }

    fn update_commit_inclutivity_after_removal(
        &mut self,
        commit_index: CommitIndex,
        word: TrigramKey,
    ) {
        let word_index = self.words.get_mut(&word);
        if word_index.is_none() {
            return;
        }

        let word_index = word_index.unwrap();
        if word_index.word_history.is_empty() {
            // Then commit_index - 1 is the last time that the document contained the word.
            match word_index.commit_inclutivity.max() {
                Some(last_enabled_bit) => {
                    word_index.commit_inclutivity.insert_range(
                        last_enabled_bit..((commit_index - 1) as u32),
                    );
                    word_index.commit_inclutivity.optimize();
                }
                None => {
                    word_index
                        .commit_inclutivity
                        .insert((commit_index - 1) as u32);
                }
            }
        }
    }

    pub fn finalize(&mut self, commit_index: CommitIndex) {
        for index in self.words.values_mut() {
            if index.commit_inclutivity.contains(commit_index as u32) {
                continue;
            }

            if !index.word_history.is_empty() {
                match index.commit_inclutivity.max() {
                    Some(last_enabled_bit) => {
                        index.commit_inclutivity.insert_range(
                            last_enabled_bit..((commit_index + 1) as u32),
                        );
                    }
                    _ => {
                        index.commit_inclutivity.insert(commit_index as u32);
                    }
                }
            }
        }

        let mut keys = self.words.keys().cloned().collect::<Vec<_>>();
        keys.sort();

        self.all_words = Some(Set::from_iter(keys).unwrap());
    }
}

#[cfg(test)]
impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        if self.words != other.words {
            return false;
        }

        if self.doc_modified_commits != other.doc_modified_commits {
            return false;
        }

        match (&self.all_words, &other.all_words) {
            (Some(left), Some(right)) => {
                left.stream().into_strs().unwrap()
                    == right.stream().into_strs().unwrap()
            }
            (None, None) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod document_test {
    use super::*;

    use bincode::serde;

    #[test]
    fn add_words() {
        let words = AHashMap::from([
            ("hi".into(), vec![1, 2]),
            ("hello".into(), vec![1, 3]),
        ]);

        let mut document = Document::new();

        document.add_words(1, words);
        assert_eq!(
            document,
            Document {
                words: AHashMap::from([
                    (
                        "hi".into(),
                        WordIndex {
                            word_history: AHashSet::from_iter([
                                WordKey {
                                    commit_id: 1,
                                    line: 1
                                },
                                WordKey {
                                    commit_id: 1,
                                    line: 2
                                },
                            ]),
                            commit_inclutivity: RoaringBitmap::from([1])
                        }
                    ),
                    (
                        "hello".into(),
                        WordIndex {
                            word_history: AHashSet::from_iter([
                                WordKey {
                                    commit_id: 1,
                                    line: 1
                                },
                                WordKey {
                                    commit_id: 1,
                                    line: 3
                                },
                            ]),
                            commit_inclutivity: RoaringBitmap::from([1])
                        }
                    )
                ]),
                all_words: None,
                doc_modified_commits: RoaringBitmap::from([1])
            }
        );
    }

    #[test]
    fn serde_document_test() {
        let document =
            Document {
                words: AHashMap::from([
                    (
                        "bye".into(),
                        WordIndex {
                            word_history: AHashSet::from_iter([
                                WordKey {
                                    commit_id: 1,
                                    line: 123,
                                },
                                WordKey {
                                    commit_id: 2,
                                    line: 10,
                                },
                            ]),
                            commit_inclutivity:
                                RoaringBitmap::from_sorted_iter(1..5).unwrap(),
                        },
                    ),
                    (
                        "hel".into(),
                        WordIndex {
                            word_history: AHashSet::from_iter([WordKey {
                                commit_id: 8,
                                line: 12,
                            }]),
                            commit_inclutivity:
                                RoaringBitmap::from_sorted_iter(3..8).unwrap(),
                        },
                    ),
                ]),
                all_words: Some(
                    Set::from_iter(vec!["bye", "hel", "llo"]).unwrap(),
                ),
                doc_modified_commits: RoaringBitmap::from_iter([1, 3, 5, 8]),
            };

        let encoded =
            serde::encode_to_vec(&document, bincode::config::standard());
        assert!(encoded.is_ok());

        let (decoded, _): (Document, usize) = serde::decode_from_slice(
            encoded.unwrap().as_slice(),
            bincode::config::standard(),
        )
        .unwrap();

        assert_eq!(decoded, document);
    }
}
