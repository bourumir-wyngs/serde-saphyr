pub(crate) const TAG_INT: &str = "!!int";
pub(crate) const TAG_INT_SHORTHAND: &str = "!int";
pub(crate) const TAG_INT_CANONICAL: &str = "tag:yaml.org,2002:int";
pub(crate) const TAG_INT_CANONICAL_ALT: &str = "tag:yaml.org,2002:!int";

pub(crate) const TAG_FLOAT: &str = "!!float";
pub(crate) const TAG_FLOAT_SHORTHAND: &str = "!float";
pub(crate) const TAG_FLOAT_CANONICAL: &str = "tag:yaml.org,2002:float";
pub(crate) const TAG_FLOAT_CANONICAL_ALT: &str = "tag:yaml.org,2002:!float";

pub(crate) const TAG_BOOL: &str = "!!bool";
pub(crate) const TAG_BOOL_SHORTHAND: &str = "!bool";
pub(crate) const TAG_BOOL_CANONICAL: &str = "tag:yaml.org,2002:bool";
pub(crate) const TAG_BOOL_CANONICAL_ALT: &str = "tag:yaml.org,2002:!bool";

pub(crate) const TAG_NULL: &str = "!!null";
pub(crate) const TAG_NULL_SHORTHAND: &str = "!null";
pub(crate) const TAG_NULL_CANONICAL: &str = "tag:yaml.org,2002:null";
pub(crate) const TAG_NULL_CANONICAL_ALT: &str = "tag:yaml.org,2002:!null";

pub(crate) const TAG_SEQ: &str = "!!seq";
pub(crate) const TAG_SEQ_SHORTHAND: &str = "!seq";
pub(crate) const TAG_SEQ_CANONICAL: &str = "tag:yaml.org,2002:seq";
pub(crate) const TAG_SEQ_CANONICAL_ALT: &str = "tag:yaml.org,2002:!seq";

pub(crate) const TAG_MAP: &str = "!!map";
pub(crate) const TAG_MAP_SHORTHAND: &str = "!map";
pub(crate) const TAG_MAP_CANONICAL: &str = "tag:yaml.org,2002:map";
pub(crate) const TAG_MAP_CANONICAL_ALT: &str = "tag:yaml.org,2002:!map";

pub(crate) const TAG_TIMESTAMP: &str = "!!timestamp";
pub(crate) const TAG_TIMESTAMP_SHORTHAND: &str = "!timestamp";
pub(crate) const TAG_TIMESTAMP_CANONICAL: &str = "tag:yaml.org,2002:timestamp";
pub(crate) const TAG_TIMESTAMP_CANONICAL_ALT: &str = "tag:yaml.org,2002:!timestamp";

pub(crate) const NON_STRING_TAGS: &[&str] = &[
    TAG_INT,
    TAG_INT_SHORTHAND,
    TAG_INT_CANONICAL,
    TAG_INT_CANONICAL_ALT,
    TAG_FLOAT,
    TAG_FLOAT_SHORTHAND,
    TAG_FLOAT_CANONICAL,
    TAG_FLOAT_CANONICAL_ALT,
    TAG_BOOL,
    TAG_BOOL_SHORTHAND,
    TAG_BOOL_CANONICAL,
    TAG_BOOL_CANONICAL_ALT,
    TAG_NULL,
    TAG_NULL_SHORTHAND,
    TAG_NULL_CANONICAL,
    TAG_NULL_CANONICAL_ALT,
    TAG_SEQ,
    TAG_SEQ_SHORTHAND,
    TAG_SEQ_CANONICAL,
    TAG_SEQ_CANONICAL_ALT,
    TAG_MAP,
    TAG_MAP_SHORTHAND,
    TAG_MAP_CANONICAL,
    TAG_MAP_CANONICAL_ALT,
    TAG_TIMESTAMP,
    TAG_TIMESTAMP_SHORTHAND,
    TAG_TIMESTAMP_CANONICAL,
    TAG_TIMESTAMP_CANONICAL_ALT,
];

pub(crate) fn can_parse_into_string(tag: Option<&str>) -> bool {
    match tag {
        None => true,
        Some(t) => !NON_STRING_TAGS.contains(&t),
    }
}

pub(crate) fn is_null_tag(tag: Option<&str>) -> bool {
    match tag {
        Some(t) => matches!(
            t,
            TAG_NULL
                | TAG_NULL_SHORTHAND
                | TAG_NULL_CANONICAL
                | TAG_NULL_CANONICAL_ALT
        ),
        None => false,
    }
}
