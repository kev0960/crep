use owo_colors::OwoColorize;
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::index::FileToWordPos;

const MAX_LINE_TO_SHOW: usize = 80;

pub struct SearchResult {
    pub words: Vec<String>,

    // File path and the id.
    pub files: Vec<(String, usize)>,
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

                if let Some(output) = self.to_search_result(&result.words, file_path, word_pos) {
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

        let mut file_line_and_pos_to_mark: BTreeMap<usize, Vec<(&str, usize)>> = BTreeMap::new();

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
                output_lines.push(format!("{:>6}| {}", line_num, lines[line_num - 1]));
            }

            output_lines.push(format!(
                "{:>6}| {}",
                line_num + 1,
                highlight_line_by_positions(&lines[line_num], &pos)
            ));
            output_lines.push(format!("pos: {:?}", pos));

            prev_line_num = Some(line_num)
        }

        Some(output_lines.join("\n"))
    }
}

fn highlight_line_by_positions(line: &str, positions: &[(&str, usize)]) -> String {
    let mut result = String::new();

    let mut current = 0;
    for pos in positions {
        let (word, start) = pos;

        if current < *start {
            result.push_str(&truncate_long_line(line));
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
    if line.len() < MAX_LINE_TO_SHOW {
        return line.to_owned();
    }

    format!(
        "{} ... {}",
        &line[0..40],
        &line[line.len() - 40..line.len()]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_search_result() {}
}
