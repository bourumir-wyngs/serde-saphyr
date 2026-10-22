"""Prepare the tag cleanup on a temporary validation branch; never move PR refs."""
from pathlib import Path
import json
import os
import sys
from urllib.request import Request, urlopen

BASE = "640565d39d6f0765e457161a6fa7429586294a96"
BASE_TREE = "a7843903bfa6bd8b0b2ff9eec46d48af889c2a7f"
REPO = "bourumir-wyngs/serde-saphyr"
CHANGED = set()


def replace(path, old, new, count=1):
    """Replace exactly count occurrences in path, failing before an unexpected edit."""
    p = Path(path)
    text = p.read_text()
    actual = text.count(old)
    if actual != count:
        raise RuntimeError(f"{path}: expected {count} matches, got {actual}: {old[:100]!r}")
    p.write_text(text.replace(old, new))
    CHANGED.add(path)


def write_new(path, content):
    """Write a new source file without overwriting an existing file."""
    p = Path(path)
    if p.exists():
        raise RuntimeError(f"new file already exists: {path}")
    p.write_text(content)
    CHANGED.add(path)


def prepare():
    """Apply source, regression-test, and documentation changes to the checkout."""
    p = "src/de/tags.rs"
    replace(p, "use std::collections::BTreeMap;\nuse std::sync::LazyLock;\n", "")
    replace(p, '    /// Non-specific tag "!" (no resolution) — we force scalar to be treated as string\n',
            '    /// Bare `!`: a string for scalars, or an ordinary sequence or mapping.\n'
            '    /// Distinct from both the standard string tag and application-specific tags.\n')
    text = Path(p).read_text()
    start = text.index("static TAG_LOOKUP_MAP:")
    end = text.index("fn core_suffix_to_sf_tag", start)
    replace(p, text[start:end], "")
    replace(p, '''                TAG_LOOKUP_MAP
                    .get(key.as_str())
                    .copied()
                    .unwrap_or(SfTag::Other)''', '''                match key.as_str() {
                    "!" => SfTag::NonSpecific,
                    "tag:yaml.org,2002:timestamp" => SfTag::TimeStamp,
                    "tag:yaml.org,2002:binary" => SfTag::Binary,
                    // Intentional application extensions, not aliases of standard YAML types.
                    "!time" | "tag:yaml.org,2002:time" | "tag:yaml.org,2002:!time" => SfTag::Time,
                    "!degrees" | "tag:yaml.org,2002:degrees" | "tag:yaml.org,2002:!degrees" => {
                        SfTag::Degrees
                    }
                    "!radians" | "tag:yaml.org,2002:radians" | "tag:yaml.org,2002:!radians" => {
                        SfTag::Radians
                    }
                    _ => SfTag::Other,
                }''')
    replace(p, '&format!("!{name}")', 'format!("!{name}")')
    replace(p, '''    #[test]
    fn maps_resolved_yaml_core_schema_suffixes() {''', '''    #[test]
    fn bare_tag_has_its_own_classification() {
        let tag = sf_tag(Tag::new("!", ""));
        assert_eq!(tag, SfTag::NonSpecific);
        assert_ne!(tag, SfTag::String);
        assert!(tag.forces_string());
        assert_eq!(tag.requires(), None);
        assert_eq!(sf_tag(Tag::new("!", "!")), SfTag::Other);
    }

    #[rstest]
    #[case("str")]
    #[case("int")]
    #[case("timestamp")]
    #[case("binary")]
    fn resolved_local_tags_are_not_standard_shorthands(#[case] name: &str) {
        assert_eq!(sf_tag(Tag::new("!!", name)), SfTag::Other);
    }

    #[test]
    fn maps_resolved_yaml_core_schema_suffixes() {''')

    p = "src/de/key_nodes.rs"
    replace(p, '''            // A local tag can select an enum carrying a string, so neither its
            // tag nor its payload spelling may be collapsed to integer identity.
            let integer_fingerprint = if raw_tag
                .as_deref()
                .and_then(local_enum_variant_name)
                .is_some()
            {
                None
            } else {
                integer_key_fingerprint(&value, tag, style, legacy_octal_numbers)
            };''', '''            let integer_fingerprint =
                integer_key_fingerprint(&value, tag, style, legacy_octal_numbers);''')

    p = "src/de/live_events.rs"
    replace(p, '''/// Local aliases can name enum variants with any payload shape, so their node-kind checks
/// cannot run while events are merely peeked at or buffered before the target type is known.''',
            '''/// Local robotics tags can also select enum variants with collection payloads.
/// Their deferred shape checks run only when the consumer is known.''')
    replace(p, "/// - `tag_kind`: The recognized tag meaning, including supported local aliases.",
            "/// - `tag_kind`: The recognized standard or application-extension tag meaning.")
    replace(p, '''/// Decide whether the recognized `tag_kind` can be a typed enum selector.
/// `raw_tag` is the resolved identity, so remapped global handles keep their YAML constraints.
fn defer_local_tag_kind(tag_kind: SfTag, raw_tag: Option<&str>) -> bool {
    tag_kind.requires().is_some()
        && tag_kind != SfTag::Include''', '''/// Defer robotics shape checks when a resolved local tag may select an enum.
/// Global tags and include processing retain their constraints.
fn defer_local_tag_kind(tag_kind: SfTag, raw_tag: Option<&str>) -> bool {
    tag_kind.is_robotics()''')
    replace(p, '''    /// - `raw_tag`: The resolved tag identity, used to defer local enum selectors.
    /// - `value`: The scalar text, used to validate key-only tags.''',
            '''    /// - `value`: The scalar text, used to validate key-only tags.''')
    replace(p, '''        raw_tag: Option<&str>,
        value: &str,''', '''        value: &str,''')
    replace(p, '''        if !defer_local_tag_kind(tag_kind, raw_tag) {
            validate_tag_kind(tag_kind, TagNodeKind::Scalar, &tag, location)?;
        }''', '''        validate_tag_kind(tag_kind, TagNodeKind::Scalar, &tag, location)?;''')
    replace(p, 'self.validate_scalar_tag(SfTag::Null, String::new, None, "", *location)',
            'self.validate_scalar_tag(SfTag::Null, String::new, "", *location)')
    replace(p, '''                || replayed_tag_spelling(*tag, raw_tag.as_deref()),
                raw_tag.as_deref(),
                value,''', '''                || replayed_tag_spelling(*tag, raw_tag.as_deref()),
                value,''')
    replace(p, '''                        || tag.as_deref().map_or_else(String::new, Tag::original),
                        raw_tag.as_deref(),
                        &val,''', '''                        || tag.as_deref().map_or_else(String::new, Tag::original),
                        &val,''')

    p = "src/de/deserializer.rs"
    replace(p, '''    fn take_scalar_view(&mut self) -> Result<ScalarView<'de>, Error> {
        let event = self.ev.next()?;
        if let Some(event) = &event {
            validate_node_tag_kind(event)?;
        }
        match event {''', '''    fn take_scalar_view(&mut self) -> Result<ScalarView<'de>, Error> {
        match self.ev.next()? {''')
    replace(p, '''            Some(event @ Ev::Scalar { .. }) => {
                validate_node_tag_kind(&event)?;
                Ok(view)
            }''', '''            Some(Ev::Scalar { .. }) => Ok(view),''')
    replace(p, '''                // A matching local tag selects an enum even when its name is also a
                // supported scalar or collection alias outside this typed context.''',
            '''                // The selected variant determines the payload type, including
                // no_schema checks and supported robotics conversions.''')
    replace(p, '''                } else {
                    let Some(event) = self.ev.peek()? else {
                        return Err(eof_with_loc(self.ev));
                    };
                    validate_node_tag_kind(event)?;
                    let Some(view) = self.peek_scalar_view()? else {''',
            '''                } else {
                    let Some(view) = self.peek_scalar_view()? else {''')

    p = "src/tag.rs"
    replace(p, '''    if matches!(candidate, "merge" | "value") {
        return Some(candidate);
    }
    let candidate = if tag.starts_with(YAML_TAG_NAMESPACE) {
        candidate.strip_prefix('!').unwrap_or(candidate)
    } else {
        candidate
    };''', '''    // Only intentional application extensions retain the historical bang-prefixed URI.
    if tag.starts_with(YAML_TAG_NAMESPACE)
        && matches!(candidate, "!time" | "!include" | "!degrees" | "!radians")
    {
        return candidate.strip_prefix('!');
    }''')
    replace(p, '''            | "binary"
            | "time"''', '''            | "binary"
            | "merge"
            | "value"
            | "time"''')
    replace(p, '''                format!("{YAML_TAG_NAMESPACE}{name}"),
                format!("{YAML_TAG_NAMESPACE}!{name}"),''',
            '''                format!("{YAML_TAG_NAMESPACE}{name}"),''')
    replace(p, '''            let local = format!("!{name}");
            assert_eq!(yaml_core_type_tag_name(&local), None, "tag: {local}");''',
            '''            let local = format!("!{name}");
            assert_eq!(yaml_core_type_tag_name(&local), None, "tag: {local}");
            let historical = format!("{YAML_TAG_NAMESPACE}!{name}");
            let expected = matches!(name, "time" | "include" | "degrees" | "radians")
                .then_some(name);
            assert_eq!(yaml_core_type_tag_name(&historical), expected, "tag: {historical}");''')

    p = "tests/local_enum_precedence.rs"
    replace(p, '''fn unmatched_local_string_tag_keeps_its_scalar_meaning() {
    assert_eq!(
        from_str::<Generator>("!str timestamp").unwrap(),
        Generator::Timestamp,
    );
}''', '''fn unmatched_local_string_tag_does_not_force_a_string() {
    assert!(from_str::<Generator>("!str timestamp").is_err());
}''')
    replace(p, '''        from_str::<Option<Transparent>>("!timestamp").unwrap(),
        Some(Transparent {
            inner: Newtype(Generator::Timestamp),
        }),''', '''        from_str::<Transparent>("!timestamp").unwrap(),
        Transparent {
            inner: Newtype(Generator::Timestamp),
        },''')

    p = "tests/local_enum_tag_shapes.rs"
    replace(p, "/// Returns normally when all payload shapes deserialize in both strict modes.",
            "/// Returns normally when all payload shapes deserialize in permissive mode.")
    text = Path(p).read_text()
    start = text.index("    for reject_unsupported_tags in [false, true] {")
    end = text.index("\n}\n", start)
    body = text[start:end]
    body = body.replace("    for reject_unsupported_tags in [false, true] {\n", "", 1)
    body = body.rsplit("\n    }", 1)[0]
    body = "\n".join(line[4:] if line.startswith("    ") else line for line in body.split("\n"))
    body = body.replace("let opts = options! { reject_unsupported_tags: reject_unsupported_tags };", "let opts = options! { reject_unsupported_tags: false };")
    replace(p, text[start:end], body)
    text = Path(p).read_text()
    start = text.index("fn non_enum_consumers_still_validate_local_tag_shapes(")
    end = text.index("\n#[rstest]\n#[case(DuplicateKeyPolicy", start)
    replace(p, text[start:end], '''fn ordinary_consumers_ignore_local_type_names(#[case] yaml: &str) {
    assert!(from_str::<IgnoredAny>(yaml).is_ok());
    assert!(from_str::<serde_json::Value>(yaml).is_ok());

    #[derive(Debug, Deserialize)]
    struct KnownOnly {
        #[allow(dead_code)]
        known: u8,
    }

    assert!(from_str::<KnownOnly>(&format!("known: 1\\nignored: {yaml}\\n")).is_ok());
}

#[rstest]
fn strict_mode_rejects_local_type_names(
    #[values("int", "float", "bool", "null", "seq", "map", "str", "timestamp", "binary")]
    tag: &str,
    #[values("", "text", "[1, 2]", "{value: 1}")] payload: &str,
) {
    let yaml = format!("!{tag} {payload}");
    let opts = options! { reject_unsupported_tags: true };
    let ignored = from_str_with_options::<IgnoredAny>(&yaml, opts.clone()).unwrap_err();
    let typed = from_str_with_options::<Unit>(&yaml, opts).unwrap_err();
    for error in [ignored, typed] {
        assert!(matches!(error.without_snippet(), Error::UnsupportedTag { .. }));
    }
}
''')
    replace(p, '''    // A discarded value has no target type. It may be a valid enum payload, so local
    // tag shape checks must wait until a value is actually deserialized.''',
            '''    // Ordinary local tags do not impose a built-in node shape, even while buffering.''')
    with Path(p).open("a") as out:
        out.write('''
#[rstest]
#[case("!time [1, 2]")]
#[case("!degrees {value: 1}")]
#[case("!radians []")]
fn ordinary_consumers_validate_robotics_collection_shapes(#[case] yaml: &str) {
    let error = from_str::<serde_json::Value>(yaml).unwrap_err();
    assert!(matches!(error.without_snippet(), Error::UnsupportedTag { .. }));
}

#[rstest]
#[case(DuplicateKeyPolicy::FirstWins, vec![1, 2])]
#[case(DuplicateKeyPolicy::LastWins, vec![3, 4])]
fn discarded_robotics_selectors_defer_shape_checks(
    #[case] duplicate_keys: DuplicateKeyPolicy,
    #[case] expected: Vec<u8>,
) {
    #[derive(Debug, Deserialize, PartialEq)]
    enum Extension {
        #[serde(rename = "time")]
        Time(Vec<u8>),
    }
    let value: BTreeMap<String, Extension> = from_str_with_options(
        "value: !time [1, 2]\\nvalue: !time [3, 4]\\n",
        options! { duplicate_keys: duplicate_keys },
    ).unwrap();
    assert_eq!(value, BTreeMap::from([("value".into(), Extension::Time(expected))]));
}
''')

    p = "tests/local_enum_keys.rs"
    replace(p, '''fn merge_containers_still_validate_tag_shapes(#[case] yaml: &str) {
    let error = serde_saphyr::from_str::<BTreeMap<String, u8>>(yaml).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::UnsupportedTag { .. }
    ));
}''', '''fn merge_containers_ignore_application_tag_names(#[case] yaml: &str) {
    let actual: BTreeMap<String, u8> = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(actual, BTreeMap::from([("x".into(), 1)]));
}

#[rstest]
#[case("<<: !time {x: 1}")]
#[case("<<: !degrees [{x: 1}]")]
#[case("<<: [!radians {x: 1}]")]
#[case("<<: {<<: !time {x: 1}}")]
#[case("<<: !!int {x: 1}")]
fn merge_containers_validate_standard_and_robotics_shapes(#[case] yaml: &str) {
    let error = serde_saphyr::from_str::<BTreeMap<String, u8>>(yaml).unwrap_err();
    assert!(matches!(error.without_snippet(), Error::UnsupportedTag { .. }));
}''')

    p = "tests/option_tag_compatibility.rs"
    replace(p, '''#[case("!null payload", Choice::Null("payload".into()))]
#[case("!null 'null'", Choice::Null("null".into()))]
''', "")
    replace(p, '#[case("!<!now>", Choice::Now)]',
            '#[case("!<!now>", Choice::Now)]\n#[case("!timestamp", Choice::Timestamp)]')
    replace(p, '#[case("!null 42")]', '#[case("!null null")]')
    replace(p, 'fn ordinary_optional_types_keep_null_alias_compatibility(',
            'fn ordinary_optional_types_resolve_null_payloads(')
    replace(p, '#[case("!timestamp", Choice::Timestamp)]\nfn optional_enums_support_mappings_and_non_null_tags',
            '#[case("!null payload", Choice::Null("payload".into()))]\n#[case("!null \'null\'", Choice::Null("null".into()))]\nfn optional_enums_support_mappings_and_non_null_payloads')
    replace(p, '#[cfg(feature = "serialize")]\n#[rstest]', '''#[test]
fn local_null_name_does_not_discard_non_null_payloads() {
    assert_eq!(from_str::<Option<u8>>("!null 42").unwrap(), Some(42));
    assert_eq!(from_str::<Option<String>>("!null payload").unwrap(), Some("payload".into()));
}

#[cfg(feature = "serialize")]
#[rstest]''')

    p = "tests/local_tagged_documents.rs"
    replace(p, "documents::<Option<Command>>(OPTIONAL_DOCUMENTS)",
            "documents::<Option<Payload>>(OPTIONAL_DOCUMENTS)", 3)
    replace(p, "assert_eq!(result.unwrap(), [None, None, None]);",
            'assert_eq!(result.unwrap(), [None, None, Some(Payload::Null("payload".into()))]);')
    replace(p, "[Validated(None), Validated(None), Validated(None)]",
            '[Validated(None), Validated(None), Validated(Some(Payload::Null("payload".into())))]', 2)

    write_new("tests/non_specific_tag_identity.rs", r'''#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_json::{Value, json};
use serde_saphyr::{Tagged, from_str, from_str_with_options, options};

/// `yaml` is the input, `expected` its generic value, and `strict` selects tag rejection.
/// Returns normally when the bare tag retains its standard non-specific meaning.
#[rstest]
#[case("! 123", json!("123"))]
#[case("! true", json!("true"))]
#[case("! null", json!("null"))]
#[case("! ~", json!("~"))]
#[case("!", json!(""))]
#[case("! [123, true, ! 123]", json!([123, true, "123"]))]
#[case("! {number: 123, text: ! 123}", json!({"number": 123, "text": "123"}))]
#[case("%TAG ! tag:example.org,2026:\n--- ! 123", json!("123"))]
#[case("%TAG ! tag:yaml.org,2002:\n--- ! 123", json!("123"))]
fn bare_tag_resolves_by_node_kind(
    #[case] yaml: &str,
    #[case] expected: Value,
    #[values(false, true)] strict: bool,
) {
    let value: Value = from_str_with_options(yaml, options! { reject_unsupported_tags: strict })
        .unwrap();
    assert_eq!(value, expected);
}

#[rstest]
#[case("!str 123", json!(123))]
#[case("!!str 123", json!("123"))]
#[case("! 123", json!("123"))]
#[case("!int '123'", json!("123"))]
#[case("!!int '123'", json!(123))]
#[case("!null 123", json!(123))]
#[case("!<tag:yaml.org,2002:!str> 123", json!(123))]
#[case("%TAG ! tag:yaml.org,2002:\n--- !str 123", json!("123"))]
#[case("%TAG !! !\n--- !!str 123", json!(123))]
fn local_standard_and_non_specific_tags_are_distinct(#[case] yaml: &str, #[case] expected: Value) {
    assert_eq!(from_str::<Value>(yaml).unwrap(), expected);
}

#[rstest]
#[case("!", "")]
#[case("! null", "null")]
#[case("! ~", "~")]
fn bare_tag_preserves_null_like_text(#[case] yaml: &str, #[case] expected: &str) {
    assert_eq!(from_str::<Option<String>>(yaml).unwrap().as_deref(), Some(expected));
    let values: Vec<String> = serde_saphyr::from_multiple(yaml).unwrap();
    assert_eq!(values, [expected]);
}

#[test]
fn bare_tag_is_preserved_but_never_selects_a_named_variant() {
    #[derive(Debug, Deserialize)]
    enum Choice {
        #[serde(rename = "!")]
        Bang(#[allow(dead_code)] u8),
    }
    assert!(from_str::<Choice>("! 12").is_err());
    let value: Tagged<Value> = from_str("! 12").unwrap();
    assert_eq!(value, Tagged(json!("12"), Some("!".into())));
}

#[test]
fn secondary_handle_requires_a_suffix() {
    assert!(from_str::<Value>("!! 12").is_err());
}
''')

    p = "README.md"
    replace(p, '''Discarded duplicate values have no requested Rust type. Their local tags may have
been enum selectors, so deferred local tag shape checks do not run on those values.''',
            '''Discarded duplicate values have no requested Rust type. Their robotics extension
tags may have been enum selectors, so deferred robotics shape checks do not run on them.''')
    replace(p, '''`!str`, `!int`, `!null`, `!seq`, and `!map`, even when the payload has a different shape
from the ordinary meaning of that tag.''',
            '''`!str`, `!int`, `!null`, `!seq`, and `!map`. These are application tags,
not standard YAML type annotations, and impose no built-in payload shape.''')
    replace(p, '''can change that identity. Global type tags retain their normal meaning. Outside a
matching enum context, existing local aliases retain their usual type semantics.
Serde's generic buffering, including untagged enums, does not provide a variant list
for this precedence rule. Include expansion and strict tag eligibility still apply.''',
            '''can change that identity. Only resolved standard tags, such as `!!str` and `!!int`,
apply the corresponding YAML type semantics. Local `!str`, `!int`, `!bool`, `!float`,
`!null`, `!seq`, `!map`, `!timestamp`, and `!binary` are ordinary application tags;
outside a matching enum context they follow the unsupported-tag policy below.

The standalone `!` is a separate non-specific tag, not an enum selector. It resolves
scalars as strings and collections as ordinary sequences or mappings, without changing
how their children resolve. For example, `! [12, ! 12]` contains a number and a string.
This behavior is unchanged by `%TAG` directives; bare `!!` is not another spelling of it.

Migration: replace former local type aliases with standard tags (`!int` becomes `!!int`,
`!binary` becomes `!!binary`, and so on); quote strings or use `!!str`. Tags such as
`!<tag:yaml.org,2002:!str>` are also distinct from standard type tags. Intentional
application extensions (`!time`, `!degrees`, `!radians`, and `!include`) retain their
existing meanings. Matching local robotics tags can still select typed enum variants.
Serde's generic buffering, including untagged enums, does not provide a variant list
for that precedence rule. Include expansion and strict tag eligibility still apply.''')
    replace(p, '''other scalar is rejected in strict mode. Except when a matching local tag selects an enum
variant, known scalar, sequence, and mapping tags are accepted only on matching node kinds,
even when `reject_unsupported_tags` is false. Robotics-only''',
            '''other scalar is rejected in strict mode. Standard scalar, sequence, and mapping tags
are accepted only on matching node kinds, even when `reject_unsupported_tags` is false.
Local robotics tag shape checks are deferred until a non-enum consumer is known. Robotics-only''')
    replace(p, '''Scalar `!null` values, including `!null payload`, and custom tags with plain null-like content, such as `!now` or `!now null`, become `None` rather than selecting a variant.''',
            '''Application tags with plain null-like content, such as `!null`, `!now`, `!timestamp`, or `!now null`, become `None` rather than selecting a variant. A non-null payload such as `!null payload` reaches the inner enum; the local name `null` does not force null.''')
    replace(p, '''Existing null handling and serialization of locally tagged `None` values remain supported for compatibility.''',
            '''Serialization of locally tagged `None` values remains supported.''')

    p = "src/de/options.rs"
    replace(p, '''    /// only on matching node kinds, unless a matching local tag selects a typed enum
    /// variant. These shape checks are independent of this option; discarded values
    /// have no target type and do not undergo deferred local tag shape checks.''',
            '''    /// only on matching node kinds. These shape checks are independent of this option.
    /// Local robotics tags may instead select typed enum variants; their deferred
    /// collection-shape checks do not run on discarded values, which have no target type.''')

    p = "CHANGELOG.md"
    text = Path(p).read_text()
    marker = "### Fixed\n\n"
    pos = text.index(marker) + len(marker)
    note = ("- Distinguish local application tags from standard YAML types. Without a `%TAG`\n"
            "  remapping, `!str`, `!int`, `!bool`, `!float`, `!null`, `!seq`, `!map`, `!timestamp`,\n"
            "  and `!binary` no longer force their similarly named standard types. Use `!!str`,\n"
            "  `!!int`, and the other standard tags for type annotations. Strict unsupported-tag\n"
            "  rejection now treats those local names like other application tags.\n"
            "- Keep bare `!` as the separate non-specific tag: scalars resolve as strings and\n"
            "  collections keep their ordinary kinds. Remove the false bare `!!` alias and\n"
            "  redundant local-type shape and integer-key workarounds.\n")
    Path(p).write_text(text[:pos] + note + text[pos:])
    CHANGED.add(p)
    Path("/tmp/pr246-changed.json").write_text(json.dumps(sorted(CHANGED)))
    print("CHANGED", json.dumps(sorted(CHANGED)))


