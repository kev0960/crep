use std::str::FromStr;

use arrayvec::ArrayVec;
use roaring::RoaringBitmap;

use crate::util::bitmap::utils::intersect_bitmaps;

use super::{
    git_searcher::RawPerFileSearchResult, permutation::PermutationIterator,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CharacterClass {
    Char(char),
}

#[derive(Clone, PartialEq, Debug)]
pub struct Trigram {
    data: ArrayVec<CharacterClass, 3>,
}

impl Trigram {
    fn new(s: &str) -> Self {
        s.parse().unwrap()
    }

    // Concat two small (the sum of trigram lengths should be <= 3) trigrams.
    fn concat_small(left: &Trigram, right: &Trigram) -> Self {
        let mut left = left.data.clone();
        left.try_extend_from_slice(&right.data).unwrap();

        Self { data: left }
    }

    fn concat(left: &Trigram, right: &Trigram) -> Vec<Self> {
        let mut v = vec![];

        let total_len = left.data.len() + right.data.len();
        for start_index in 0..total_len - 2 {
            let mut data: ArrayVec<CharacterClass, 3> = ArrayVec::new();

            for i in start_index..start_index + 3 {
                if i < left.data.len() {
                    data.push(left.data[i]);
                } else {
                    data.push(right.data[i - left.data.len()])
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
        if s.len() > 3 {
            return Err("String is too long".to_owned());
        }

        let mut data = ArrayVec::<CharacterClass, 3>::new();
        for c in s.chars() {
            data.push(CharacterClass::Char(c))
        }

        Ok(Self { data })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct SearchPartTrigram {
    trigrams: Vec<Trigram>,
    docs_to_check: RoaringBitmap,
}

#[derive(Clone, PartialEq, Debug)]
pub struct RegexSearchPart {
    candidates: Vec<SearchPartTrigram>,
}

impl RegexSearchPart {
    pub fn concat(parts: &[RegexSearchPart]) -> Self {
        let permutations = PermutationIterator::new(
            &parts
                .iter()
                .map(|p| p.candidates.len() as u32)
                .collect::<Vec<u32>>(),
        );

        let mut new_candidates: Vec<SearchPartTrigram> = vec![];
        for permutation in permutations {
            let mut trigrams: Vec<&Vec<Trigram>> = vec![];
            let mut docs_bitmaps: Vec<&RoaringBitmap> = vec![];
            for (index, perm_index) in permutation.iter().enumerate() {
                let part = &parts[index].candidates[*perm_index as usize];
                trigrams.push(&part.trigrams);
                docs_bitmaps.push(&part.docs_to_check)
            }

            // Now merge trigrams.
            let merged_trigrams = merge_trigrams(&trigrams);
            let docs_to_check = intersect_bitmaps(&docs_bitmaps).unwrap();

            new_candidates.push({
                SearchPartTrigram {
                    trigrams: merged_trigrams,
                    docs_to_check,
                }
            })
        }

        Self {
            candidates: new_candidates,
        }
    }

    pub fn alternation(parts: &[RegexSearchPart]) -> Self {
        Self {
            candidates: parts
                .iter()
                .flat_map(|p| p.candidates.clone())
                .collect(),
        }
    }

    pub fn repeat(part: &RegexSearchPart, min: u32, max: Option<u32>) -> Self {
        let mut repeated_trigrams: Vec<SearchPartTrigram> = vec![];

        // Only repeat upto 3 really matters.
        let min_repeat_count = std::cmp::min(min, 3);
        let max_repeat_count = std::cmp::min(max.unwrap_or(3), 3);

        for repeat in min_repeat_count..(max_repeat_count + 1) {
            // Now pick (repeat)
            if repeat == 0 {
                repeated_trigrams.push(SearchPartTrigram {
                    trigrams: vec![],
                    docs_to_check: RoaringBitmap::new(),
                });

                continue;
            }

            let permutations = PermutationIterator::new(
                vec![part.candidates.len() as u32; repeat as usize].as_slice(),
            );

            for permutation in permutations {
                let mut doc_bitmaps = vec![];
                let mut trigrams = vec![];
                for index in permutation {
                    trigrams.push(&part.candidates[index as usize].trigrams);
                    doc_bitmaps
                        .push(&part.candidates[index as usize].docs_to_check);
                }

                repeated_trigrams.push(SearchPartTrigram {
                    trigrams: merge_trigrams(&trigrams),
                    docs_to_check: intersect_bitmaps(&doc_bitmaps).unwrap(),
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

fn regex_search(query: &str) -> anyhow::Result<Vec<RawPerFileSearchResult>> {
    let hir = regex_syntax::parse(query)?;

    print!("{hir:?}");
    match hir.kind() {
        regex_syntax::hir::HirKind::Empty => todo!(),
        regex_syntax::hir::HirKind::Literal(literal) => todo!(),
        regex_syntax::hir::HirKind::Class(class) => {}
        regex_syntax::hir::HirKind::Look(look) => {}
        regex_syntax::hir::HirKind::Repetition(repetition) => todo!(),
        regex_syntax::hir::HirKind::Capture(capture) => todo!(),
        regex_syntax::hir::HirKind::Concat(hirs) => todo!(),
        regex_syntax::hir::HirKind::Alternation(hirs) => todo!(),
    }

    Ok(vec![])
}

#[cfg(test)]
mod trigram_test {
    use super::*;

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
    use std::hash::RandomState;

    use super::*;

    #[cfg(test)]
    macro_rules! t {
        ($($s:literal),+ $(,)?) => {
            vec![$(Trigram::new($s)),+]
        };
    }

    #[test]
    fn test_concat() {
        let part1 = RegexSearchPart {
            candidates: vec![SearchPartTrigram {
                trigrams: t!("abc", "bcd"),
                docs_to_check: RoaringBitmap::from_iter([1, 2, 3]),
            }],
        };

        let part2 = RegexSearchPart {
            candidates: vec![SearchPartTrigram {
                trigrams: t!("12"),
                docs_to_check: RoaringBitmap::from_iter([3, 4]),
            }],
        };

        let part3 = RegexSearchPart {
            candidates: vec![SearchPartTrigram {
                trigrams: t!("xyz"),
                docs_to_check: RoaringBitmap::from_iter([3, 4, 5]),
            }],
        };

        assert_eq!(
            RegexSearchPart::concat(&[part1, part2, part3]),
            RegexSearchPart {
                candidates: vec![SearchPartTrigram {
                    trigrams: t!(
                        "abc", "bcd", "cd1", "d12", "12x", "2xy", "xyz"
                    ),
                    docs_to_check: RoaringBitmap::from_iter([3])
                }]
            }
        );
    }

    #[test]
    fn test_alternation() {
        let part1 = RegexSearchPart {
            candidates: vec![SearchPartTrigram {
                trigrams: t!("abc", "bcd"),
                docs_to_check: RoaringBitmap::from_iter([1, 2, 3]),
            }],
        };

        let part2 = RegexSearchPart {
            candidates: vec![SearchPartTrigram {
                trigrams: t!("12"),
                docs_to_check: RoaringBitmap::from_iter([3, 4]),
            }],
        };

        assert_eq!(
            RegexSearchPart::alternation(&[part1, part2]),
            RegexSearchPart {
                candidates: vec![
                    SearchPartTrigram {
                        trigrams: t!("abc", "bcd"),
                        docs_to_check: RoaringBitmap::from_iter([1, 2, 3])
                    },
                    SearchPartTrigram {
                        trigrams: t!("12"),
                        docs_to_check: RoaringBitmap::from_iter([3, 4])
                    }
                ]
            }
        );
    }

    #[test]
    fn test_repeat_simple() {
        let part = RegexSearchPart {
            candidates: vec![SearchPartTrigram {
                trigrams: t!("a"),
                docs_to_check: RoaringBitmap::from_iter([1, 2, 3]),
            }],
        };

        assert_eq!(
            RegexSearchPart::repeat(&part, 0, Some(100)),
            RegexSearchPart {
                candidates: vec![
                    SearchPartTrigram {
                        trigrams: vec![],
                        docs_to_check: RoaringBitmap::new()
                    },
                    SearchPartTrigram {
                        trigrams: t!("a"),
                        docs_to_check: RoaringBitmap::from_iter([1, 2, 3]),
                    },
                    SearchPartTrigram {
                        trigrams: t!("aa"),
                        docs_to_check: RoaringBitmap::from_iter([1, 2, 3]),
                    },
                    SearchPartTrigram {
                        trigrams: t!("aaa"),
                        docs_to_check: RoaringBitmap::from_iter([1, 2, 3]),
                    }
                ]
            }
        );
    }

    #[test]
    fn test_repeat_complex() {
        let part = RegexSearchPart {
            candidates: vec![
                SearchPartTrigram {
                    trigrams: t!("a"),
                    docs_to_check: RoaringBitmap::from_iter([1, 2, 3]),
                },
                SearchPartTrigram {
                    trigrams: t!("bc"),
                    docs_to_check: RoaringBitmap::from_iter([2, 3, 4]),
                },
                SearchPartTrigram {
                    trigrams: t!("xyz"),
                    docs_to_check: RoaringBitmap::from_iter([3, 4, 5]),
                },
            ],
        };

        assert_eq!(
            RegexSearchPart::repeat(&part, 2, Some(2)),
            RegexSearchPart {
                candidates: vec![
                    SearchPartTrigram {
                        trigrams: t!("aa"),
                        docs_to_check: RoaringBitmap::from_iter([1, 2, 3]),
                    },
                    SearchPartTrigram {
                        trigrams: t!("abc"),
                        docs_to_check: RoaringBitmap::from_iter([2, 3]),
                    },
                    SearchPartTrigram {
                        trigrams: t!("axy", "xyz"),
                        docs_to_check: RoaringBitmap::from_iter([3]),
                    },
                    SearchPartTrigram {
                        trigrams: t!("bca"),
                        docs_to_check: RoaringBitmap::from_iter([2, 3]),
                    },
                    SearchPartTrigram {
                        trigrams: t!("bcb", "cbc"),
                        docs_to_check: RoaringBitmap::from_iter([2, 3, 4]),
                    },
                    SearchPartTrigram {
                        trigrams: t!("bcx", "cxy", "xyz"),
                        docs_to_check: RoaringBitmap::from_iter([3, 4]),
                    },
                    SearchPartTrigram {
                        trigrams: t!("xyz", "yza"),
                        docs_to_check: RoaringBitmap::from_iter([3]),
                    },
                    SearchPartTrigram {
                        trigrams: t!("xyz", "yzb", "zbc"),
                        docs_to_check: RoaringBitmap::from_iter([3, 4]),
                    },
                    SearchPartTrigram {
                        trigrams: t!("xyz", "yzx", "zxy", "xyz"),
                        docs_to_check: RoaringBitmap::from_iter([3, 4, 5]),
                    },
                ]
            }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex() {
        // regex_search("[^abd]cd|def|asdf");
        // assert!(false);
    }
}
