use crate::index::git_indexer::CommitIndex;

#[derive(Debug, PartialEq, Eq, Default)]
pub struct FileDiffTracker {
    // the index that each line ends.
    commit_line_end: Vec<usize>,

    // (commit_index, line_start_in_commit)
    commit_indexes: Vec<(CommitIndex, usize)>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LineDeleteResult {
    pub commit_id: CommitIndex,

    // [start, end)
    pub start_and_end: (usize, usize),
}

impl FileDiffTracker {
    pub fn new(init_commit: CommitIndex, total_line: usize) -> Self {
        Self {
            commit_line_end: vec![total_line],
            commit_indexes: vec![(init_commit, 0)],
        }
    }

    pub fn add_lines(
        &mut self,
        insert_start: usize,
        num_added_lines: usize,
        commit: (CommitIndex, usize),
    ) {
        if num_added_lines == 0 {
            return;
        }

        let chunk_index = self.find_chunk_index_by_line_num(insert_start);
        if chunk_index == self.commit_line_end.len() {
            self.commit_line_end
                .push(self.commit_line_end.last().unwrap_or(&0) + num_added_lines);
            self.commit_indexes.push(commit);

            return;
        }

        let chunk_start = self.get_chunk_start(chunk_index);
        let is_chunk_start = chunk_start == insert_start;

        if is_chunk_start {
            self.commit_line_end
                .insert(chunk_index, chunk_start + num_added_lines);

            self.commit_indexes.insert(chunk_index, commit);

            for line_end in &mut self.commit_line_end[chunk_index + 1..] {
                *line_end += num_added_lines
            }
        } else {
            let prev_line_end = self.commit_line_end[chunk_index];
            self.commit_line_end[chunk_index] = insert_start;
            self.commit_line_end.splice(
                (chunk_index + 1)..(chunk_index + 1),
                vec![
                    insert_start + num_added_lines,
                    prev_line_end + num_added_lines,
                ],
            );

            for line_end in &mut self.commit_line_end[chunk_index + 3..] {
                *line_end += num_added_lines;
            }

            self.commit_indexes.splice(
                (chunk_index + 1)..(chunk_index + 1),
                vec![
                    commit,
                    (
                        self.commit_indexes[chunk_index].0,
                        // New commit start index is shifted by the (insert_start - chunk_start)
                        self.commit_indexes[chunk_index].1 + (insert_start - chunk_start),
                    ),
                ],
            );
        }
    }

