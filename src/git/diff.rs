use crate::git_indexer::CommitIndex;

#[derive(Debug, PartialEq, Eq)]
pub struct FileDiffTracker {
    // commit index to number of lines.
    commit_line_end: Vec<usize>,
    commit_indexes: Vec<CommitIndex>,
}

impl FileDiffTracker {
    pub fn new(init_commit: CommitIndex, total_line: usize) -> Self {
        Self {
            commit_line_end: vec![total_line],
            commit_indexes: vec![init_commit],
        }
    }

    pub fn add_lines(&mut self, insert_start: usize, num_added_lines: usize, commit: CommitIndex) {
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
                vec![commit, self.commit_indexes[chunk_index]],
            );
        }
    }

    pub fn delete_lines(&mut self, delete_start: usize, num_deleted_lines: usize) {
        if num_deleted_lines == 0 {
            return;
        }

        let delete_start_index = self.find_chunk_index_by_line_num(delete_start);
        let delete_end_index =
            self.find_chunk_index_by_line_num(delete_start + num_deleted_lines - 1);

        // Need to find the chunk_start to delete completely.
        let should_delete_start = (self.commit_line_end[delete_start_index]
            - self.get_chunk_start(delete_start_index))
            >= num_deleted_lines;

        let should_delete_end =
            self.commit_line_end[delete_end_index] == (delete_start + num_deleted_lines);

        let purge_start_index = match should_delete_start {
            true => delete_start_index,
            false => delete_start_index + 1,
        };

        let purge_end_index = match should_delete_end {
            true => delete_end_index,
            false => delete_end_index - 1,
        };

        let num_line_to_delete_from_start = std::cmp::min(
            self.commit_line_end[delete_start_index],
            delete_start + num_deleted_lines,
        ) - delete_start;

        self.commit_line_end[delete_start_index] -= num_line_to_delete_from_start;

        for line in &mut self.commit_line_end[delete_start_index + 1..] {
            // Note: this will set the values between delete_start_index .. delete_end_index
            // incorrectly. But this is okay because they will be drained below anyway.
            *line -= num_deleted_lines;
        }

        dbg!(
            delete_start_index,
            delete_end_index,
            purge_start_index,
            purge_end_index,
            should_delete_start,
            should_delete_end
        );
        // Now remove the unnecessary chunks (chunk with the 0 size).
        if purge_start_index <= purge_end_index {
            self.commit_line_end
                .drain(purge_start_index..purge_end_index + 1);
            self.commit_indexes
                .drain(purge_start_index..purge_end_index + 1);
        }
    }

    pub fn updated_lines(&mut self, start_line: usize, changed_lines: usize, added_lines: usize) {}

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
            commit_indexes: vec![1, 2, 1, 3],
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
    fn test_add_lines() {
        let mut tracker = FileDiffTracker {
            commit_line_end: vec![5, 8, 14, 21],
            commit_indexes: vec![1, 2, 1, 3],
        };

        tracker.add_lines(0, 3, 4);
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![3, 8, 11, 17, 24],
                commit_indexes: vec![4, 1, 2, 1, 3]
            }
        );

        tracker.add_lines(11, 2, 5);
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![3, 8, 11, 13, 19, 26],
                commit_indexes: vec![4, 1, 2, 5, 1, 3]
            }
        );

        tracker.add_lines(14, 10, 6);
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![3, 8, 11, 13, 14, 24, 29, 36],
                commit_indexes: vec![4, 1, 2, 5, 1, 6, 1, 3]
            }
        );

        tracker.add_lines(36, 5, 7);
        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![3, 8, 11, 13, 14, 24, 29, 36, 41],
                commit_indexes: vec![4, 1, 2, 5, 1, 6, 1, 3, 7]
            }
        )
    }

    #[test]
    fn test_delete_lines() {
        let mut tracker = FileDiffTracker {
            commit_line_end: vec![3, 8, 11, 13, 14, 24, 29, 36, 41],
            commit_indexes: vec![4, 1, 2, 5, 1, 6, 1, 3, 7],
        };

        // Delete partially from single chunk.
        tracker.delete_lines(7, 1);

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![3, 7, 10, 12, 13, 23, 28, 35, 40],
                commit_indexes: vec![4, 1, 2, 5, 1, 6, 1, 3, 7],
            }
        );

        // Delete one chunk completely.
        tracker.delete_lines(7, 3);

        assert_eq!(
            tracker,
            FileDiffTracker {
                commit_line_end: vec![3, 7, 9, 10, 20, 25, 32, 37],
                commit_indexes: vec![4, 1, 5, 1, 6, 1, 3, 7],
            }
        );
    }
}
