use fst::Set;
use roaring::RoaringBitmap;
use std::collections::HashMap;

pub type FileToWordPos = HashMap<usize, HashMap<String, Vec<(usize, usize)>>>;

pub struct Index {
    // List of all files.
    pub files: Vec<String>,

    // Word to the bitmap to indicate the file that contains the word.
    pub word_to_bitmap: HashMap<String, RoaringBitmap>,

    // File index to the map of word and it's positions in the file (line, pos).
    pub file_to_word_pos: FileToWordPos,

    // List of all words.
    pub words: Set<Vec<u8>>,
}

impl Index {
    pub fn new(
        files: Vec<String>,
        word_to_bitmap: HashMap<String, RoaringBitmap>,
        file_to_word_pos: HashMap<usize, HashMap<String, Vec<(usize, usize)>>>,
    ) -> Self {
        let mut keys = word_to_bitmap.keys().cloned().collect::<Vec<_>>();
        keys.sort();

        let set = Set::from_iter(keys).unwrap();

        Self {
            files,
            word_to_bitmap,
            file_to_word_pos,
            words: set,
        }
    }
}
