use std::fs;
use std::path::Path;

pub struct BundledWordlist {
    pub id: &'static str,
    pub label: &'static str,
    pub content: &'static str,
}

/// Decision: a couple of small lists are compiled straight into the binary
/// with `include_str!`, so the app is useful the moment it's installed —
/// no download step, no "where do I get a wordlist" friction for a first
/// run. Anything bigger or custom is read from disk at runtime instead of
/// embedded, which is what keeps the compiled binary itself small and lets
/// the user bring their own lists, per the original brief.
pub const BUNDLED_WORDLISTS: &[BundledWordlist] = &[
    BundledWordlist {
        id: "common-small",
        label: "Common paths (~150 words)",
        content: include_str!("../wordlists/common-small.txt"),
    },
    BundledWordlist {
        id: "api-small",
        label: "API endpoints (~95 words)",
        content: include_str!("../wordlists/api-small.txt"),
    },
];

pub fn load_from_str(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

pub fn load_from_file(path: &str) -> Result<Vec<String>, String> {
    let data = fs::read_to_string(Path::new(path))
        .map_err(|e| format!("Could not read wordlist file: {e}"))?;
    Ok(load_from_str(&data))
}

pub fn get_bundled(id: &str) -> Option<Vec<String>> {
    BUNDLED_WORDLISTS
        .iter()
        .find(|w| w.id == id)
        .map(|w| load_from_str(w.content))
}
