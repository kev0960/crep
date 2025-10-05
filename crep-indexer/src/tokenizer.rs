use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

pub struct Tokenizer {}

pub enum TokenizerMethod {
    WordOnlyIgnoreWhiteSpaceAndPunctuation = 1,
    Trigram = 2,
}

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
                        word_pos
                            .entry(word)
                            .or_default()
                            .push((line_num, start));
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

    pub fn split_lines_to_tokens(
        lines: &[String],
        line_start_index: usize,
        method: TokenizerMethod,
    ) -> TokenizerResult {
        let mut total_words = HashSet::new();
        let mut word_pos: HashMap<&str, BTreeSet<usize>> = HashMap::new();

        for (line_num, line) in lines.iter().enumerate() {
            match method {
                TokenizerMethod::WordOnlyIgnoreWhiteSpaceAndPunctuation => {
                    split_by_word(
                        line,
                        &mut total_words,
                        &mut word_pos,
                        line_num + line_start_index,
                    );
                }
                TokenizerMethod::Trigram => {
                    split_by_trigram(
                        line,
                        &mut total_words,
                        &mut word_pos,
                        line_num + line_start_index,
                    );
                }
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

fn split_by_word<'a>(
    line: &'a str,
    total_words: &mut HashSet<&'a str>,
    word_pos: &mut HashMap<&'a str, BTreeSet<usize>>,
    line_num: usize,
) {
    let mut start = 0;
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

fn split_by_trigram<'a>(
    line: &'a str,
    total_words: &mut HashSet<&'a str>,
    word_pos: &mut HashMap<&'a str, BTreeSet<usize>>,
    line_num: usize,
) {
    let mut indexes = [0, 0, 0] as [usize; 3];

    for (count, (index, c)) in line.char_indices().enumerate() {
        let start = indexes[(count + 1) % 3];
        let word = &line[start..index + c.len_utf8()];

        total_words.insert(word);
        word_pos.entry(word).or_default().insert(line_num);

        indexes[count % 3] = index;
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

    #[test]
    fn word_only_ignore_space_and_punctuation() {
        let lines = vec!["a bc def".to_owned(), "this is a word".to_owned()];

        let result = Tokenizer::split_lines_to_tokens(
            &lines,
            1,
            TokenizerMethod::WordOnlyIgnoreWhiteSpaceAndPunctuation,
        );

        assert_eq!(
            result.total_words,
            HashSet::from_iter(vec![
                "a", "bc", "def", "this", "is", "a", "word"
            ])
        );
        assert_eq!(
            result.word_pos,
            WordPosition::LineNumOnlyWithDedup(HashMap::from_iter(vec![
                ("a", vec![1, 2]),
                ("bc", vec![1]),
                ("def", vec![1]),
                ("this", vec![2]),
                ("is", vec![2]),
                ("word", vec![2])
            ]))
        );
    }

    #[test]
    fn trigram() {
        let lines = vec![
            "".to_owned(),
            "a".to_owned(),
            "ab".to_owned(),
            "abc".to_owned(),
            "1234".to_owned(),
            "56789".to_owned(),
        ];

        let result = Tokenizer::split_lines_to_tokens(
            &lines,
            1,
            TokenizerMethod::Trigram,
        );

        assert_eq!(
            result.total_words,
            HashSet::from_iter(vec![
                "a", "ab", "abc", "1", "12", "123", "234", "5", "56", "567",
                "678", "789"
            ])
        );

        assert_eq!(
            result.word_pos,
            WordPosition::LineNumOnlyWithDedup(HashMap::from_iter(vec![
                ("a", vec![2, 3, 4]),
                ("ab", vec![3, 4]),
                ("abc", vec![4]),
                ("1", vec![5]),
                ("12", vec![5]),
                ("123", vec![5]),
                ("234", vec![5]),
                ("5", vec![6]),
                ("56", vec![6]),
                ("567", vec![6]),
                ("678", vec![6]),
                ("789", vec![6])
            ]))
        );
    }
}
