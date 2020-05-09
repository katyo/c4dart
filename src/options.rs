use regex::Regex;

#[derive(Debug, Clone)]
pub struct Options {
    /// Name match
    pub match_: Regex,

    /// Name replace
    pub replace: String,
}