    // There are 4 possible cases when deleting the commit chunk.
    //
    // Case 1. Right end of the start chunk and left end of the last chunk gets removed.
    //
    //        |   chunk start  |  chunk last |
    //             |x-----delete-----x|
    //                                ^
    //                     should be the new line start
    //
    // In this case,
    //   - line_start_in_commit of chunk start remains the same.
    //   - line_start_in_commit of the chunk last should be updated
    //
    // Case 2. Middle of the chunk gets removed.
    //
    //          |                     chunk                 |
    //                       |x-----delete-----x|
    //
    // In this case, line_start_in_commit is no longer continuous. Thus it will create two chunks.
    //
    //          | chunk old |                   | chunk new |
    //                                          ^
    //                                    new line start
    //
    //
    // Case 3. Only the right end of the last chunk is removed
    //
    //          |          chunk             |
    //                    |x-----delete-----x|
    //
    //  -- This case, even if the chunk is the "chunk last", we shouldn't change the line start in
    //     commit.
    //
    pub fn delete_lines(
        &mut self,
        delete_start: usize,
        num_deleted_lines: usize,
    ) -> Vec<LineDeleteResult> {
        if num_deleted_lines == 0 {
            return vec![];
        }

        // Both indices are inclusive.
        let delete_start_index = self.find_chunk_index_by_line_num(delete_start);
        let delete_end_index =
            self.find_chunk_index_by_line_num(delete_start + num_deleted_lines - 1);

        if delete_start_index == delete_end_index {
            let chunk_start = self.get_chunk_start(delete_start_index);

            // Handles case #2. This is the only case where the splitting the chunk is needed.
            if chunk_start < delete_start
                && delete_start + num_deleted_lines < self.commit_line_end[delete_start_index]
            {
                let (commit_index, line_start) = self.commit_indexes[delete_start_index];
                self.commit_indexes.insert(
                    delete_start_index + 1,
                    (
                        commit_index,
                        line_start + delete_start + num_deleted_lines - chunk_start,
                    ),
                );
                self.commit_line_end
                    .insert(delete_start_index, delete_start);

                // Now reduce the end inde by num deleted lines.
                for line in &mut self.commit_line_end[delete_start_index + 1..] {
                    *line = line.saturating_sub(num_deleted_lines);
                }

                let delete_start_pos_within_commit = line_start + delete_start - chunk_start;
                return vec![LineDeleteResult {
                    commit_id: commit_index,
                    start_and_end: (
                        delete_start_pos_within_commit,
                        delete_start_pos_within_commit + num_deleted_lines,
                    ),
                }];
            }
        }

        let mut line_delete_result: Vec<LineDeleteResult> =
            Vec::with_capacity(delete_end_index - delete_start_index + 1);

        for i in delete_start_index..(delete_end_index + 1) {
            let chunk_start = self.get_chunk_start(i);

            let delete_start_offset_within_chunk = match chunk_start < delete_start {
                true => delete_start - chunk_start,
                false => 0,
            };

            let delete_end_offset_within_chunk =
                match self.commit_line_end[i] < delete_start + num_deleted_lines {
                    true => self.commit_line_end[i] - chunk_start,
                    false => delete_start + num_deleted_lines - chunk_start,
                };

            let (commit_id, line_end) = self.commit_indexes[i];
            line_delete_result.push(LineDeleteResult {
                commit_id,
                start_and_end: (
                    line_end + delete_start_offset_within_chunk,
                    line_end + delete_end_offset_within_chunk,
                ),
            })
        }

        // Need to find the chunk_start to delete completely.
        let should_delete_start = ((self.commit_line_end[delete_start_index]
            - self.get_chunk_start(delete_start_index))
            <= num_deleted_lines)
            && self.get_chunk_start(delete_start_index) == delete_start;

        let should_delete_end =
            self.commit_line_end[delete_end_index] == (delete_start + num_deleted_lines);

        let purge_start_index = match should_delete_start {
            true => delete_start_index,
            false => delete_start_index + 1,
        };

        let purge_end_index = match should_delete_end {
            true => delete_end_index,
            false => delete_end_index.saturating_sub(1),
        };

        let last_chunk_start = self.get_chunk_start(delete_end_index);

        // Case #3.
        if last_chunk_start >= delete_start {
            self.commit_indexes[delete_end_index].1 +=
                delete_start + num_deleted_lines - last_chunk_start;
        }

        let num_line_to_delete_from_start = std::cmp::min(
            self.commit_line_end[delete_start_index],
            delete_start + num_deleted_lines,
        ) - delete_start;

        self.commit_line_end[delete_start_index] -= num_line_to_delete_from_start;

        for line in &mut self.commit_line_end[delete_start_index + 1..] {
            // Note: this will set the values between delete_start_index .. delete_end_index
            // incorrectly. But this is okay because they will be drained below anyway.
            *line = line.saturating_sub(num_deleted_lines);
        }

        // Now remove the unnecessary chunks (chunk with the 0 size).
        if purge_start_index <= purge_end_index {
            self.commit_line_end
                .drain(purge_start_index..purge_end_index + 1);
            self.commit_indexes
                .drain(purge_start_index..purge_end_index + 1);
        }

        line_delete_result
    }

    fn find_chunk_index_by_line_num(&self, line_num: usize) -> usize {
        match self.commit_line_end.binary_search_by(|x| x.cmp(&line_num)) {
            Ok(pos) => {
                if pos + 1 < self.commit_line_end.len() {
                    pos + 1
                } else {
                    self.commit_line_end.len()
                }
            }
            Err(pos) => pos,
        }
    }

