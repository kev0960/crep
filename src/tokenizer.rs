use std::collections::{BTreeSet, HashMap, HashSet};

pub struct Tokenizer {}

impl Tokenizer {
    pub fn new() -> Self {
        Tokenizer {}
    }

    pub fn split_to_words_with_col(content: &str) -> TokenizerResult {
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

        TokenizerResult {
            total_words,
            word_pos: WordPosition::LineNumWithColNoDedup(word_pos),
        }
    }

    pub fn split_to_word_line_only(content: &str) -> TokenizerResult {
        let mut total_words = HashSet::new();
        let mut word_pos: HashMap<&str, BTreeSet<usize>> = HashMap::new();

        let mut start = 0;
        for (line_num, line) in content.lines().enumerate() {
            for (i, c) in line.char_indices() {
                if c.is_ascii_punctuation() || c.is_ascii_whitespace() {
                    if i > start {
                        let word = &line[start..i];
                        total_words.insert(word);
                        word_pos.entry(word).or_default().insert(line_num);
                    }
                    start = i + c.len_utf8()
                }
            }

            if start < line.len() {
                let word = &line[start..];
                total_words.insert(word);
                word_pos.entry(word).or_default().insert(line_num);
            }
        }

        TokenizerResult {
            total_words,
            word_pos: WordPosition::LineNumOnlyWithDedup(
                word_pos
                    .into_iter()
                    .map(|(word, lines)| (word, lines.into_iter().collect()))
                    .collect(),
            ),
        }
    }

    pub fn split_lines_to_word_line_only(
        lines: &[String],
        line_start_index: usize,
    ) -> TokenizerResult {
        let mut total_words = HashSet::new();
        let mut word_pos: HashMap<&str, BTreeSet<usize>> = HashMap::new();

        let mut start = 0;
        for (line_num, line) in lines.iter().enumerate() {
            for (i, c) in line.char_indices() {
                if c.is_ascii_punctuation() || c.is_ascii_whitespace() {
                    if i > start {
                        let word = &line[start..i];
                        total_words.insert(word);
                        word_pos.entry(word).or_default().insert(line_num);
                    }
                    start = i + c.len_utf8()
                }
            }

            if start < line.len() {
                let word = &line[start..];
                total_words.insert(word);
                word_pos
                    .entry(word)
                    .or_default()
                    .insert(line_num + line_start_index);
            }
        }

        TokenizerResult {
            total_words,
            word_pos: WordPosition::LineNumOnlyWithDedup(
                word_pos
                    .into_iter()
                    .map(|(word, lines)| (word, lines.into_iter().collect()))
                    .collect(),
            ),
        }
    }
}

type LineNumAndByteIndexPos = (usize, usize);

#[derive(Debug, PartialEq)]
pub enum WordPosition<'a> {
    LineNumOnlyWithDedup(HashMap<&'a str, Vec<usize>>),
    LineNumWithColNoDedup(HashMap<&'a str, Vec<LineNumAndByteIndexPos>>),
}

#[derive(Debug)]
pub struct TokenizerResult<'a> {
    pub total_words: HashSet<&'a str>,
    pub word_pos: WordPosition<'a>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing_empty_line() {
        let result = Tokenizer::split_to_words_with_col("");
        assert_eq!(result.total_words, (HashSet::from_iter(vec![])));
        assert_eq!(
            result.word_pos,
            WordPosition::LineNumWithColNoDedup(HashMap::from_iter(vec![]))
        );
    }

    #[test]
    fn test_parsing_words() {
        let result = Tokenizer::split_to_words_with_col("this is  a word");
        assert_eq!(
            result.total_words,
            HashSet::from_iter(vec!["this", "is", "a", "word"])
        );
        assert_eq!(
            result.word_pos,
            WordPosition::LineNumWithColNoDedup(HashMap::from_iter(vec![
                ("this", vec![(0, 0)]),
                ("is", vec![(0, 5)]),
                ("a", vec![(0, 9)]),
                ("word", vec![(0, 11)])
            ]))
        );
    }

    #[test]
    fn test_parsing_non_ascii_words() {
        let result = Tokenizer::split_to_words_with_col("中文 한글 English");
        assert_eq!(
            result.total_words,
            HashSet::from_iter(vec!["中文", "한글", "English"])
        );
        assert_eq!(
            result.word_pos,
            WordPosition::LineNumWithColNoDedup(HashMap::from_iter(vec![
                ("中文", vec![(0, 0)]),
                ("한글", vec![(0, 7)]),
                ("English", vec![(0, 14)])
            ]))
        );
    }

    #[test]
    fn test_parsing_punctuations() {
        let result = Tokenizer::split_to_words_with_col(" std::vector<int> ");
        assert_eq!(
            result.total_words,
            HashSet::from_iter(vec!["std", "vector", "int"])
        );
        assert_eq!(
            result.word_pos,
            WordPosition::LineNumWithColNoDedup(HashMap::from_iter(vec![
                ("std", vec![(0, 1)]),
                ("vector", vec![(0, 6)]),
                ("int", vec![(0, 13)]),
            ]))
        );
    }

    #[test]
    fn test_same_words() {
        let result = Tokenizer::split_to_words_with_col("a ab a");
        assert_eq!(result.total_words, HashSet::from_iter(vec!["a", "ab"]));
        assert_eq!(
            result.word_pos,
            WordPosition::LineNumWithColNoDedup(HashMap::from_iter(vec![
                ("a", vec![(0, 0), (0, 5)]),
                ("ab", vec![(0, 2)]),
            ]))
        );
    }
}
