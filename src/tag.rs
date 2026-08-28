pub(crate) const YAML_TAG_NAMESPACE: &str = "tag:yaml.org,2002:";

/// Return the simple enum variant selected by a resolved or source-spelled YAML tag.
///
/// Verbatim syntax is unwrapped first, the core namespace is removed when present, and every
/// leading `!` is ignored. Names containing namespace or handle separators are not simple enum
/// variants.
pub(crate) fn simple_enum_variant_name(tag: &str) -> Option<&str> {
    let mut candidate =
        if let Some(inner) = tag.strip_prefix("!<").and_then(|s| s.strip_suffix('>')) {
            inner
        } else {
            tag
        };

    if let Some(stripped) = candidate.strip_prefix(YAML_TAG_NAMESPACE) {
        candidate = stripped;
    }

    candidate = candidate.trim_start_matches('!');

    if candidate.is_empty() || candidate.contains([':', '!']) {
        return None;
    }

    Some(candidate)
}

#[cfg(test)]
mod tests {
    use super::simple_enum_variant_name;

    #[test]
    fn normalizes_simple_local_core_and_verbatim_enum_tags() {
        for tag in [
            "!Int",
            "!!Int",
            "tag:yaml.org,2002:Int",
            "tag:yaml.org,2002:!Int",
            "!<tag:yaml.org,2002:!Int>",
        ] {
            assert_eq!(simple_enum_variant_name(tag), Some("Int"), "tag: {tag}");
        }
    }

    #[test]
    fn rejects_non_simple_enum_tags() {
        for tag in ["!", "!!", "!bad:name", "!bang!oops"] {
            assert_eq!(simple_enum_variant_name(tag), None, "tag: {tag}");
        }
    }
}
