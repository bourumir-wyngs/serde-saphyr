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

/// Return the simple variant name selected by a resolved local tag identity.
///
/// Global tags and non-specific tags retain their YAML meanings even when a variant has the same
/// name. Source handles must be resolved before calling this function.
pub(crate) fn local_enum_variant_name(tag: &str) -> Option<&str> {
    let candidate = tag.strip_prefix('!')?;
    (simple_enum_variant_name(candidate) == Some(candidate)).then_some(candidate)
}

/// Return the type name of a resolved tag identity that this crate treats as a YAML built-in type.
///
/// Local tags can select typed enum variants and therefore are not core type tags here.
#[cfg(feature = "serialize")]
pub(crate) fn yaml_core_type_tag_name(tag: &str) -> Option<&str> {
    let candidate = if let Some(stripped) = tag.strip_prefix(YAML_TAG_NAMESPACE) {
        stripped
    } else if let Some(stripped) = tag.strip_prefix("!!") {
        stripped
    } else {
        return None;
    };

    if matches!(candidate, "merge" | "value") {
        return Some(candidate);
    }
    let candidate = if tag.starts_with(YAML_TAG_NAMESPACE) {
        candidate.strip_prefix('!').unwrap_or(candidate)
    } else {
        candidate
    };

    matches!(
        candidate,
        "null"
            | "bool"
            | "int"
            | "float"
            | "map"
            | "seq"
            | "str"
            | "timestamp"
            | "binary"
            | "time"
            | "include"
            | "degrees"
            | "radians"
    )
    .then_some(candidate)
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "serialize")]
    use super::{YAML_TAG_NAMESPACE, yaml_core_type_tag_name};
    use super::{local_enum_variant_name, simple_enum_variant_name};

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

    #[rstest::rstest]
    #[case("!timestamp", Some("timestamp"))]
    #[case("!str", Some("str"))]
    #[case("!map", Some("map"))]
    #[case("!Value", Some("Value"))]
    #[case("!", None)]
    #[case("!!str", None)]
    #[case("!bad:name", None)]
    #[case("!bang!oops", None)]
    #[case("tag:yaml.org,2002:str", None)]
    #[case("tag:example.org,2026:timestamp", None)]
    fn identifies_only_resolved_simple_local_enum_tags(
        #[case] tag: &str,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(local_enum_variant_name(tag), expected);
    }

    #[cfg(feature = "serialize")]
    #[test]
    fn identifies_core_type_tags_without_claiming_local_tags() {
        for name in [
            "null",
            "bool",
            "int",
            "float",
            "map",
            "seq",
            "str",
            "timestamp",
            "binary",
            "time",
            "include",
            "degrees",
            "radians",
        ] {
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

        assert_eq!(yaml_core_type_tag_name("!map"), None);
        assert_eq!(yaml_core_type_tag_name("tag:yaml.org,2002:Map"), None);
        assert_eq!(yaml_core_type_tag_name("!Int"), None);
    }

    #[cfg(feature = "serialize")]
    #[rstest::rstest]
    #[case("merge")]
    #[case("value")]
    fn recognizes_yaml_key_tags_without_inventing_aliases(#[case] name: &str) {
        assert_eq!(
            yaml_core_type_tag_name(&format!("{YAML_TAG_NAMESPACE}{name}")),
            Some(name),
        );
        assert_eq!(
            yaml_core_type_tag_name(&format!("{YAML_TAG_NAMESPACE}!{name}")),
            None,
        );
        assert_eq!(yaml_core_type_tag_name(&format!("!{name}")), None);
    }
}
