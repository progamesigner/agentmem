//! The default recall backend: an in-RAM content cache scanned per query.
//!
//! Holds each note's body in memory keyed by clean virtual path and answers
//! queries by case-insensitive substring and/or regex matching. It does not rank
//! by BM25 (the score is a match count) and does not support frontmatter property
//! filters — those require the tantivy backend.

use std::collections::BTreeMap;

use super::{BackendIndex, CompiledQuery, MAX_SNIPPET_LEN, MAX_SNIPPETS, RawHit, ScanResult};

/// An in-memory map of clean virtual path → note body.
#[derive(Default)]
pub(crate) struct SimpleIndex {
    docs: BTreeMap<String, String>,
}

impl BackendIndex for SimpleIndex {
    fn upsert(&mut self, clean_path: &str, body: &str) {
        self.docs.insert(clean_path.to_string(), body.to_string());
    }

    fn remove(&mut self, clean_path: &str) {
        self.docs.remove(clean_path);
    }

    fn query(&self, query: &CompiledQuery, byte_cap: usize) -> ScanResult {
        let mut hits = Vec::new();
        let mut truncated = false;
        // Only regex scans are byte-capped; substring matching is cheap.
        let cap_applies = query.regex.is_some();
        let mut scanned: usize = 0;

        // Deterministic iteration order (the engine re-sorts, but this keeps
        // truncation reproducible under the byte cap).
        for (path, body) in &self.docs {
            if cap_applies && scanned >= byte_cap {
                truncated = true;
                break;
            }
            scanned = scanned.saturating_add(body.len());

            if let Some(score) = score_doc(body, query)
                && score > 0.0
            {
                hits.push(RawHit {
                    clean_path: path.clone(),
                    raw_score: score,
                    snippets: snippets_for(body, query),
                });
            }
        }

        ScanResult { hits, truncated }
    }
}

/// Score a document, or `None` if it fails any supplied matcher. The score is the
/// total match count across the supplied matchers.
fn score_doc(body: &str, query: &CompiledQuery) -> Option<f32> {
    let mut score = 0usize;
    if let Some(needle) = &query.substring {
        let count = body.to_lowercase().matches(needle.as_str()).count();
        if count == 0 {
            return None;
        }
        score += count;
    }
    if let Some(regex) = &query.regex {
        let count = regex.find_iter(body).count();
        if count == 0 {
            return None;
        }
        score += count;
    }
    if score == 0 {
        // Neither matcher was supplied — defensive; the engine guarantees one.
        return None;
    }
    Some(score as f32)
}

/// Collect up to [`MAX_SNIPPETS`] matching lines, trimmed and length-capped.
fn snippets_for(body: &str, query: &CompiledQuery) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        if out.len() >= MAX_SNIPPETS {
            break;
        }
        let matched = match (&query.substring, &query.regex) {
            (Some(needle), _) if line.to_lowercase().contains(needle.as_str()) => true,
            (_, Some(regex)) if regex.is_match(line) => true,
            _ => false,
        };
        if matched {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                out.push(truncate_chars(trimmed, MAX_SNIPPET_LEN));
            }
        }
    }
    out
}

/// Truncate `s` to at most `max` bytes without splitting a char.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = s[..end].to_string();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiled(text: Option<&str>, regex: Option<&str>) -> CompiledQuery {
        CompiledQuery {
            raw_text: text.map(|s| s.to_string()),
            substring: text.map(|s| s.to_lowercase()),
            regex: regex.map(|r| regex::Regex::new(r).unwrap()),
            filters: Vec::new(),
        }
    }

    fn index() -> SimpleIndex {
        let mut idx = SimpleIndex::default();
        idx.upsert(
            "Agents/topics/rust.md",
            "The borrow checker enforces ownership.\nLifetimes are elided.",
        );
        idx.upsert("Agents/topics/python.md", "The GIL serializes threads.");
        idx
    }

    #[test]
    fn substring_match_is_case_insensitive() {
        let idx = index();
        let scan = idx.query(&compiled(Some("BORROW"), None), usize::MAX);
        assert_eq!(scan.hits.len(), 1);
        assert_eq!(scan.hits[0].clean_path, "Agents/topics/rust.md");
        assert!(scan.hits[0].snippets[0].to_lowercase().contains("borrow"));
        assert!(!scan.truncated);
    }

    #[test]
    fn regex_match_finds_pattern() {
        let idx = index();
        let scan = idx.query(&compiled(None, Some(r"\bGIL\b")), usize::MAX);
        assert_eq!(scan.hits.len(), 1);
        assert_eq!(scan.hits[0].clean_path, "Agents/topics/python.md");
    }

    #[test]
    fn no_match_returns_no_hits() {
        let idx = index();
        let scan = idx.query(&compiled(Some("kotlin"), None), usize::MAX);
        assert!(scan.hits.is_empty());
    }

    #[test]
    fn regex_scan_is_byte_capped() {
        let idx = index();
        // A tiny cap truncates the regex scan after the first document.
        let scan = idx.query(&compiled(None, Some("the")), 1);
        assert!(scan.truncated);
    }

    #[test]
    fn remove_drops_the_doc() {
        let mut idx = index();
        idx.remove("Agents/topics/rust.md");
        let scan = idx.query(&compiled(Some("borrow"), None), usize::MAX);
        assert!(scan.hits.is_empty());
    }
}
