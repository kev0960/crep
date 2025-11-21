#[derive(Clone, Eq, Hash, PartialEq)]
pub enum Query {
    Plain(String),
    Regex(String),
}
