use std::collections::{HashMap, HashSet};

pub struct Tokenizer {}

impl Tokenizer {
    pub fn new() -> Self {
        Tokenizer {}
    }

    pub fn split_to_words(content: &str) -> (HashSet<&str>, HashMap<&str, Vec<(usize, usize)>>) {
        let mut total_words = HashSet::new();
        let mut word_pos: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();

        let mut start = 0;
        for (line_num, line) in content.lines().enumerate() {
            for (i, c) in line.char_indices() {
                if c.is_ascii_punctuation() || c.is_ascii_whitespace() {
                    if i > start {
                        let word = &line[start..i];
                        total_words.insert(word);
                        word_pos.entry(word).or_default().push((line_num, start));
                    }
                    start = i + c.len_utf8()
                }
            }

            if start < line.len() {
                let word = &line[start..];
                total_words.insert(word);
                word_pos.entry(word).or_default().push((line_num, start));
            }
        }

        (total_words, word_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing_empty_line() {
        assert_eq!(
            Tokenizer::split_to_words(""),
            (HashSet::from_iter(vec![]), HashMap::new())
        );
    }

    #[test]
    fn test_parsing_words() {
        assert_eq!(
            Tokenizer::split_to_words("this is  a word"),
            (
                HashSet::from_iter(vec!["this", "is", "a", "word"]),
                HashMap::from_iter(vec![
                    ("this", vec![(0, 0)]),
                    ("is", vec![(0, 5)]),
                    ("a", vec![(0, 9)]),
                    ("word", vec![(0, 11)])
                ])
            )
        );
    }

    #[test]
    fn test_parsing_punctuations() {
        assert_eq!(
            Tokenizer::split_to_words(" std::vector<int> "),
            (
                HashSet::from_iter(vec!["std", "vector", "int"]),
                HashMap::from_iter(vec![
                    ("std", vec![(0, 1)]),
                    ("vector", vec![(0, 6)]),
                    ("int", vec![(0, 13)]),
                ])
            )
        );
    }

    #[test]
    fn test_same_words() {
        assert_eq!(
            Tokenizer::split_to_words("a ab a"),
            (
                HashSet::from_iter(vec!["a", "ab"]),
                HashMap::from_iter(vec![("a", vec![(0, 0), (0, 5)]), ("ab", vec![(0, 2)]),])
            )
        );
    }
}
