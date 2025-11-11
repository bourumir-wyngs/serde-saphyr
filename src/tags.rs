//! Tag map. We only care about tags as much as we support them

use saphyr_parser::Tag;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::LazyLock;

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
    // Custom angle tags supported by angles_hook
    Degrees,
    Radians,
    Other,
}

static TAG_LOOKUP_MAP: LazyLock<BTreeMap<&'static str, SfTag>> = LazyLock::new(|| {
    BTreeMap::from([
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

impl SfTag {
    pub(crate) fn from_optional_cow(tag: &Option<Cow<Tag>>) -> SfTag {
        match tag {
            Some(cow) => {
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
            SfTag::None | SfTag::String | SfTag::Other => true,
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
