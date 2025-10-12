use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

pub struct Tokenizer {}

impl Tokenizer {
    pub fn new() -> Self {
        Tokenizer {}
    }

    pub fn split_lines_to_tokens(
        lines: &[String],
        line_start_index: usize,
    ) -> TokenizerResult {
        let mut total_words = HashSet::new();
        let mut word_pos: HashMap<&str, BTreeSet<usize>> = HashMap::new();

        for (line_num, line) in lines.iter().enumerate() {
            split_by_trigram(
                line,
                &mut total_words,
                &mut word_pos,
                line_num + line_start_index,
            );
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

    let mut first_and_second = Vec::with_capacity(2);
    let mut total_count = 0;
    for (index, c) in line.char_indices() {
        let start = indexes[(total_count + 1) % 3];
        let word = &line[start..index + c.len_utf8()];

        if total_count < 2 {
            first_and_second.push(word);
        } else {
            total_words.insert(word);
            word_pos.entry(word).or_default().insert(line_num);
        }

        indexes[total_count % 3] = index;
        total_count += 1;
    }

    if total_count <= 2 && total_count > 0 {
        let word = first_and_second.last().unwrap();
        total_words.insert(word);
        word_pos.entry(word).or_default().insert(line_num);
    }
}

#[derive(Debug, PartialEq)]
pub enum WordPosition<'a> {
    LineNumOnlyWithDedup(HashMap<&'a str, Vec<usize>>),
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
    fn trigram() {
        let lines = vec![
            "".to_owned(),
            "a".to_owned(),
            "ab".to_owned(),
            "abc".to_owned(),
            "1234".to_owned(),
            "56789".to_owned(),
        ];

        let result = Tokenizer::split_lines_to_tokens(&lines, 1);

        assert_eq!(
            result.total_words,
            HashSet::from_iter(vec![
                "a", "ab", "abc", "123", "234", "567", "678", "789"
            ])
        );

        assert_eq!(
            result.word_pos,
            WordPosition::LineNumOnlyWithDedup(HashMap::from_iter(vec![
                ("a", vec![2]),
                ("ab", vec![3]),
                ("abc", vec![4]),
                ("123", vec![5]),
                ("234", vec![5]),
                ("567", vec![6]),
                ("678", vec![6]),
                ("789", vec![6])
            ]))
        );
    }
}
