use std::time::Instant;

struct WordSearchTimingStat {
    document_containing_word_find_timing: Vec<Instant>,
}

impl WordSearchTimingStat {
    pub fn record_document_finding_each_word(&mut self) {
        self.document_containing_word_find_timing
            .push(Instant::now());
    }
}