    fn get_chunk_start(&self, chunk_index: usize) -> usize {
        match chunk_index {
            0 => 0,
            _ => self.commit_line_end[chunk_index - 1],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_chunk_index_by_line_num() {
        let tracker = FileDiffTracker {
            commit_line_end: vec![5, 8, 14, 21],
            commit_indexes: vec![(1, 0), (2, 5), (1, 5), (3, 10)],
        };

        assert_eq!(
            (0..=22)
                .map(|pos| tracker.find_chunk_index_by_line_num(pos))
                .collect::<Vec<usize>>(),
            vec![
                0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 4, 4
            ]
        );

        let empty_tracker = FileDiffTracker {
            commit_line_end: vec![],
            commit_indexes: vec![],
        };

        assert_eq!(empty_tracker.find_chunk_index_by_line_num(0), 0);
        assert_eq!(empty_tracker.find_chunk_index_by_line_num(1), 0);
    }

    #[test]
    fn test_add_lines_front() {
        let mut tracker = FileDiffTracker {
            commit_line_end: vec![36],
            commit_indexes: vec![(0, 0)],
        };

        tracker.add_lines(12, 1, (1, 13));
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![12, 13, 37],
                commit_indexes: vec![(0, 0), (1, 13), (0, 12)],
            }
        );

        tracker.add_lines(10, 1, (2, 10));
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![10, 11, 13, 14, 38],
                commit_indexes: vec![(0, 0), (2, 10), (0, 10), (1, 13), (0, 12)],
            }
        );

