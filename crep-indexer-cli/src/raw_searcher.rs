use color_eyre::owo_colors::OwoColorize;
use crep_indexer::search::search_result::SearchResult;

use crate::searcher::{Query, Searcher};

pub fn handle_query(searcher: &mut Searcher) -> anyhow::Result<()> {
    loop {
        eprint!("Query :: ");

        let mut query = String::new();
        std::io::stdin().read_line(&mut query)?;

        let query = string_to_query(query);

        let results = searcher.handle_query(&query)?;

        let mut lines: Vec<String> = vec![];
        for result in results {
            lines.push(format!("File: {}", result.first.file_name));

            match &result.last {
                Some(last) => {
                    lines.push(format!(
                        "First seen at commit {} ... last seen at {}",
                        result.first.commit_id, last.commit_id
                    ));

                    lines.extend_from_slice(&convert_search_result_to_lines(
                        &result.first,
                    ));
                    lines.extend_from_slice(&[
                        "".to_owned(),
                        "---------------------------------------".to_owned(),
                        "".to_owned(),
                    ]);
                    lines.extend_from_slice(&convert_search_result_to_lines(
                        last,
                    ));
                }
                None => {
                    lines.push(format!(
                        "Seen at commit {}",
                        result.first.commit_id
                    ));
                    lines.extend_from_slice(&convert_search_result_to_lines(
                        &result.first,
                    ));
                }
            };

            lines.push("".to_owned());
        }

        println!("{}", lines.join("\n"));
    }
}

fn convert_search_result_to_lines(result: &SearchResult) -> Vec<String> {
    let mut lines = vec![];
    for (line_num, line) in &result.lines {
        let words = result.words_per_line.get(line_num);
        lines.push(get_highlighted_line(
            line,
            *line_num,
            words.unwrap_or(&Vec::new()),
        ))
    }

    lines
}

fn string_to_query(query: String) -> Query {
    if query.starts_with("q:") {
        Query::RawString(query.trim().chars().skip(2).collect())
    } else if query.starts_with("r:") {
        Query::Regex(query.trim().chars().skip(2).collect())
    } else {
        Query::Regex(query.trim().to_owned())
    }
}

fn get_highlighted_line(
    line: &str,
    line_number: usize,
    positions: &[(String, usize)],
) -> String {
    let mut result =
        vec![format!("{:>6}| ", (line_number + 1).to_string().yellow())];

    let mut current = 0;
    for pos in positions {
        let (word, start) = pos;
        if current < *start {
            result.push(truncate_long_line(&line[current..*start]));
        }

        result.push(format!("{}", word.to_string().red()));

        current = start + word.len();
    }

    if current < line.len() {
        result.push(truncate_long_line(&line[current..]));
    }

    result.join("")
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

const MAX_CHARS_TO_SHOW: usize = 80;
