use ahash::AHashMap;
use ahash::AHashSet;
use core::fmt;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use std::collections::BTreeSet;
use std::hash::Hash;
use std::hash::Hasher;

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

    #[inline]
    fn from_u128(v: u128) -> Self {
        unsafe { std::mem::transmute::<u128, TrigramKey>(v) }
    }
}

impl PartialEq for TrigramKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_u128() == other.as_u128()
    }
}

impl Eq for TrigramKey {}

impl PartialOrd for TrigramKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TrigramKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_u128()
            .swap_bytes()
            .cmp(&other.as_u128().swap_bytes())
    }
}

impl Hash for TrigramKey {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u128(self.as_u128());
    }
}

pub fn split_lines_to_tokens(
    lines: &[String],
    line_start_index: usize,
) -> AHashMap<TrigramKey, Vec<usize>> {
    let mut word_pos: AHashMap<TrigramKey, BTreeSet<usize>> = AHashMap::new();

    for (line_num, line) in lines.iter().enumerate() {
        split_by_trigram(line, &mut word_pos, line_num + line_start_index);
    }

    word_pos
        .into_iter()
        .map(|(word, lines)| (word, lines.into_iter().collect()))
        .collect()
}

pub fn split_lines_to_token_set(lines: &[String]) -> AHashSet<TrigramKey> {
    let mut s = AHashSet::new();

    for line in lines {
        split_by_trigram_to_set(line, &mut s);
    }

    s
}

fn split_by_trigram(
    line: &str,
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
            word_pos.entry(w).or_default().insert(line_num);
        }

        indexes[total_count % 3] = index;
        total_count += 1;
    }

    if total_count <= 2 && total_count > 0 {
        let word = first_and_second.last().unwrap();
        let w = TrigramKey::from_utf8(word);
        word_pos.entry(w).or_default().insert(line_num);
    }
}

fn split_by_trigram_to_set(line: &str, words: &mut AHashSet<TrigramKey>) {
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
            words.insert(w);
        }

        indexes[total_count % 3] = index;
        total_count += 1;
    }

    if total_count <= 2 && total_count > 0 {
        let word = first_and_second.last().unwrap();
        let w = TrigramKey::from_utf8(word);
        words.insert(w);
    }
}

impl Serialize for TrigramKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(
                std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap(),
            )
        } else {
            serializer.serialize_u128(self.as_u128())
        }
    }
}

impl<'de> Deserialize<'de> for TrigramKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s = <&str>::deserialize(deserializer)?;
            Ok(TrigramKey::from_utf8(s))
        } else {
            let v = u128::deserialize(deserializer)?;
            Ok(TrigramKey::from_u128(v))
        }
    }
}

impl fmt::Debug for TrigramKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap();
        f.write_str(s)
    }
}

impl AsRef<[u8]> for TrigramKey {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

impl From<&str> for TrigramKey {
    #[inline]
    fn from(s: &str) -> Self {
        TrigramKey::from_utf8(s)
    }
}

impl From<String> for TrigramKey {
    #[inline]
    fn from(s: String) -> Self {
        TrigramKey::from_utf8(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_key_test() {
        let mut v: Vec<TrigramKey> =
            vec!["a".into(), "bc".into(), "def".into(), "efa".into()];

        v.sort();
        assert_eq!(v, vec!["a".into(), "bc".into(), "def".into(), "efa".into()])
    }
}
