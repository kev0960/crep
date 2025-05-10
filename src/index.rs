use fst::Set;
use roaring::RoaringBitmap;
use std::collections::HashMap;

pub struct Index {
    pub files: Vec<String>,
    pub word_to_bitmap: HashMap<String, RoaringBitmap>,
    pub words: Set<Vec<u8>>,
}

impl Index {
    pub fn new(files: Vec<String>, word_to_bitmap: HashMap<String, RoaringBitmap>) -> Self {
        let mut keys = word_to_bitmap.keys().cloned().collect::<Vec<_>>();
        keys.sort();

        let set = Set::from_iter(keys.into_iter()).unwrap();

        Self {
            files,
            word_to_bitmap,
            words: set,
        }
    }
}
