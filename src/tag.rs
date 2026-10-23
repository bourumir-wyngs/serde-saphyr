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

/// Return the type name of a resolved tag identity that this crate treats as a YAML core type.
///
/// Local core-like spellings such as `!str` or `!map` are excluded because, when deserializing
/// into an enum with a variant of that name, they select the variant.
#[cfg(feature = "serialize")]
pub(crate) fn yaml_core_type_tag_name(tag: &str) -> Option<&str> {
    let candidate = if let Some(stripped) = tag.strip_prefix(YAML_TAG_NAMESPACE) {
        stripped.strip_prefix('!').unwrap_or(stripped)
    } else {
        tag.strip_prefix("!!")?
    };

    matches!(
        candidate,
        "null" | "bool" | "int" | "float" | "map" | "seq" | "str"
    )
    .then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::simple_enum_variant_name;
    #[cfg(feature = "serialize")]
    use super::{YAML_TAG_NAMESPACE, yaml_core_type_tag_name};

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

    #[cfg(feature = "serialize")]
    #[test]
    fn identifies_core_type_tags_without_claiming_local_spellings() {
        for name in ["null", "bool", "int", "float", "map", "seq", "str"] {
            for tag in [
                format!("!!{name}"),
                format!("{YAML_TAG_NAMESPACE}{name}"),
                format!("{YAML_TAG_NAMESPACE}!{name}"),
            ] {
                assert_eq!(yaml_core_type_tag_name(&tag), Some(name), "tag: {tag}");
            }

            let local = format!("!{name}");
            assert_eq!(yaml_core_type_tag_name(&local), None, "tag: {local}");
        }

        assert_eq!(yaml_core_type_tag_name("tag:yaml.org,2002:Map"), None);
        assert_eq!(yaml_core_type_tag_name("!Int"), None);
    }
}