        // Add a new line at the front.
        tracker.add_lines(0, 5, (3, 0));
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![5, 15, 16, 18, 19, 43],
                commit_indexes: vec![(3, 0), (0, 0), (2, 10), (0, 10), (1, 13), (0, 12)],
            }
        );
    }

    #[test]
    fn test_add_lines_at_end() {
        let mut tracker = FileDiffTracker {
            commit_line_end: vec![36],
            commit_indexes: vec![(0, 0)],
        };

        tracker.add_lines(36, 5, (1, 36));
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![36, 41],
                commit_indexes: vec![(0, 0), (1, 36)],
            }
        );

        // Add at the middle of the last element.
        tracker.add_lines(38, 2, (2, 38));
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![36, 38, 40, 43],
                commit_indexes: vec![(0, 0), (1, 36), (2, 38), (1, 38)],
            }
        );

        // Add at the middle element.
        tracker.add_lines(39, 5, (3, 39));
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![36, 38, 39, 44, 45, 48],
                commit_indexes: vec![(0, 0), (1, 36), (2, 38), (3, 39), (2, 39), (1, 38)],
            }
        );
    }

    #[test]
    fn test_delete_lines_front() {
        let mut tracker = FileDiffTracker {
            commit_line_end: vec![36, 38, 39, 44, 45, 48],
            commit_indexes: vec![(0, 0), (1, 36), (2, 38), (3, 39), (2, 39), (1, 38)],
        };

        assert_eq!(
            tracker.delete_lines(0, 10),
            vec![LineDeleteResult {
                commit_id: 0,
                start_and_end: (0, 10)
            }]
        );

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![26, 28, 29, 34, 35, 38],
                commit_indexes: vec![(0, 10), (1, 36), (2, 38), (3, 39), (2, 39), (1, 38)]
            }
        );

        // Delete the middle of the first chunk.
        assert_eq!(
            tracker.delete_lines(5, 10),
            vec![LineDeleteResult {
                commit_id: 0,
                start_and_end: (15, 25)
            }]
        );

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![5, 16, 18, 19, 24, 25, 28],
                commit_indexes: vec![
                    (0, 10),
                    (0, 25),
                    (1, 36),
                    (2, 38),
                    (3, 39),
                    (2, 39),
                    (1, 38)
                ]
            }
        );

        // Delete the entire first chunk.
        assert_eq!(
            tracker.delete_lines(0, 5),
            vec![LineDeleteResult {
                commit_id: 0,
                start_and_end: (10, 15)
            }]
        );
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![11, 13, 14, 19, 20, 23],
                commit_indexes: vec![(0, 25), (1, 36), (2, 38), (3, 39), (2, 39), (1, 38)]
            }
        );
    }

    #[test]
    fn test_delete_lines_middle() {
        let mut tracker = FileDiffTracker {
            commit_line_end: vec![36, 38, 39, 44, 45, 48],
            commit_indexes: vec![(0, 0), (1, 36), (2, 38), (3, 39), (2, 39), (1, 38)],
        };

        assert_eq!(
            tracker.delete_lines(41, 6),
            vec![
                LineDeleteResult {
                    commit_id: 3,
                    start_and_end: (41, 44)
                },
                LineDeleteResult {
                    commit_id: 2,
                    start_and_end: (39, 40)
                },
                LineDeleteResult {
                    commit_id: 1,
                    start_and_end: (38, 40)
                }
            ]
        );

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![36, 38, 39, 41, 42],
                commit_indexes: vec![(0, 0), (1, 36), (2, 38), (3, 39), (1, 40)]
            }
        );

        assert_eq!(
            tracker.delete_lines(20, 18),
            vec![
                LineDeleteResult {
                    commit_id: 0,
                    start_and_end: (20, 36)
                },
                LineDeleteResult {
                    commit_id: 1,
                    start_and_end: (36, 38)
                }
            ]
        );

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![20, 21, 23, 24],
                commit_indexes: vec![(0, 0), (2, 38), (3, 39), (1, 40)]
            }
        );

        assert_eq!(
            tracker.delete_lines(20, 3),
            vec![
                LineDeleteResult {
                    commit_id: 2,
                    start_and_end: (38, 39)
                },
                LineDeleteResult {
                    commit_id: 3,
                    start_and_end: (39, 41)
                }
            ]
        );

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![20, 21],
                commit_indexes: vec![(0, 0), (1, 40)]
            }
        );

        assert_eq!(
            tracker.delete_lines(0, 21),
            vec![
                LineDeleteResult {
                    commit_id: 0,
                    start_and_end: (0, 20)
                },
                LineDeleteResult {
                    commit_id: 1,
                    start_and_end: (40, 41)
                }
            ]
        );

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![],
                commit_indexes: vec![]
            }
        );

        // Case #1 for the middle element.
        let mut tracker = FileDiffTracker {
            commit_line_end: vec![36, 38, 39, 44, 45, 48],
            commit_indexes: vec![(0, 0), (1, 36), (2, 38), (3, 39), (2, 39), (1, 38)],
        };

        assert_eq!(
            tracker.delete_lines(40, 2),
            vec![LineDeleteResult {
                commit_id: 3,
                start_and_end: (40, 42)
            },]
        );

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![36, 38, 39, 40, 42, 43, 46],
                commit_indexes: vec![(0, 0), (1, 36), (2, 38), (3, 39), (3, 42), (2, 39), (1, 38)]
            }
        );
    }

    #[test]
    fn test_delete_lines_at_end() {
        let mut tracker = FileDiffTracker {
            commit_line_end: vec![36, 38, 39, 44, 45, 48],
            commit_indexes: vec![(0, 0), (1, 36), (2, 38), (3, 39), (2, 39), (1, 38)],
        };

        assert_eq!(
            tracker.delete_lines(46, 2),
            vec![LineDeleteResult {
                commit_id: 1,
                start_and_end: (39, 41)
            },]
        );

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![36, 38, 39, 44, 45, 46],
                commit_indexes: vec![(0, 0), (1, 36), (2, 38), (3, 39), (2, 39), (1, 38)]
            }
        );

        assert_eq!(
            tracker.delete_lines(45, 1),
            vec![LineDeleteResult {
                commit_id: 1,
                start_and_end: (38, 39)
            },]
        );

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![36, 38, 39, 44, 45],
                commit_indexes: vec![(0, 0), (1, 36), (2, 38), (3, 39), (2, 39)]
            }
        );

        assert_eq!(
            tracker.delete_lines(39, 6),
            vec![
                LineDeleteResult {
                    commit_id: 3,
                    start_and_end: (39, 44)
                },
                LineDeleteResult {
                    commit_id: 2,
                    start_and_end: (39, 40)
                },
            ]
        );

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![36, 38, 39],
                commit_indexes: vec![(0, 0), (1, 36), (2, 38)]
            }
        );
    }
}
