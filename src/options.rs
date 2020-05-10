use std::path::PathBuf;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct Options {
    /// Library wrapper class name
    pub class_name: String,
    
    /// Includes paths
    pub include_paths: Vec<PathBuf>,
    
    /// Detect system includes paths
    pub detect_isystem: bool,
    
    /// Name matching regexp
    pub names_match: Regex,

    /// Name replace pattern
    pub names_replace: String,
}

