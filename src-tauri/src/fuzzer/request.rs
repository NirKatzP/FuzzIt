use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The token the user places in the URL, a header value, or the body to
/// mark where each wordlist entry gets substituted in — same convention
/// ffuf itself uses, so anyone who already knows ffuf can use this tool
/// without relearning a syntax.
pub const FUZZ_KEYWORD: &str = "FUZZ";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FuzzConfig {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub concurrency: usize,
    pub timeout_ms: u64,
    pub follow_redirects: bool,
    /// Optional per-request delay in ms (politeness / rate limiting). 0 = off.
    pub delay_ms: u64,
}

/// A request with FUZZ already substituted in every field that can carry it.
pub struct BuiltRequest {
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

/// Decision: one substitution pass over url + headers + body, instead of
/// separate "directory fuzzing mode" / "parameter fuzzing mode" / "vhost
/// fuzzing mode" code paths. Putting FUZZ in the URL path gives directory
/// and file fuzzing; putting it in a header value (e.g. the Host header)
/// gives vhost fuzzing; putting it in the body or a query-string header
/// gives parameter fuzzing. One engine, three use cases, because the
/// *mechanism* is identical — only where the user types FUZZ changes.
pub fn build_request(config: &FuzzConfig, word: &str) -> BuiltRequest {
    let url = config.url.replace(FUZZ_KEYWORD, word);
    let mut headers: HashMap<String, String> = config
        .headers
        .iter()
        .map(|(k, v)| (k.clone(), v.replace(FUZZ_KEYWORD, word)))
        .collect();
    let body = config
        .body
        .as_ref()
        .filter(|b| !b.is_empty())
        .map(|b| b.replace(FUZZ_KEYWORD, word));

    // Without a Content-Type most servers won't parse the body at all, which
    // silently turns parameter fuzzing into a no-op. A user-supplied header
    // always wins; otherwise guess from the body's shape.
    if let Some(b) = &body {
        if !headers.keys().any(|k| k.eq_ignore_ascii_case("content-type")) {
            let trimmed = b.trim_start();
            let content_type = if trimmed.starts_with('{') || trimmed.starts_with('[') {
                "application/json"
            } else {
                "application/x-www-form-urlencoded"
            };
            headers.insert("Content-Type".into(), content_type.into());
        }
    }

    BuiltRequest { url, headers, body }
}
