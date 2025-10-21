use std::{
    fmt,
    time::{Duration, Instant},
};

pub struct IndexDebugStats {
    git_diff_tree_to_tree: Duration,
    file_diff_times: Vec<Duration>,
    git_delta_index_overall_duration: Duration,
}

impl IndexDebugStats {
    pub fn new(
        diff_start: Instant,
        diff_call_end: Instant,
        for_each_start_times: Vec<Instant>,
        git_delta_index_done: Instant,
    ) -> Self {
        let git_diff_tree_to_tree = diff_call_end.duration_since(diff_start);

        let mut file_diff_times = vec![];
        for (i, t) in for_each_start_times.iter().enumerate() {
            if i == 0 {
                file_diff_times.push(t.duration_since(diff_call_end));
            } else {
                file_diff_times
                    .push(t.duration_since(for_each_start_times[i - 1]))
            }
        }

        file_diff_times.push(git_delta_index_done.duration_since(
            *for_each_start_times.last().unwrap_or(&diff_call_end),
        ));

        let git_delta_index_overall_duration =
            git_delta_index_done.duration_since(diff_start);

        Self {
            git_diff_tree_to_tree,
            file_diff_times,
            git_delta_index_overall_duration,
        }
    }
}

impl fmt::Display for IndexDebugStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let file_diff_avg = (self
            .file_diff_times
            .iter()
            .map(|d| d.as_millis())
            .sum::<u128>() as f64)
            / (self.file_diff_times.len() as f64);

        write!(
            f,
            "{} [File Avg: {}, total files: {}]",
            self.git_delta_index_overall_duration.as_millis(),
            file_diff_avg,
            self.file_diff_times.len(),
        )
    }
}