def api(path, payload):
    """Create a Git object in the fixed repository and return its JSON response."""
    request = Request(
        f"https://api.github.com/repos/{REPO}/{path}",
        data=json.dumps(payload).encode(),
        headers={"Authorization": f"Bearer {os.environ['GH_TOKEN']}",
                 "Accept": "application/vnd.github+json",
                 "Content-Type": "application/json"},
        method="POST",
    )
    with urlopen(request, timeout=60) as response:
        return json.load(response)


def export_tree():
    """Upload changed source blobs and a tree without creating commits or updating refs."""
    paths = json.loads(Path("/tmp/pr246-changed.json").read_text())
    entries = []
    for path in paths:
        if not path.startswith(("src/", "tests/")) and path not in ("README.md", "CHANGELOG.md"):
            raise RuntimeError(f"unexpected export path: {path}")
        blob = api("git/blobs", {"content": Path(path).read_text(), "encoding": "utf-8"})
        entries.append({"path": path, "mode": "100644", "type": "blob", "sha": blob["sha"]})
    tree = api("git/trees", {"base_tree": BASE_TREE, "tree": entries})
    print("PREPARED_TREE", tree["sha"], flush=True)
    Path("/tmp/pr246-tree.json").write_text(json.dumps({"base": BASE, "tree": tree["sha"], "paths": paths}))


if __name__ == "__main__":
    if sys.argv[1:] == ["export"]:
        export_tree()
    elif not sys.argv[1:]:
        prepare()
    else:
        raise SystemExit("expected no arguments or 'export'")
