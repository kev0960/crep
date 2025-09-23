use std::fmt::Write;
use std::str::FromStr;

use arrayvec::ArrayVec;
use regex_syntax::hir::Class;

use super::permutation::PermutationIterator;

#[derive(Clone, PartialEq, Debug)]
pub enum CharacterClass {
    Char(char),
    CharRange(Vec<CharRange>),
}

#[derive(Clone, PartialEq, Debug)]
pub struct CharRange {
    // Ranges are *inclusive*.
    start: char,
    end: char,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Trigram {
    data: ArrayVec<CharacterClass, 3>,
}

pub enum RegexOrString {
    String(String),
    Regex(String),
}

impl Trigram {
    pub fn new(s: &str) -> Self {
        s.parse().unwrap()
    }

    pub fn from_long_string(s: &str) -> Vec<Self> {
        if s.len() < 3 {
            return vec![Trigram::new(s)];
        }

        let mut v = vec![];

        let mut indexes = [0, 0, 0] as [usize; 3];
        for (count, (index, c)) in s.char_indices().enumerate() {
            let start = indexes[(count + 1) % 3];
            indexes[count % 3] = index;

            if count < 2 {
                // Do not create the length 1 or 2 grams.
                continue;
            }

            v.push(Trigram::new(&s[start..index + c.len_utf8()]));
        }

        v
    }

    pub fn create_matching_regex_or_string(&self) -> RegexOrString {
        if self.data.len() == 3
            && self
                .data
                .iter()
                .all(|c| matches!(c, CharacterClass::Char(_)))
        {
            return RegexOrString::String(
                self.data
                    .iter()
                    .map(|c| {
                        if let CharacterClass::Char(c) = c {
                            c
                        } else {
                            panic!("This should not happen");
                        }
                    })
                    .collect::<String>(),
            );
        }

        // Otherwise this is the regex.
        let mut pattern = String::from(".*");
        for c in &self.data {
            match c {
                CharacterClass::Char(c) => {
                    pattern.push(*c);
                }
                CharacterClass::CharRange(range) => {
                    pattern.push('[');

                    for interval in range {
                        write!(
                            &mut pattern,
                            r"\u{{{:X}}}-\u{{{:X}}}",
                            interval.start as u32, interval.end as u32
                        )
                        .unwrap();
                    }

                    pattern.push(']');
                }
            }
        }

        pattern.push_str(".*");
        RegexOrString::Regex(pattern)
    }

    // Concat two small (the sum of trigram lengths should be <= 3) trigrams.
    fn concat_small(left: &Trigram, right: &Trigram) -> Self {
        let mut left = left.data.clone();
        for r in &right.data {
            left.push(r.clone());
        }

        Self { data: left }
    }

    fn concat(left: &Trigram, right: &Trigram) -> Vec<Self> {
        let mut v = vec![];

        let total_len = left.data.len() + right.data.len();
        for start_index in 0..total_len - 2 {
            let mut data: ArrayVec<CharacterClass, 3> = ArrayVec::new();

            for i in start_index..start_index + 3 {
                if i < left.data.len() {
                    data.push(left.data[i].clone());
                } else {
                    data.push(right.data[i - left.data.len()].clone())
                }
            }

            v.push(Trigram { data })
        }

        v
    }
}

impl FromStr for Trigram {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let mut data = ArrayVec::<CharacterClass, 3>::new();
        for c in s.chars() {
            data.push(CharacterClass::Char(c))
        }

        Ok(Self { data })
    }
}

