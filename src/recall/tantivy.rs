//! The opt-in tantivy recall backend (the `recall-tantivy` feature).
//!
//! Holds an in-RAM tantivy index (a `RamDirectory`) per region — nothing is
//! written to disk. Full-text `query` is BM25-ranked with snippet generation;
//! `regex` and frontmatter property `filters` are applied as a post-filter over
//! the candidate documents (BM25 results when a text query narrows the set, or a
//! bounded full scan otherwise). Frontmatter properties are parsed at index time
//! and stored as JSON for filtering.

use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, QueryParser};
use tantivy::schema::{Field, OwnedValue, STORED, STRING, Schema, TEXT, TantivyDocument};
use tantivy::snippet::SnippetGenerator;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, Term};

use crate::frontmatter;

use super::{
    BackendIndex, CompiledQuery, FilterOp, MAX_SNIPPET_LEN, MAX_SNIPPETS, PropertyFilter, RawHit,
    ScanResult,
};

/// Writer heap budget; tantivy requires a few MiB minimum.
const WRITER_HEAP: usize = 30_000_000;

pub(crate) struct TantivyIndex {
    index: Index,
    writer: IndexWriter,
    reader: IndexReader,
    path: Field,
    body: Field,
    props_json: Field,
}

impl TantivyIndex {
    pub(crate) fn new() -> TantivyIndex {
        let mut builder = Schema::builder();
        // `path`: stored clean virtual path, and the unique key for upsert/delete.
        let path = builder.add_text_field("path", STRING | STORED);
        // `body`: frontmatter-stripped prose, BM25-indexed and stored for snippets.
        let body = builder.add_text_field("body", TEXT | STORED);
        // `props_json`: the serialized frontmatter properties, stored for post-filtering.
        let props_json = builder.add_text_field("props_json", STORED);
        let schema = builder.build();

        let index = Index::create_in_ram(schema);
        let writer = index
            .writer(WRITER_HEAP)
            .expect("create tantivy in-RAM writer");
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .expect("create tantivy reader");
        TantivyIndex {
            index,
            writer,
            reader,
            path,
            body,
            props_json,
        }
    }

    /// Read the three stored fields off a document.
    fn fields(&self, doc: &TantivyDocument) -> (String, String, serde_json::Value) {
        let path = stored_str(doc, self.path).unwrap_or_default();
        let body = stored_str(doc, self.body).unwrap_or_default();
        let props = stored_str(doc, self.props_json)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
        (path, body, props)
    }

    /// Build snippets: the BM25 fragment when available, else matching lines.
    fn snippets(
        &self,
        doc: &TantivyDocument,
        body: &str,
        compiled: &CompiledQuery,
        generator: Option<&SnippetGenerator>,
    ) -> Vec<String> {
        if let Some(generator) = generator {
            let fragment = generator
                .snippet_from_doc(doc)
                .fragment()
                .trim()
                .to_string();
            if !fragment.is_empty() {
                return vec![truncate(&fragment, MAX_SNIPPET_LEN)];
            }
        }
        let mut out = Vec::new();
        for line in body.lines() {
            if out.len() >= MAX_SNIPPETS {
                break;
            }
            let matched = match &compiled.regex {
                Some(re) => re.is_match(line),
                None => true,
            };
            if matched {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    out.push(truncate(trimmed, MAX_SNIPPET_LEN));
                }
            }
        }
        out
    }
}

impl BackendIndex for TantivyIndex {
    fn upsert(&mut self, clean_path: &str, body: &str) {
        // delete-then-add (the delete carries an earlier opstamp, so the new doc
        // survives the next commit) makes this an upsert keyed by `path`.
        self.writer
            .delete_term(Term::from_field_text(self.path, clean_path));
        let parsed = frontmatter::parse(body);
        let props_json = serde_json::to_string(&parsed.props).unwrap_or_else(|_| "{}".to_string());
        let mut doc = TantivyDocument::default();
        doc.add_text(self.path, clean_path);
        doc.add_text(self.body, &parsed.body);
        doc.add_text(self.props_json, &props_json);
        let _ = self.writer.add_document(doc);
    }

    fn remove(&mut self, clean_path: &str) {
        self.writer
            .delete_term(Term::from_field_text(self.path, clean_path));
    }

    fn flush(&mut self) {
        if self.writer.commit().is_ok() {
            let _ = self.reader.reload();
        }
    }

