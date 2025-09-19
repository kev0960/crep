pub enum SearchStrategy {
    Word(String),
    Any(Vec<SearchStrategy>),
    Every(Vec<SearchStrategy>),
}
