use owo_colors::OwoColorize;

const MAX_CHARS_TO_SHOW: usize = 80;

pub fn highlight_line_by_positions(
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
            format!("this {} a line", "is".red())
        );
    }

    #[test]
    fn test_highlight_line_multiple_positions() {
        assert_eq!(
            highlight_line_by_positions(
                //12345678
                "this is a line",
                &[("is", 5), ("a", 8)]
            ),
            format!("this {} {} line", "is".red(), "a".red())
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
