//! A minimal `{{key}}` substitution template — the lax sibling of
//! [`crate::scheme::Scheme`].
//!
//! Where the scheme is a strict path grammar with an exact scope-key contract, a
//! template is free prose with `{{key}}` placeholders. Rendering substitutes
//! recognised keys from a context map and leaves unknown tokens verbatim,
//! reporting them so the caller can log once. There are no loops or conditionals
//! — substitution only. The type knows nothing about files, scope, or "missing";
//! that orchestration lives in [`crate::session_context`].

use std::collections::BTreeMap;

/// A parsed template: an ordered run of literal text and `{{key}}` placeholders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Literal(String),
    /// `key` is the trimmed lookup identifier; `raw` is the original inner text,
    /// used to reproduce the token verbatim when the key is unknown.
    Placeholder { key: String, raw: String },
}

/// The result of rendering: the output string plus any unrecognised placeholder
/// keys encountered (deduplicated, in first-seen order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    pub unknown: Vec<String>,
}

impl Template {
    /// Parse a template source into literal and placeholder segments.
    ///
    /// A placeholder is `{{` … `}}`; the enclosed text is trimmed to form the
    /// lookup key. An unclosed `{{` is treated as literal text to the end.
    pub fn parse(source: &str) -> Template {
        let mut segments = Vec::new();
        let mut rest = source;
        while let Some(open) = rest.find("{{") {
            if open > 0 {
                segments.push(Segment::Literal(rest[..open].to_string()));
            }
            let after = &rest[open + 2..];
            match after.find("}}") {
                Some(close) => {
                    let raw = &after[..close];
                    segments.push(Segment::Placeholder {
                        key: raw.trim().to_string(),
                        raw: raw.to_string(),
                    });
                    rest = &after[close + 2..];
                }
                None => {
                    // Unclosed: everything from the `{{` onward is literal.
                    segments.push(Segment::Literal(rest[open..].to_string()));
                    rest = "";
                    break;
                }
            }
        }
        if !rest.is_empty() {
            segments.push(Segment::Literal(rest.to_string()));
        }
        Template { segments }
    }

    /// Render against a context map. Recognised keys are substituted; unknown
    /// `{{…}}` tokens are emitted verbatim and collected in [`Rendered::unknown`].
    pub fn render(&self, context: &BTreeMap<String, String>) -> Rendered {
        let mut text = String::new();
        let mut unknown = Vec::new();
        for segment in &self.segments {
            match segment {
                Segment::Literal(s) => text.push_str(s),
                Segment::Placeholder { key, raw } => match context.get(key) {
                    Some(value) => text.push_str(value),
                    None => {
                        text.push_str("{{");
                        text.push_str(raw);
                        text.push_str("}}");
                        if !unknown.iter().any(|u| u == key) {
                            unknown.push(key.clone());
                        }
                    }
                },
            }
        }
        Rendered { text, unknown }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn substitutes_recognised_keys() {
        let t = Template::parse("Hello {{name}}, welcome to {{place}}.");
        let r = t.render(&ctx(&[("name", "alice"), ("place", "the vault")]));
        assert_eq!(r.text, "Hello alice, welcome to the vault.");
        assert!(r.unknown.is_empty());
    }

    #[test]
    fn unknown_token_left_literal_and_reported() {
        let t = Template::parse("a {{known}} b {{missing}} c");
        let r = t.render(&ctx(&[("known", "X")]));
        assert_eq!(r.text, "a X b {{missing}} c");
        assert_eq!(r.unknown, vec!["missing".to_string()]);
    }

    #[test]
    fn dotted_keys_and_whitespace_are_trimmed() {
        let t = Template::parse("{{ files.persona }} / {{scope.agent}}");
        let r = t.render(&ctx(&[("files.persona", "P"), ("scope.agent", "coder")]));
        assert_eq!(r.text, "P / coder");
    }

    #[test]
    fn adjacent_and_repeated_placeholders() {
        let t = Template::parse("{{a}}{{a}}{{b}}");
        let r = t.render(&ctx(&[("a", "1"), ("b", "2")]));
        assert_eq!(r.text, "112");
        // A repeated unknown is reported once.
        let r2 = t.render(&ctx(&[("b", "2")]));
        assert_eq!(r2.text, "{{a}}{{a}}2");
        assert_eq!(r2.unknown, vec!["a".to_string()]);
    }

    #[test]
    fn unclosed_braces_are_literal() {
        let t = Template::parse("text {{unterminated and more");
        let r = t.render(&ctx(&[]));
        assert_eq!(r.text, "text {{unterminated and more");
        assert!(r.unknown.is_empty());
    }

    #[test]
    fn no_placeholders_is_identity() {
        let t = Template::parse("just plain text");
        let r = t.render(&ctx(&[]));
        assert_eq!(r.text, "just plain text");
    }
}
