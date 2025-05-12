use owo_colors::OwoColorize;
use regex::Regex;
use std::collections::{BTreeSet, HashMap, HashSet};

pub struct SearchResult {
    pub words: Vec<String>,
    pub files: Vec<String>,
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

    pub fn show_results(&mut self, search_result: &[SearchResult]) -> String {
        let files_to_read = search_result
            .iter()
            .flat_map(|r| &r.files)
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
            for file in &result.files {
                if let Some(output) = self.to_search_result(&result.words, file) {
                    total_output.push(format!("{}. {}\n{}", search_result_index, file, output));
                    search_result_index += 1;
                }
            }
        }

        total_output.join("\n\n\n")
    }

    fn to_search_result(&self, words: &[String], file: &str) -> Option<String> {
        let lines = self.file_path_to_content.get(file)?;

        let mut file_lines_to_show = BTreeSet::new();
        let mut words_to_check = words.iter().collect::<HashSet<&String>>();

        for i in 0..lines.len() {
            let line = &lines[i];

            let mut word_found = HashSet::new();
            for word in &words_to_check {
                if line.contains(*word) {
                    // We want to show the lines nearby the line that contains the word.
                    if i > 0 {
                        file_lines_to_show.insert(i - 1);
                    }

                    file_lines_to_show.insert(i);

                    if i < lines.len() - 1 {
                        file_lines_to_show.insert(i + 1);
                    }

                    word_found.insert(*word);
                }
            }

            words_to_check = words_to_check.difference(&word_found).cloned().collect();
        }

        // Now, construct the serach result.
        let pattern = words
            .iter()
            .map(|w| regex::escape(w))
            .collect::<Vec<_>>()
            .join("|");
        let regex = Regex::new(&pattern).unwrap();

        Some(
            file_lines_to_show
                .into_iter()
                .map(|line_num| {
                    let line = &lines[line_num];

                    // Highlight the words in the line.
                    format!(
                        "{:>6}| {}",
                        line_num + 1,
                        regex.replace_all(line, |caps: &regex::Captures| {
                            caps[0].to_string().red().to_string()
                        })
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}
