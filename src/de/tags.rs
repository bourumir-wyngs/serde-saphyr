//! Tag map. We only care about tags as much as we support them

use granit_parser::Tag;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::LazyLock;

const INCLUDE_TAG_PLAIN: [&str; 3] = [
    "!include",
    "tag:yaml.org,2002:include",
    "tag:yaml.org,2002:!include",
];

const INCLUDE_TAG_WITH_FRAGMENT_PREFIX: [&str; 3] = [
    "!include#",
    "tag:yaml.org,2002:include#",
    "tag:yaml.org,2002:!include#",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IncludeTag {
    NotInclude,
    Plain,
    WithFragment(String),
    InvalidFragment,
}

pub(crate) fn parse_include_tag(tag: &Option<Cow<Tag>>) -> IncludeTag {
    let Some(tag) = tag else {
        return IncludeTag::NotInclude;
    };

    let raw = tag.to_string();
    if INCLUDE_TAG_PLAIN.contains(&raw.as_str()) {
        return IncludeTag::Plain;
    }

    for prefix in INCLUDE_TAG_WITH_FRAGMENT_PREFIX {
        if let Some(fragment) = raw.strip_prefix(prefix) {
            if fragment.is_empty() {
                return IncludeTag::InvalidFragment;
            }
            return IncludeTag::WithFragment(fragment.to_string());
        }
    }

    IncludeTag::NotInclude
}

#[cfg(feature = "include")]
pub(crate) fn include_spec_from_tag_and_value(
    tag: &Option<Cow<Tag>>,
    value: &str,
) -> Result<Option<String>, &'static str> {
    match parse_include_tag(tag) {
        IncludeTag::NotInclude => Ok(None),
        IncludeTag::Plain => Ok(Some(value.to_string())),
        IncludeTag::WithFragment(fragment) => {
            if value.contains('#') {
                return Err(
                    "include spec must not contain '#' when using !include#fragment tag form",
                );
            }
            Ok(Some(format!("{value}#{fragment}")))
        }
        IncludeTag::InvalidFragment => {
            Err("!include tag fragment must not be empty (expected !include#anchor_name)")
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SfTag {
    None,
    Int,
    Float,
    Bool,
    Null,
    Seq,
    Map,
    TimeStamp,
    Binary,
    String,
    /// Non-specific tag "!" (no resolution) — we force scalar to be treated as string
    NonSpecific,
    /// !include tag - include external resource
    Include,
    // Custom angle tags supported by angles_hook
    Degrees,
    Radians,
    Other,
}

static TAG_LOOKUP_MAP: LazyLock<BTreeMap<&'static str, SfTag>> = LazyLock::new(|| {
    BTreeMap::from([
        // Core-like spellings kept for compatibility. Resolved YAML 1.2.2 Core Schema tags are
        // handled first via `Tag::core_suffix()`, which is independent of source handle spelling.
        // int
        ("!!int", SfTag::Int),
        ("!int", SfTag::Int),
        ("tag:yaml.org,2002:int", SfTag::Int),
        ("tag:yaml.org,2002:!int", SfTag::Int),
        // float
        ("!!float", SfTag::Float),
        ("!float", SfTag::Float),
        ("tag:yaml.org,2002:float", SfTag::Float),
        ("tag:yaml.org,2002:!float", SfTag::Float),
        // bool
        ("!!bool", SfTag::Bool),
        ("!bool", SfTag::Bool),
        ("tag:yaml.org,2002:bool", SfTag::Bool),
        ("tag:yaml.org,2002:!bool", SfTag::Bool),
        // null
        ("!!null", SfTag::Null),
        ("!null", SfTag::Null),
        ("tag:yaml.org,2002:null", SfTag::Null),
        ("tag:yaml.org,2002:!null", SfTag::Null),
        // seq
        ("!!seq", SfTag::Seq),
        ("!seq", SfTag::Seq),
        ("tag:yaml.org,2002:seq", SfTag::Seq),
        ("tag:yaml.org,2002:!seq", SfTag::Seq),
        // map
        ("!!map", SfTag::Map),
        ("!map", SfTag::Map),
        ("tag:yaml.org,2002:map", SfTag::Map),
        ("tag:yaml.org,2002:!map", SfTag::Map),
        // string (null key or value with this tag can be serialized into empty string)
        ("!!str", SfTag::String),
        ("!str", SfTag::String),
        ("tag:yaml.org,2002:str", SfTag::String),
        ("tag:yaml.org,2002:!str", SfTag::String),
        // timestamp / time
        ("!!timestamp", SfTag::TimeStamp),
        ("!timestamp", SfTag::TimeStamp),
        ("tag:yaml.org,2002:timestamp", SfTag::TimeStamp),
        ("tag:yaml.org,2002:!timestamp", SfTag::TimeStamp),
        // additional time aliases (custom)
        ("!time", SfTag::TimeStamp),
        ("tag:yaml.org,2002:time", SfTag::TimeStamp),
        ("tag:yaml.org,2002:!time", SfTag::TimeStamp),
        // binary
        ("!!binary", SfTag::Binary),
        ("!binary", SfTag::Binary),
        ("tag:yaml.org,2002:binary", SfTag::Binary),
        ("tag:yaml.org,2002:!binary", SfTag::Binary),
        // include
        ("!include", SfTag::Include),
        ("tag:yaml.org,2002:include", SfTag::Include),
        ("tag:yaml.org,2002:!include", SfTag::Include),
        // angles (custom)
        ("!degrees", SfTag::Degrees),
        ("tag:yaml.org,2002:degrees", SfTag::Degrees),
        ("tag:yaml.org,2002:!degrees", SfTag::Degrees),
        ("!radians", SfTag::Radians),
        ("tag:yaml.org,2002:radians", SfTag::Radians),
        ("tag:yaml.org,2002:!radians", SfTag::Radians),
        // non-specific ("!", "!!"), should force into string.
        ("!", SfTag::NonSpecific),
        ("!!", SfTag::NonSpecific),
    ])
});

fn core_suffix_to_sf_tag(suffix: &str) -> Option<SfTag> {
    match suffix {
        "null" => Some(SfTag::Null),
        "bool" => Some(SfTag::Bool),
        "int" => Some(SfTag::Int),
        "float" => Some(SfTag::Float),
        "map" => Some(SfTag::Map),
        "seq" => Some(SfTag::Seq),
        "str" => Some(SfTag::String),
        _ => None,
    }
}

impl SfTag {
    pub(crate) fn from_optional_cow(tag: &Option<Cow<Tag>>) -> SfTag {
        match parse_include_tag(tag) {
            IncludeTag::Plain | IncludeTag::WithFragment(_) | IncludeTag::InvalidFragment => {
                return SfTag::Include;
            }
            IncludeTag::NotInclude => {}
        }

        match tag {
            Some(cow) => {
                if let Some(core_tag) = cow.core_suffix().and_then(core_suffix_to_sf_tag) {
                    return core_tag;
                }

                let key = cow.to_string();
                TAG_LOOKUP_MAP
                    .get(key.as_str())
                    .copied()
                    .unwrap_or(SfTag::Other)
            }
            None => SfTag::None,
        }
    }

    pub(crate) fn can_parse_into_string(&self) -> bool {
        match self {
            SfTag::None | SfTag::String | SfTag::Other | SfTag::Include => true,
            SfTag::Binary
            | SfTag::Int
            | SfTag::Float
            | SfTag::Bool
            | SfTag::Null
            | SfTag::Seq
            | SfTag::Map
            | SfTag::TimeStamp
            | SfTag::Degrees
            | SfTag::Radians
            | SfTag::NonSpecific => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SfTag, core_suffix_to_sf_tag};
    use granit_parser::Tag;
    use std::borrow::Cow;

    fn sf_tag(tag: Tag) -> SfTag {
        SfTag::from_optional_cow(&Some(Cow::Owned(tag)))
    }

    #[test]
    fn maps_resolved_yaml_core_schema_suffixes() {
        for (suffix, expected) in [
            ("null", SfTag::Null),
            ("bool", SfTag::Bool),
            ("int", SfTag::Int),
            ("float", SfTag::Float),
            ("map", SfTag::Map),
            ("seq", SfTag::Seq),
            ("str", SfTag::String),
        ] {
            assert_eq!(core_suffix_to_sf_tag(suffix), Some(expected));
            assert_eq!(
                sf_tag(Tag::with_original_handle(
                    "tag:yaml.org,2002:",
                    suffix,
                    "!!"
                )),
                expected
            );
        }
    }

    #[test]
    fn maps_resolved_core_tag_split_by_tag_directive() {
        let tag = Tag::with_original_handle("tag:yaml.org,2002:i", "nt", "!core!");

        assert_eq!(sf_tag(tag), SfTag::Int);
    }

    #[test]
    fn keeps_non_core_yaml_tags_on_fallback_path() {
        let timestamp = Tag::with_original_handle("tag:yaml.org,2002:", "timestamp", "!!");
        let binary = Tag::with_original_handle("tag:yaml.org,2002:", "binary", "!!");
        let unknown = Tag::with_original_handle("tag:yaml.org,2002:", "application", "!!");

        assert_eq!(sf_tag(timestamp), SfTag::TimeStamp);
        assert_eq!(sf_tag(binary), SfTag::Binary);
        assert_eq!(sf_tag(unknown), SfTag::Other);
    }
}
