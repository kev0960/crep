use priority_queue::PriorityQueue;
use std::{cmp::Ordering, collections::HashMap};

use roaring::RoaringBitmap;

use super::git_indexer::CommitIndex;

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct WordKey {
    commit_id: CommitIndex,

    // Line within the commit when the word was first introduced.
    line: usize,
}

#[derive(Debug, Eq, PartialEq)]
struct CommitEndPriority(Option<usize>);

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

pub struct WordIndex {
    word_history: PriorityQueue<WordKey, CommitEndPriority>,
}

pub struct Document {
    words: HashMap<String, WordIndex>,

    // For each word, we track whether the specific word was included in the specific commit.
    word_commit_inclutivity: HashMap<String, RoaringBitmap>,
}

impl Document {
    pub fn new(commit_id: CommitIndex, words: HashMap<&str, Vec<usize>>) -> Self {
        let mut word_history = HashMap::<String, WordIndex>::new();
        for (word, lines) in words.into_iter() {
            let word_index = word_history
                .entry(word.to_owned())
                .or_insert_with(|| WordIndex {
                    word_history: PriorityQueue::new(),
                });

            for line in lines {
                word_index
                    .word_history
                    .push(WordKey { commit_id, line }, CommitEndPriority(None));
            }
        }

        Self {
            words: word_history,
            word_commit_inclutivity: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod test {
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
