//! YAML frontmatter extraction and merging.
//!
//! Obsidian "properties" live in a leading `---` fenced YAML block. This module
//! pulls those properties out as a JSON object and returns the body with the block
//! removed, so the recall indexer can search the prose and filter on the properties
//! separately, and the property tools can read and merge them as structured data.
//! Parsing happens only here — the storage layer stays byte-exact and
//! frontmatter-agnostic. For [`parse`], malformed or absent frontmatter is never
//! an error: it yields empty properties and the original content as the body.
//! [`merge`] is stricter: it refuses a leading fence it cannot interpret rather
//! than clobber it.

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

/// Why [`merge`] refused to touch a note's frontmatter. Every variant means the
/// file was left unchanged; a human fixes the block in an editor.
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("frontmatter fence is not terminated by a closing `---` line")]
    UnterminatedFence,
    #[error("existing frontmatter is not valid YAML: {0}")]
    InvalidYaml(String),
    #[error("existing frontmatter is not a YAML mapping")]
    NotAMapping,
    #[error("failed to serialize frontmatter: {0}")]
    Serialize(String),
}

/// Merge `updates` into the note's frontmatter properties: each key upserts its
/// JSON value, an explicit `null` deletes the key. The block is re-emitted as
/// `---\n<yaml>\n---\n` with top-level keys in sorted order (comments and YAML
/// formatting are not preserved), created when the note has none, and omitted
/// entirely when the merge empties it. The body is re-attached byte-identical.
/// A leading fence that cannot be interpreted as a property block is refused.
pub fn merge(
    content: &str,
    updates: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, MergeError> {
    let (mut props, body) = split_existing(content)?;
    for (key, value) in updates {
        if value.is_null() {
            props.remove(key);
        } else {
            props.insert(key.clone(), value.clone());
        }
    }
    if props.is_empty() {
        return Ok(body.to_string());
    }
    let ordered: std::collections::BTreeMap<&String, &serde_json::Value> = props.iter().collect();
    let yaml = serde_yaml::to_string(&ordered).map_err(|e| MergeError::Serialize(e.to_string()))?;
    Ok(format!("---\n{yaml}---\n{body}"))
}

/// Split `content` into its existing properties and body for a merge. Unlike
/// [`parse`], a leading fence that does not yield a YAML mapping is an error —
/// blindly prepending a fresh block would silently demote the existing one to
/// body text. An empty block (`---\n---\n`) counts as an empty mapping.
fn split_existing(
    content: &str,
) -> Result<(serde_json::Map<String, serde_json::Value>, &str), MergeError> {
    let Some(rest) = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
    else {
        return Ok((serde_json::Map::new(), content));
    };
    let Some((yaml, body)) = split_closing_fence(rest) else {
        return Err(MergeError::UnterminatedFence);
    };
    if yaml.trim().is_empty() {
        return Ok((serde_json::Map::new(), body));
    }
    match serde_yaml::from_str::<serde_json::Value>(yaml) {
        Ok(serde_json::Value::Object(map)) => Ok((map, body)),
        Ok(_) => Err(MergeError::NotAMapping),
        Err(e) => Err(MergeError::InvalidYaml(e.to_string())),
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

    /// Build an updates map from a JSON literal.
    fn updates(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn merge_upserts_and_deletes_keys() {
        let merged = merge(
            "---\nstatus: draft\npriority: 2\n---\nThe body.\n",
            &updates(serde_json::json!({
                "status": "done",
                "reviewed": true,
                "priority": null,
            })),
        )
        .unwrap();
        assert_eq!(
            merged,
            "---\nreviewed: true\nstatus: done\n---\nThe body.\n"
        );
        let fm = parse(&merged);
        assert_eq!(
            fm.props,
            serde_json::json!({ "status": "done", "reviewed": true })
        );
        assert_eq!(fm.body, "The body.\n");
    }

    #[test]
    fn merge_emits_sorted_top_level_keys() {
        let merged = merge(
            "---\nzebra: 1\n---\nbody\n",
            &updates(serde_json::json!({ "alpha": 2 })),
        )
        .unwrap();
        assert_eq!(merged, "---\nalpha: 2\nzebra: 1\n---\nbody\n");
    }

    #[test]
    fn merge_creates_block_when_absent() {
        let merged = merge(
            "Just body, no fence.\n",
            &updates(serde_json::json!({ "status": "draft" })),
        )
        .unwrap();
        assert_eq!(merged, "---\nstatus: draft\n---\nJust body, no fence.\n");
    }

    #[test]
    fn merge_removes_block_when_emptied() {
        let merged = merge(
            "---\nstatus: draft\n---\nThe body.\n",
            &updates(serde_json::json!({ "status": null })),
        )
        .unwrap();
        assert_eq!(merged, "The body.\n");
    }

    #[test]
    fn merge_treats_empty_block_as_empty_mapping() {
        let merged = merge(
            "---\n---\nbody\n",
            &updates(serde_json::json!({ "status": "done" })),
        )
        .unwrap();
        assert_eq!(merged, "---\nstatus: done\n---\nbody\n");
    }

    #[test]
    fn merge_refuses_fences_it_cannot_interpret() {
        let upd = updates(serde_json::json!({ "status": "done" }));
        assert!(matches!(
            merge("---\n: : not valid : :\n---\nbody\n", &upd),
            Err(MergeError::InvalidYaml(_))
        ));
        assert!(matches!(
            merge("---\njust a scalar\n---\nbody\n", &upd),
            Err(MergeError::NotAMapping)
        ));
        assert!(matches!(
            merge("---\ntags: [a]\nno closing fence\n", &upd),
            Err(MergeError::UnterminatedFence)
        ));
    }

    #[test]
    fn merge_round_trips_nested_values_and_arrays() {
        let props = serde_json::json!({
            "tags": ["rust", "async"],
            "meta": { "owner": "tony", "score": 9.5 },
            "count": 3,
            "done": false,
        });
        let merged = merge("body\n", &updates(props.clone())).unwrap();
        assert_eq!(parse(&merged).props, props);
        assert_eq!(parse(&merged).body, "body\n");
    }

    #[test]
    fn merge_keeps_body_byte_identical_including_crlf() {
        let body = "line one\r\n\r\n\tindented\nmixed endings\r\n";
        let content = format!("---\r\nstatus: draft\r\n---\r\n{body}");
        let merged = merge(&content, &updates(serde_json::json!({ "n": 1 }))).unwrap();
        assert!(merged.ends_with(body));
        assert_eq!(merged, format!("---\nn: 1\nstatus: draft\n---\n{body}"));
    }
}
