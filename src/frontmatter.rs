//! YAML frontmatter extraction for the tantivy recall backend.
//!
//! Obsidian "properties" live in a leading `---` fenced YAML block. This module
//! pulls those properties out as a JSON object and returns the body with the block
//! removed, so the indexer can search the prose and filter on the properties
//! separately. Parsing happens only in the indexer — the storage layer stays
//! byte-exact and frontmatter-agnostic. Malformed or absent frontmatter is never
//! an error: it yields empty properties and the original content as the body.

/// The result of splitting a note into its frontmatter properties and its body.
pub struct Frontmatter {
    /// The parsed properties as a JSON object (empty when there is no frontmatter).
    pub props: serde_json::Value,
    /// The note body with the frontmatter block removed.
    pub body: String,
}

/// Parse a leading `---` YAML frontmatter block, if present.
pub fn parse(content: &str) -> Frontmatter {
    let empty = || serde_json::Value::Object(serde_json::Map::new());
    let rest = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"));
    if let Some(rest) = rest
        && let Some((yaml, body)) = split_closing_fence(rest)
        && let Ok(value) = serde_yaml::from_str::<serde_json::Value>(yaml)
        && value.is_object()
    {
        return Frontmatter {
            props: value,
            body: body.to_string(),
        };
    }
    Frontmatter {
        props: empty(),
        body: content.to_string(),
    }
}

/// Split `rest` at the first line that is exactly `---`, returning the YAML before
/// it and the body after it.
fn split_closing_fence(rest: &str) -> Option<(&str, &str)> {
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_properties_and_strips_block() {
        let fm = parse("---\ntags: [rust, async]\nstatus: draft\n---\nThe body text.\n");
        assert_eq!(fm.props["status"], "draft");
        assert_eq!(fm.props["tags"][0], "rust");
        assert_eq!(fm.body, "The body text.\n");
    }

    #[test]
    fn no_frontmatter_returns_content_verbatim() {
        let fm = parse("Just body, no fence.\n");
        assert!(fm.props.as_object().unwrap().is_empty());
        assert_eq!(fm.body, "Just body, no fence.\n");
    }

    #[test]
    fn malformed_yaml_is_non_fatal() {
        let fm = parse("---\n: : not valid : :\n---\nbody\n");
        assert!(fm.props.as_object().unwrap().is_empty());
        // Body is the original content (parse failed, nothing stripped).
        assert!(fm.body.contains("body"));
    }

    #[test]
    fn unterminated_fence_is_not_frontmatter() {
        let fm = parse("---\ntags: [a]\nno closing fence\n");
        assert!(fm.props.as_object().unwrap().is_empty());
        assert!(fm.body.starts_with("---"));
    }
}
