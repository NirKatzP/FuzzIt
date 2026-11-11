use std::fs;
use std::path::Path;

pub struct BundledWordlist {
    pub id: &'static str,
    pub label: &'static str,
    pub content: &'static str,
}

/// Decision: the lists are compiled straight into the binary with
/// `include_str!`, so the app is useful the moment it's installed — no
/// download step, no "where do I get a wordlist" friction for a first run.
/// Besides the small starter lists this includes standard SecLists lists up
/// to the 220K-entry DirBuster medium list (~2.5MB of binary size in
/// total); sources and licenses are in `wordlists/THIRD_PARTY_NOTICES.md`.
/// Custom lists are read from disk at runtime instead.
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
    BundledWordlist {
        id: "raft-medium-directories",
        label: "Directories, raft-medium (~30K words)",
        content: include_str!("../wordlists/raft-medium-directories.txt"),
    },
    BundledWordlist {
        id: "raft-medium-files",
        label: "Files, raft-medium (~17K words)",
        content: include_str!("../wordlists/raft-medium-files.txt"),
    },
    BundledWordlist {
        id: "api-medium",
        label: "API endpoints, extended (~520 words)",
        content: include_str!("../wordlists/api-medium.txt"),
    },
    BundledWordlist {
        id: "directory-list-medium",
        label: "Directories, DirBuster medium (~220K words)",
        content: include_str!("../wordlists/directory-list-2.3-medium.txt"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_lists_load_with_expected_sizes() {
        let expected = [
            ("common-small", 100..200),
            ("api-small", 80..100),
            ("api-medium", 450..600),
            ("raft-medium-files", 16_000..18_000),
            ("raft-medium-directories", 29_000..31_000),
            ("directory-list-medium", 215_000..225_000),
        ];
        assert_eq!(expected.len(), BUNDLED_WORDLISTS.len());
        for (id, range) in expected {
            let words = get_bundled(id).unwrap_or_else(|| panic!("missing list {id}"));
            println!("{id}: {}", words.len());
            assert!(range.contains(&words.len()), "{id}: {} words", words.len());
            // License/comment headers must not leak into the requests.
            assert!(words.iter().all(|w| !w.starts_with('#')), "{id}");
        }
    }
}
