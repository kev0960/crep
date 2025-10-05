use fst::Set;
use priority_queue::PriorityQueue;
use serde::Deserialize;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;

use roaring::RoaringBitmap;

use super::git_indexer::CommitIndex;

#[derive(Debug, Eq, Hash, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub struct WordKey {
    pub commit_id: CommitIndex,

    // Line within the commit when the word was first introduced.
    pub line: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommitEndPriority(Option<usize>);

impl Ord for CommitEndPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.0, &other.0) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(a), Some(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for CommitEndPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WordIndex {
    // PQ where CommitEndPriority refers to the last commit that the word was used.
    pub word_history: PriorityQueue<WordKey, CommitEndPriority>,

    // Whether the specific word is included in a given commit.
    pub commit_inclutivity: RoaringBitmap,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Document {
    pub words: HashMap<String, WordIndex>,

    #[serde(with = "crate::util::serde::fst::fst_set_to_vec::option")]
    pub all_words: Option<Set<Vec<u8>>>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            words: HashMap::new(),
            all_words: None,
        }
    }

    pub fn add_words(
        &mut self,
        commit_index: CommitIndex,
        words: HashMap<&str, Vec<usize>>,
    ) {
        for (word, lines) in words {
            let word_index = self.words.entry(word.to_owned()).or_default();

            for line in lines {
                word_index.word_history.push(
                    WordKey {
                        commit_id: commit_index,
                        line,
                    },
                    CommitEndPriority(None),
                );
            }

            word_index.commit_inclutivity.insert(commit_index as u32);
        }
    }

    pub fn remove_words(
        &mut self,
        commit_index: CommitIndex,
        words: &[(&str, Vec<WordKey>)],
    ) {
        for (word, word_keys) in words {
            let word_index = self.words.get_mut(*word);
            if let Some(word_index) = word_index {
                for word_key in word_keys {
                    word_index.word_history.change_priority(
                        word_key,
                        // TODO: Get prev commit properly.
                        CommitEndPriority(Some(commit_index - 1)),
                    );
                }
            }
        }

        let modified_words: HashSet<&str> =
            words.iter().map(|(word, _)| *word).collect();
        for word in modified_words {
            self.update_commit_inclutivity(commit_index, word);
        }
    }

    pub fn remove_document(&mut self, commit_index: CommitIndex) {
        for word_index in self.words.values_mut() {
            let mut is_commit_end_modified = false;

            // If there is a word key that is not marked as ended,
            // then end it now.
            loop {
                let end = match word_index.word_history.peek() {
                    Some((key, priority)) => Some((*key, priority)),
                    _ => None,
                };

                if let Some((key, priority)) = end {
                    if priority == &CommitEndPriority(None) {
                        word_index.word_history.change_priority(
                            &key,
                            // TODO: Get prev commit properly.
                            CommitEndPriority(Some(commit_index - 1)),
                        );

                        is_commit_end_modified = true;
                        continue;
                    }
                }

                break;
            }

            if is_commit_end_modified {
                let last_enabled_commit = word_index.commit_inclutivity.max();
                word_index.commit_inclutivity.insert_range(
                    last_enabled_commit.unwrap()..((commit_index) as u32),
                );
            }
        }
    }

    fn update_commit_inclutivity(
        &mut self,
        commit_index: CommitIndex,
        word: &str,
    ) {
        if let Some(word_index) = self.words.get_mut(word) {
            if let Some((_, last_commit)) = word_index.word_history.peek() {
                let last_enabled_commit = word_index.commit_inclutivity.max();
                let end_commit_index = match last_commit {
                    CommitEndPriority(None) => commit_index,
                    CommitEndPriority(Some(commit_id)) => *commit_id,
                };

                if let Some(last_enabled_bit) = last_enabled_commit {
                    word_index.commit_inclutivity.insert_range(
                        last_enabled_bit..((end_commit_index + 1) as u32),
                    );
                } else {
                    word_index
                        .commit_inclutivity
                        .insert(end_commit_index as u32);
                }
            }
        }
    }

    pub fn finalize(&mut self, commit_index: CommitIndex) {
        for index in self.words.values_mut() {
            if index.commit_inclutivity.contains(commit_index as u32) {
                continue;
            }

            if let Some((_, CommitEndPriority(None))) =
                index.word_history.peek()
            {
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
mod document_test {
    use super::*;

    use bincode::serde;

    impl PartialEq for Document {
        fn eq(&self, other: &Self) -> bool {
            if self.words != other.words {
                return false;
            }

            let self_words = match &self.all_words {
                Some(w) => w.stream().into_strs().unwrap(),
                None => vec![],
            };

            let other_words = match &self.all_words {
                Some(w) => w.stream().into_strs().unwrap(),
                None => vec![],
            };

            self_words == other_words
        }
    }

    #[test]
    fn add_words() {
        let words = HashMap::from([("hi", vec![1, 2]), ("hello", vec![1, 3])]);

        let mut document = Document::new();

        document.add_words(1, words);
        assert_eq!(
            document,
            Document {
                words: HashMap::from([
                    (
                        "hi".to_owned(),
                        WordIndex {
                            word_history: PriorityQueue::from(vec![
                                (
                                    WordKey {
                                        commit_id: 1,
                                        line: 1
                                    },
                                    CommitEndPriority(None)
                                ),
                                (
                                    WordKey {
                                        commit_id: 1,
                                        line: 2
                                    },
                                    CommitEndPriority(None)
                                )
                            ]),
                            commit_inclutivity: RoaringBitmap::from([1])
                        }
                    ),
                    (
                        "hello".to_owned(),
                        WordIndex {
                            word_history: PriorityQueue::from(vec![
                                (
                                    WordKey {
                                        commit_id: 1,
                                        line: 1
                                    },
                                    CommitEndPriority(None)
                                ),
                                (
                                    WordKey {
                                        commit_id: 1,
                                        line: 3
                                    },
                                    CommitEndPriority(None)
                                )
                            ]),
                            commit_inclutivity: RoaringBitmap::from([1])
                        }
                    )
                ]),
                all_words: None
            }
        );
    }

    #[test]
    fn serde_document_test() {
        let document =
            Document {
                words: HashMap::from([
                    (
                        "bye".to_owned(),
                        WordIndex {
                            word_history: PriorityQueue::from_iter(vec![
                                (
                                    WordKey {
                                        commit_id: 1,
                                        line: 123,
                                    },
                                    CommitEndPriority(None),
                                ),
                                (
                                    WordKey {
                                        commit_id: 2,
                                        line: 10,
                                    },
                                    CommitEndPriority(Some(3)),
                                ),
                            ]),
                            commit_inclutivity:
                                RoaringBitmap::from_sorted_iter(1..5).unwrap(),
                        },
                    ),
                    (
                        "hel".to_owned(),
                        WordIndex {
                            word_history: PriorityQueue::from_iter(vec![(
                                WordKey {
                                    commit_id: 8,
                                    line: 12,
                                },
                                CommitEndPriority(Some(6)),
                            )]),
                            commit_inclutivity:
                                RoaringBitmap::from_sorted_iter(3..8).unwrap(),
                        },
                    ),
                ]),
                all_words: Some(
                    Set::from_iter(vec!["bye", "hel", "llo"]).unwrap(),
                ),
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

#[cfg(test)]
mod pq_test {
    use super::*;

    fn insert_into_pq(
        pq: &mut PriorityQueue<WordKey, CommitEndPriority>,
        commit_id: CommitIndex,
        line: usize,
        priority: Option<CommitIndex>,
    ) {
        pq.push(WordKey { commit_id, line }, CommitEndPriority(priority));
    }

    #[test]
    fn test_priority_queue() {
        let mut pq = PriorityQueue::<WordKey, CommitEndPriority>::new();

        insert_into_pq(&mut pq, 0, 5, Some(2));
        insert_into_pq(&mut pq, 0, 7, Some(1));
        insert_into_pq(&mut pq, 1, 10, None);
        insert_into_pq(&mut pq, 2, 8, Some(4));
        insert_into_pq(&mut pq, 3, 8, Some(3));
        insert_into_pq(&mut pq, 10, 20, Some(1));

        assert_eq!(
            pq.get(&WordKey {
                commit_id: 0,
                line: 7
            }),
            Some((
                &WordKey {
                    commit_id: 0,
                    line: 7
                },
                &CommitEndPriority(Some(1))
            ))
        );

        assert_eq!(
            pq.pop(),
            Some((
                WordKey {
                    commit_id: 1,
                    line: 10
                },
                CommitEndPriority(None)
            ))
        );

        assert_eq!(
            pq.pop(),
            Some((
                WordKey {
                    commit_id: 2,
                    line: 8
                },
                CommitEndPriority(Some(4))
            ))
        );

        assert_eq!(
            pq.pop(),
            Some((
                WordKey {
                    commit_id: 3,
                    line: 8
                },
                CommitEndPriority(Some(3))
            ))
        );

        pq.change_priority(
            &WordKey {
                commit_id: 10,
                line: 20,
            },
            CommitEndPriority(Some(4)),
        );

        assert_eq!(
            pq.pop(),
            Some((
                WordKey {
                    commit_id: 10,
                    line: 20
                },
                CommitEndPriority(Some(4))
            ))
        );
    }
}
