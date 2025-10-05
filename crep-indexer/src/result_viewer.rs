use owo_colors::OwoColorize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::index::index::FileToWordPos;

const MAX_CHARS_TO_SHOW: usize = 80;

pub struct SearchResult {
    pub words: Vec<String>,

    // File path and the id.
    pub files: Vec<(String, usize)>,

    pub git_commit_range: Option<(usize, usize)>,
}

pub struct SearchResultViewer {
    file_path_to_content: HashMap<String, Vec<String>>,
}

impl SearchResultViewer {
    pub fn new() -> Self {
        Self {
            file_path_to_content: HashMap::new(),
        }
    }

    pub fn show_results(
        &mut self,
        search_result: &[SearchResult],
        file_to_word_pos: &FileToWordPos,
    ) -> String {
        let files_to_read = search_result
            .iter()
            .flat_map(|r| &r.files)
            .map(|(file_path, _)| file_path)
            .collect::<HashSet<_>>();

        for file_path in files_to_read {
            if self.file_path_to_content.contains_key(file_path) {
                continue;
            }

            let content = std::fs::read_to_string(file_path)
                .unwrap()
                .lines()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();

            self.file_path_to_content.insert(file_path.clone(), content);
        }

        let mut search_result_index = 1;
        let mut total_output: Vec<String> = vec![];
        for result in search_result {
            for (file_path, file_id) in &result.files {
                let word_pos = match file_to_word_pos.get(file_id) {
                    Some(pos) => pos,
                    None => continue,
                };

                if let Some(output) =
                    self.to_search_result(&result.words, file_path, word_pos)
                {
                    total_output.push(format!(
                        "{}. {}\n{}",
                        search_result_index, file_path, output
                    ));
                    search_result_index += 1;
                }
            }
        }

        total_output.join("\n\n\n")
    }

    fn to_search_result(
        &self,
        words: &[String],
        file: &str,
        word_pos: &HashMap<String, Vec<(usize, usize)>>,
    ) -> Option<String> {
        let lines = self.file_path_to_content.get(file)?;

        let mut file_line_and_pos_to_mark: BTreeMap<usize, Vec<(&str, usize)>> =
            BTreeMap::new();

        for word in words {
            let positions = word_pos.get(word);

            if let Some(line_and_cols) = positions {
                if line_and_cols.is_empty() {
                    continue;
                }

                let (line_num, col) = *line_and_cols.first().unwrap();

                file_line_and_pos_to_mark
                    .entry(line_num)
                    .or_default()
                    .push((word, col));
            }
        }

        let mut output_lines = Vec::new();

        let mut prev_line_num = None;
        for (line_num, pos) in file_line_and_pos_to_mark {
            if line_num > 0 && prev_line_num != Some(line_num - 1) {
                output_lines.push(format!(
                    "{:>6}| {}",
                    line_num,
                    lines[line_num - 1]
                ));
            }

            output_lines.push(format!(
                "{:>6}| {}",
                line_num + 1,
                highlight_line_by_positions(&lines[line_num], &pos)
            ));

            prev_line_num = Some(line_num)
        }

        Some(output_lines.join("\n"))
    }
}

fn highlight_line_by_positions(
    line: &str,
    positions: &[(&str, usize)],
) -> String {
    let mut result = String::new();

    let mut current = 0;
    for pos in positions {
        let (word, start) = pos;

        if current < *start {
            result.push_str(&truncate_long_line(&line[current..*start]));
        }

        result.push_str(&word.to_string().red().to_string());

        current = start + word.len();
    }

    if current < line.len() {
        result.push_str(&truncate_long_line(&line[current..]));
    }

    result
}

fn truncate_long_line(line: &str) -> String {
    if line.len() < MAX_CHARS_TO_SHOW {
        return line.to_owned();
    }

    format!(
        "{} ... {}",
        get_first_n_chars(line, MAX_CHARS_TO_SHOW / 2),
        get_last_n_chars(line, MAX_CHARS_TO_SHOW / 2)
    )
}

fn get_first_n_chars(line: &str, n: usize) -> &str {
    let end_byte_index = line
        .char_indices()
        .nth(n)
        .map_or(line.len(), |(idx, _)| idx);

    &line[0..end_byte_index]
}

fn get_last_n_chars(line: &str, n: usize) -> &str {
    let start_byte_index = line
        .char_indices()
        .rev()
        .nth(n.saturating_sub(1))
        .map_or(0, |(idx, _)| idx);

    &line[start_byte_index..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_line_position() {
        assert_eq!(
            highlight_line_by_positions("this is a line", &[("is", 5)]),
            format!("this {} a line", "is".to_string().red())
        );
    }

    #[test]
    fn test_get_first_n_chars() {
        assert_eq!(get_first_n_chars("", 3), "");
        assert_eq!(get_first_n_chars("a", 3), "a");
        assert_eq!(get_first_n_chars("ab", 3), "ab");
        assert_eq!(get_first_n_chars("abc", 3), "abc");
        assert_eq!(get_first_n_chars("abcd", 3), "abc");

        assert_eq!(get_first_n_chars("가", 0), "");
        assert_eq!(get_first_n_chars("가나a다", 1), "가");
        assert_eq!(get_first_n_chars("가나a다", 2), "가나");
        assert_eq!(get_first_n_chars("가나a다", 3), "가나a");
        assert_eq!(get_first_n_chars("가나a다", 4), "가나a다");
    }

    #[test]
    fn test_get_last_n_chars() {
        assert_eq!(get_last_n_chars("", 3), "");
        assert_eq!(get_last_n_chars("a", 3), "a");
        assert_eq!(get_last_n_chars("ab", 3), "ab");
        assert_eq!(get_last_n_chars("abc", 3), "abc");
        assert_eq!(get_last_n_chars("abcd", 3), "bcd");

        assert_eq!(get_last_n_chars("가", 1), "가");
        assert_eq!(get_last_n_chars("가a나", 2), "a나");
        assert_eq!(get_last_n_chars("가a나", 3), "가a나");
    }
}