    fn query(&self, compiled: &CompiledQuery, byte_cap: usize) -> ScanResult {
        let searcher = self.reader.searcher();
        let mut hits = Vec::new();
        let mut truncated = false;

        if let Some(text) = &compiled.raw_text {
            // BM25 over the narrowed candidate set, then regex/filter post-checks.
            let parser = QueryParser::for_index(&self.index, vec![self.body]);
            let query = match parser.parse_query(text).or_else(|_| {
                // Lenient retry as a quoted phrase for inputs with query syntax.
                parser.parse_query(&format!("\"{}\"", text.replace('"', " ")))
            }) {
                Ok(query) => query,
                Err(_) => return ScanResult { hits, truncated },
            };
            let generator = SnippetGenerator::create(&searcher, &*query, self.body).ok();
            let limit = searcher.num_docs().max(1) as usize;
            let top = searcher
                .search(&query, &TopDocs::with_limit(limit))
                .unwrap_or_default();
            for (score, address) in top {
                let Ok(doc) = searcher.doc::<TantivyDocument>(address) else {
                    continue;
                };
                let (path, body, props) = self.fields(&doc);
                if !passes(&body, &props, compiled) {
                    continue;
                }
                let snippets = self.snippets(&doc, &body, compiled, generator.as_ref());
                hits.push(RawHit {
                    clean_path: path,
                    raw_score: score,
                    snippets,
                });
            }
        } else {
            // No text query: a bounded full scan filtered by regex and/or properties.
            let limit = searcher.num_docs().max(1) as usize;
            let all = searcher
                .search(&AllQuery, &TopDocs::with_limit(limit))
                .unwrap_or_default();
            let cap_applies = compiled.regex.is_some();
            let mut scanned = 0usize;
            for (_, address) in all {
                if cap_applies && scanned >= byte_cap {
                    truncated = true;
                    break;
                }
                let Ok(doc) = searcher.doc::<TantivyDocument>(address) else {
                    continue;
                };
                let (path, body, props) = self.fields(&doc);
                scanned = scanned.saturating_add(body.len());
                if !passes(&body, &props, compiled) {
                    continue;
                }
                let raw_score = match &compiled.regex {
                    Some(re) => re.find_iter(&body).count() as f32,
                    None => 1.0,
                };
                let snippets = self.snippets(&doc, &body, compiled, None);
                hits.push(RawHit {
                    clean_path: path,
                    raw_score,
                    snippets,
                });
            }
        }

        ScanResult { hits, truncated }
    }
}

/// A document passes when its body matches the regex (if any) and its properties
/// satisfy every filter.
fn passes(body: &str, props: &serde_json::Value, compiled: &CompiledQuery) -> bool {
    if let Some(re) = &compiled.regex
        && !re.is_match(body)
    {
        return false;
    }
    compiled.filters.iter().all(|f| eval_filter(props, f))
}

/// Evaluate one property predicate against the parsed frontmatter.
fn eval_filter(props: &serde_json::Value, filter: &PropertyFilter) -> bool {
    let value = props.get(&filter.key);
    match filter.op {
        FilterOp::Exists => value.is_some(),
        FilterOp::Eq => value.is_some_and(|v| scalar_eq(v, filter.value.as_deref())),
        FilterOp::Contains => match value {
            Some(serde_json::Value::Array(items)) => {
                items.iter().any(|e| scalar_eq(e, filter.value.as_deref()))
            }
            Some(serde_json::Value::String(s)) => filter
                .value
                .as_deref()
                .is_some_and(|needle| s.contains(needle)),
            _ => false,
        },
        FilterOp::Gt | FilterOp::Lt | FilterOp::Ge | FilterOp::Le => {
            compare(value, filter.value.as_deref(), filter.op)
        }
    }
}

/// Equality between a JSON scalar and the filter's string value.
fn scalar_eq(value: &serde_json::Value, want: Option<&str>) -> bool {
    let Some(want) = want else { return false };
    match value {
        serde_json::Value::String(s) => s == want,
        serde_json::Value::Number(n) => n.to_string() == want,
        serde_json::Value::Bool(b) => b.to_string() == want,
        _ => false,
    }
}

/// Ordered comparison: numeric when both sides parse as numbers, else lexical.
fn compare(value: Option<&serde_json::Value>, want: Option<&str>, op: FilterOp) -> bool {
    let (Some(value), Some(want)) = (value, want) else {
        return false;
    };
    let lhs_str = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => return false,
    };
    let ordering = match (lhs_str.parse::<f64>(), want.parse::<f64>()) {
        (Ok(a), Ok(b)) => a.partial_cmp(&b),
        _ => Some(lhs_str.as_str().cmp(want)),
    };
    match ordering {
        Some(std::cmp::Ordering::Greater) => matches!(op, FilterOp::Gt | FilterOp::Ge),
        Some(std::cmp::Ordering::Less) => matches!(op, FilterOp::Lt | FilterOp::Le),
        Some(std::cmp::Ordering::Equal) => matches!(op, FilterOp::Ge | FilterOp::Le),
        None => false,
    }
}