impl From<&Class> for Trigram {
    fn from(c: &Class) -> Self {
        let mut range: Vec<CharRange> = vec![];
        match c {
            Class::Bytes(b) => {
                for interval in b.ranges() {
                    range.push(CharRange {
                        start: interval.start() as char,
                        end: interval.end() as char,
                    });
                }
            }
            Class::Unicode(u) => {
                for interval in u.ranges() {
                    range.push(CharRange {
                        start: interval.start(),
                        end: interval.end(),
                    });
                }
            }
        }

        Self {
            data: ArrayVec::<CharacterClass, 3>::from_iter([
                CharacterClass::CharRange(range),
            ]),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct SearchPartTrigram {
    pub trigrams: Vec<Trigram>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct RegexSearchCandidates {
    pub candidates: Vec<SearchPartTrigram>,
}

impl RegexSearchCandidates {
    pub fn concat(parts: &[RegexSearchCandidates]) -> Self {
        let candidate_sizes = parts
            .iter()
            .map(|p| p.candidates.len() as u32)
            .collect::<Vec<u32>>();

        if candidate_sizes.contains(&0) {
            return Self { candidates: vec![] };
        }

        let permutations = PermutationIterator::new(&candidate_sizes);

        let mut new_candidates: Vec<SearchPartTrigram> = vec![];
        for permutation in permutations {
            let mut trigrams: Vec<&Vec<Trigram>> = vec![];
            for (index, perm_index) in permutation.iter().enumerate() {
                let part = &parts[index].candidates[*perm_index as usize];

                if !part.trigrams.is_empty() {
                    trigrams.push(&part.trigrams);
                }
            }

            // Now merge trigrams.
            let merged_trigrams = merge_trigrams(&trigrams);

            new_candidates.push({
                SearchPartTrigram {
                    trigrams: merged_trigrams,
                }
            })
        }

        Self {
            candidates: new_candidates,
        }
    }

    pub fn alternation(parts: &[RegexSearchCandidates]) -> Self {
        Self {
            candidates: parts
                .iter()
                .flat_map(|p| p.candidates.clone())
                .collect(),
        }
    }

    pub fn repeat(
        part: &RegexSearchCandidates,
        min: u32,
        max: Option<u32>,
    ) -> Self {
        let mut repeated_trigrams: Vec<SearchPartTrigram> = vec![];

        // Only repeat upto 3 really matters.
        let min_repeat_count = std::cmp::min(min, 3);
        let max_repeat_count = std::cmp::min(max.unwrap_or(3), 3);

        for repeat in min_repeat_count..(max_repeat_count + 1) {
            // Now pick (repeat)
            if repeat == 0 {
                repeated_trigrams.push(SearchPartTrigram { trigrams: vec![] });

                continue;
            }

            let permutations = PermutationIterator::new(
                vec![part.candidates.len() as u32; repeat as usize].as_slice(),
            );

            for permutation in permutations {
                let mut trigrams = vec![];
                for index in permutation {
                    trigrams.push(&part.candidates[index as usize].trigrams);
                }

                repeated_trigrams.push(SearchPartTrigram {
                    trigrams: merge_trigrams(&trigrams),
                });
            }
        }

        Self {
            candidates: repeated_trigrams,
        }
    }
}

fn merge_trigrams(trigrams: &[&Vec<Trigram>]) -> Vec<Trigram> {
    let mut merged = trigrams[0].clone();

    for trigram in trigrams.iter().skip(1) {
        let left = merged.pop().unwrap();
        let first_on_right = &trigram[0];

        if first_on_right.data.len() <= 3 - left.data.len() {
            assert!(trigram.len() == 1);

            merged.push(Trigram::concat_small(&left, first_on_right));
        } else {
            merged.extend_from_slice(
                Trigram::concat(&left, first_on_right).as_slice(),
            );
            merged.extend_from_slice(&trigram[1..]);
        }
    }

    merged
}

#[cfg(test)]
mod trigram_test {
    use super::*;

    #[test]
    fn trigram_from_long_string() {
        assert_eq!(Trigram::from_long_string("a"), vec!["a".parse().unwrap()]);
        assert_eq!(
            Trigram::from_long_string("ab"),
            vec!["ab".parse().unwrap()]
        );
        assert_eq!(
            Trigram::from_long_string("abc"),
            vec!["abc".parse().unwrap()]
        );
        assert_eq!(
            Trigram::from_long_string("abcd"),
            vec!["abc".parse().unwrap(), "bcd".parse().unwrap()]
        );
        assert_eq!(
            Trigram::from_long_string("a b c"),
            vec![
                "a b".parse().unwrap(),
                " b ".parse().unwrap(),
                "b c".parse().unwrap()
            ]
        );
    }

    #[test]
    fn trigram_concat_small() {
        let empty_gram: Trigram = "".parse().unwrap();
        let one_gram: Trigram = "a".parse().unwrap();
        let two_gram: Trigram = "bb".parse().unwrap();
        let three_gram: Trigram = "ccc".parse().unwrap();

        assert_eq!(
            Trigram::concat_small(&one_gram, &one_gram),
            "aa".parse().unwrap()
        );
        assert_eq!(
            Trigram::concat_small(&two_gram, &one_gram),
            "bba".parse().unwrap()
        );
        assert_eq!(
            Trigram::concat_small(&one_gram, &two_gram),
            "abb".parse().unwrap()
        );
        assert_eq!(
            Trigram::concat_small(&three_gram, &empty_gram),
            "ccc".parse().unwrap()
        );
        assert_eq!(
            Trigram::concat_small(&empty_gram, &three_gram),
            "ccc".parse().unwrap()
        );
    }

    #[test]
    fn trigram_concat_large() {
        let one_gram = Trigram::new("a");
        let two_gram1 = Trigram::new("bb");
        let two_gram2 = Trigram::new("cc");
        let three_gram1 = Trigram::new("ddd");
        let three_gram2 = Trigram::new("eee");

        assert_eq!(
            Trigram::concat(&one_gram, &two_gram1),
            vec!["abb".parse().unwrap()]
        );
        assert_eq!(
            Trigram::concat(&two_gram1, &two_gram2),
            vec![Trigram::new("bbc"), Trigram::new("bcc")]
        );

        assert_eq!(
            Trigram::concat(&two_gram1, &three_gram1),
            vec![
                Trigram::new("bbd"),
                Trigram::new("bdd"),
                Trigram::new("ddd")
            ]
        );

        assert_eq!(
            Trigram::concat(&three_gram1, &three_gram2),
            vec![
                Trigram::new("ddd"),
                Trigram::new("dde"),
                Trigram::new("dee"),
                Trigram::new("eee")
            ]
        );
    }

    #[test]
    fn merge_trigrams_test() {
        let one_gram = Trigram::new("a");
        let two_gram1 = Trigram::new("bb");
        let two_gram2 = Trigram::new("cc");
        let three_gram1 = Trigram::new("ddd");
        let three_gram2 = Trigram::new("eee");

        assert_eq!(
            merge_trigrams(&[&vec![one_gram.clone()]]),
            vec![Trigram::new("a")]
        );

        assert_eq!(
            merge_trigrams(&[&vec![one_gram.clone()], &vec![one_gram.clone()]]),
            vec![Trigram::new("aa")]
        );

        assert_eq!(
            merge_trigrams(&[
                &vec![two_gram1.clone()],
                &vec![one_gram.clone()]
            ]),
            vec![Trigram::new("bba")]
        );

        assert_eq!(
            merge_trigrams(&[
                &vec![one_gram.clone()],
                &vec![two_gram2.clone()]
            ]),
            vec![Trigram::new("acc")]
        );

        assert_eq!(
            merge_trigrams(&[
                &vec![two_gram1.clone()],
                &vec![two_gram2.clone()]
            ]),
            vec![Trigram::new("bbc"), Trigram::new("bcc")]
        );

        assert_eq!(
            merge_trigrams(&[
                &vec![two_gram1.clone()],
                &vec![three_gram1.clone()]
            ]),
            vec![
                Trigram::new("bbd"),
                Trigram::new("bdd"),
                Trigram::new("ddd")
            ]
        );

        assert_eq!(
            merge_trigrams(&[
                &vec![three_gram2.clone()],
                &vec![three_gram1.clone()]
            ]),
            vec![
                Trigram::new("eee"),
                Trigram::new("eed"),
                Trigram::new("edd"),
                Trigram::new("ddd")
            ]
        );

        assert_eq!(
            merge_trigrams(&[
                &vec![one_gram.clone()],
                &vec![one_gram.clone()],
                &vec![one_gram.clone()]
            ]),
            vec![Trigram::new("aaa")]
        );

        assert_eq!(
            merge_trigrams(&[
                &vec![one_gram.clone()],
                &vec![two_gram1.clone()],
                &vec![one_gram.clone()]
            ]),
            vec![Trigram::new("abb"), Trigram::new("bba")]
        );

        assert_eq!(
            merge_trigrams(&[
                &vec![two_gram1.clone()],
                &vec![two_gram2.clone()],
                &vec![one_gram.clone()]
            ]),
            vec![
                Trigram::new("bbc"),
                Trigram::new("bcc"),
                Trigram::new("cca")
            ]
        );

        assert_eq!(
            merge_trigrams(&[
                &vec![three_gram1.clone()],
                &vec![two_gram1.clone()],
                &vec![one_gram.clone()]
            ]),
            vec![
                Trigram::new("ddd"),
                Trigram::new("ddb"),
                Trigram::new("dbb"),
                Trigram::new("bba")
            ]
        );
    }
}

#[cfg(test)]
mod regex_search_part_tests {
    use super::*;

    #[cfg(test)]
    macro_rules! t {
        ($($s:literal),+ $(,)?) => {
            vec![$(Trigram::new($s)),+]
        };
    }

    #[test]
    fn test_concat() {
        let part1 = RegexSearchCandidates {
            candidates: vec![SearchPartTrigram {
                trigrams: t!("abc", "bcd"),
            }],
        };

        let part2 = RegexSearchCandidates {
            candidates: vec![SearchPartTrigram { trigrams: t!("12") }],
        };

        let part3 = RegexSearchCandidates {
            candidates: vec![SearchPartTrigram {
                trigrams: t!("xyz"),
            }],
        };

        assert_eq!(
            RegexSearchCandidates::concat(&[part1, part2, part3]),
            RegexSearchCandidates {
                candidates: vec![SearchPartTrigram {
                    trigrams: t!(
                        "abc", "bcd", "cd1", "d12", "12x", "2xy", "xyz"
                    ),
                }]
            }
        );
    }

    #[test]
    fn test_alternation() {
        let part1 = RegexSearchCandidates {
            candidates: vec![SearchPartTrigram {
                trigrams: t!("abc", "bcd"),
            }],
        };

        let part2 = RegexSearchCandidates {
            candidates: vec![SearchPartTrigram { trigrams: t!("12") }],
        };

        assert_eq!(
            RegexSearchCandidates::alternation(&[part1, part2]),
            RegexSearchCandidates {
                candidates: vec![
                    SearchPartTrigram {
                        trigrams: t!("abc", "bcd"),
                    },
                    SearchPartTrigram { trigrams: t!("12") }
                ]
            }
        );
    }

    #[test]
    fn test_repeat_simple() {
        let part = RegexSearchCandidates {
            candidates: vec![SearchPartTrigram { trigrams: t!("a") }],
        };

        assert_eq!(
            RegexSearchCandidates::repeat(&part, 0, Some(100)),
            RegexSearchCandidates {
                candidates: vec![
                    SearchPartTrigram { trigrams: vec![] },
                    SearchPartTrigram { trigrams: t!("a") },
                    SearchPartTrigram { trigrams: t!("aa") },
                    SearchPartTrigram {
                        trigrams: t!("aaa"),
                    }
                ]
            }
        );
    }

    #[test]
    fn test_repeat_complex() {
        let part = RegexSearchCandidates {
            candidates: vec![
                SearchPartTrigram { trigrams: t!("a") },
                SearchPartTrigram { trigrams: t!("bc") },
                SearchPartTrigram {
                    trigrams: t!("xyz"),
                },
            ],
        };

        assert_eq!(
            RegexSearchCandidates::repeat(&part, 2, Some(2)),
            RegexSearchCandidates {
                candidates: vec![
                    SearchPartTrigram { trigrams: t!("aa") },
                    SearchPartTrigram {
                        trigrams: t!("abc"),
                    },
                    SearchPartTrigram {
                        trigrams: t!("axy", "xyz"),
                    },
                    SearchPartTrigram {
                        trigrams: t!("bca"),
                    },
                    SearchPartTrigram {
                        trigrams: t!("bcb", "cbc"),
                    },
                    SearchPartTrigram {
                        trigrams: t!("bcx", "cxy", "xyz"),
                    },
                    SearchPartTrigram {
                        trigrams: t!("xyz", "yza"),
                    },
                    SearchPartTrigram {
                        trigrams: t!("xyz", "yzb", "zbc"),
                    },
                    SearchPartTrigram {
                        trigrams: t!("xyz", "yzx", "zxy", "xyz"),
                    },
                ]
            }
        );
    }
}
