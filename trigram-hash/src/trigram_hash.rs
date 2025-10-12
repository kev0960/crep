use ahash::AHashMap;
use ahash::AHashSet;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct TrigramKey {
    bytes: [u8; 12],
    len: u8,
    _padding: [u8; 3],
}

impl TrigramKey {
    #[inline]
    pub fn from_utf8(s: &str) -> Self {
        let len = s.len() as u8;
        debug_assert!(len <= 12);

        let mut bytes = [0u8; 12];
        bytes[..len as usize].copy_from_slice(s.as_bytes());

        Self {
            bytes,
            len,
            _padding: [0u8; 3],
        }
    }

    #[inline]
    fn as_u128(&self) -> u128 {
        unsafe { std::mem::transmute::<TrigramKey, u128>(*self) }
    }
}

impl PartialEq for TrigramKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_u128() == other.as_u128()
    }
}

impl Eq for TrigramKey {}

impl Hash for TrigramKey {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u128(self.as_u128());
    }
}

pub fn split_lines_to_tokens(
    lines: &[String],
    line_start_index: usize,
) -> (AHashSet<TrigramKey>, AHashMap<TrigramKey, Vec<usize>>) {
    let mut total_words = AHashSet::new();
    let mut word_pos: AHashMap<TrigramKey, BTreeSet<usize>> = AHashMap::new();

    for (line_num, line) in lines.iter().enumerate() {
        split_by_trigram(
            line,
            &mut total_words,
            &mut word_pos,
            line_num + line_start_index,
        );
    }

    (
        total_words,
        word_pos
            .into_iter()
            .map(|(word, lines)| (word, lines.into_iter().collect()))
            .collect(),
    )
}

fn split_by_trigram(
    line: &str,
    total_words: &mut AHashSet<TrigramKey>,
    word_pos: &mut AHashMap<TrigramKey, BTreeSet<usize>>,
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
            let w = TrigramKey::from_utf8(word);
            total_words.insert(w);
            word_pos.entry(w).or_default().insert(line_num);
        }

        indexes[total_count % 3] = index;
        total_count += 1;
    }

    if total_count <= 2 && total_count > 0 {
        let word = first_and_second.last().unwrap();
        let w = TrigramKey::from_utf8(word);
        total_words.insert(w);
        word_pos.entry(w).or_default().insert(line_num);
    }
}
