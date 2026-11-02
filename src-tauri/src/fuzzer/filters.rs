use serde::{Deserialize, Serialize};

use super::engine::FuzzResult;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct FilterConfig {
    pub exclude_status: Vec<u16>,
    /// Empty = include every status except whatever is excluded above.
    pub include_status: Vec<u16>,
    pub exclude_size: Vec<u64>,
    pub exclude_words: Vec<u64>,
    pub exclude_lines: Vec<u64>,
}

impl FilterConfig {
    /// Decision: filtering happens in the backend, on the full result,
    /// before it's ever sent to the UI — not as a client-side table filter
    /// over an already-fetched result set. With large wordlists that keeps
    /// the frontend from ever having to hold (or re-render) thousands of
    /// rows the user has already said they don't want to see.
    pub fn matches(&self, r: &FuzzResult) -> bool {
        if !self.include_status.is_empty() && !self.include_status.contains(&r.status) {
            return false;
        }
        if self.exclude_status.contains(&r.status) {
            return false;
        }
        if self.exclude_size.contains(&r.size) {
            return false;
        }
        if self.exclude_words.contains(&r.words) {
            return false;
        }
        if self.exclude_lines.contains(&r.lines) {
            return false;
        }
        true
    }
}