/// Read a stored text field as a `String`.
fn stored_str(doc: &TantivyDocument, field: Field) -> Option<String> {
    doc.get_first(field).and_then(|value| match value {
        OwnedValue::Str(s) => Some(s.clone()),
        _ => None,
    })
}

/// Truncate to at most `max` bytes on a char boundary, adding an ellipsis.
fn truncate(s: &str, max: usize) -> String {
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

    fn compiled(
        text: Option<&str>,
        regex: Option<&str>,
        filters: Vec<PropertyFilter>,
    ) -> CompiledQuery {
        CompiledQuery {
            raw_text: text.map(|s| s.to_string()),
            substring: text.map(|s| s.to_lowercase()),
            regex: regex.map(|r| regex::Regex::new(r).unwrap()),
            filters,
        }
    }

    fn index() -> TantivyIndex {
        let mut idx = TantivyIndex::new();
        idx.upsert(
            "Agents/topics/rust.md",
            "---\ntags: [rust, systems]\nstatus: published\nweight: 5\n---\nThe borrow checker enforces ownership.",
        );
        idx.upsert(
            "Agents/topics/python.md",
            "---\ntags: [python]\nstatus: draft\nweight: 2\n---\nThe GIL serializes threads.",
        );
        idx.flush();
        idx
    }

    #[test]
    fn bm25_full_text_ranks_and_snippets() {
        let idx = index();
        let scan = idx.query(&compiled(Some("borrow"), None, vec![]), usize::MAX);
        assert_eq!(scan.hits.len(), 1);
        assert_eq!(scan.hits[0].clean_path, "Agents/topics/rust.md");
        assert!(scan.hits[0].raw_score > 0.0);
        assert!(!scan.hits[0].snippets.is_empty());
        // Frontmatter is stripped from the indexed body: a property word is not prose.
        let none = idx.query(&compiled(Some("published"), None, vec![]), usize::MAX);
        assert!(none.hits.is_empty());
    }

    #[test]
    fn property_filter_eq_and_contains() {
        let idx = index();
        let eq = idx.query(
            &compiled(
                None,
                None,
                vec![PropertyFilter {
                    key: "status".into(),
                    op: FilterOp::Eq,
                    value: Some("draft".into()),
                }],
            ),
            usize::MAX,
        );
        assert_eq!(eq.hits.len(), 1);
        assert_eq!(eq.hits[0].clean_path, "Agents/topics/python.md");

        let contains = idx.query(
            &compiled(
                None,
                None,
                vec![PropertyFilter {
                    key: "tags".into(),
                    op: FilterOp::Contains,
                    value: Some("systems".into()),
                }],
            ),
            usize::MAX,
        );
        assert_eq!(contains.hits.len(), 1);
        assert_eq!(contains.hits[0].clean_path, "Agents/topics/rust.md");
    }

    #[test]
    fn property_filter_numeric_comparison() {
        let idx = index();
        let scan = idx.query(
            &compiled(
                None,
                None,
                vec![PropertyFilter {
                    key: "weight".into(),
                    op: FilterOp::Gt,
                    value: Some("3".into()),
                }],
            ),
            usize::MAX,
        );
        assert_eq!(scan.hits.len(), 1);
        assert_eq!(scan.hits[0].clean_path, "Agents/topics/rust.md");
    }

    #[test]
    fn text_plus_filter_compose() {
        let idx = index();
        let scan = idx.query(
            &compiled(
                Some("threads"),
                None,
                vec![PropertyFilter {
                    key: "status".into(),
                    op: FilterOp::Eq,
                    value: Some("published".into()),
                }],
            ),
            usize::MAX,
        );
        // "threads" matches python (draft), but the published filter excludes it.
        assert!(scan.hits.is_empty());
    }

    #[test]
    fn regex_over_candidates() {
        let idx = index();
        let scan = idx.query(&compiled(None, Some(r"\bGIL\b"), vec![]), usize::MAX);
        assert_eq!(scan.hits.len(), 1);
        assert_eq!(scan.hits[0].clean_path, "Agents/topics/python.md");
    }

    #[test]
    fn remove_then_flush_drops_doc() {
        let mut idx = index();
        idx.remove("Agents/topics/rust.md");
        idx.flush();
        let scan = idx.query(&compiled(Some("borrow"), None, vec![]), usize::MAX);
        assert!(scan.hits.is_empty());
    }
}
