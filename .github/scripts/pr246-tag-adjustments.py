"""Align remaining regressions with resolved tag identity before validation."""
from pathlib import Path
import json

CHANGED = set(json.loads(Path('/tmp/pr246-changed.json').read_text()))


def replace(path, old, new):
    """Replace one occurrence of old with new in path and record the changed file."""
    p = Path(path)
    source = p.read_text()
    if source.count(old) != 1:
        raise RuntimeError(f'{path}: expected one occurrence: {old[:100]!r}')
    p.write_text(source.replace(old, new))
    CHANGED.add(path)


def append(path, content):
    """Append regression tests to path and record the changed file."""
    with Path(path).open('a') as output:
        output.write(content)
    CHANGED.add(path)


def prepare():
    """Update stale alias tests and add serializer round-trip coverage."""
    replace('tests/binary.rs',
        '#[values("!!binary", "!binary", "!<tag:yaml.org,2002:binary>")] tag: &str,',
        '#[values("!!binary", "!<tag:yaml.org,2002:binary>", "%TAG ! tag:yaml.org,2002:\\n--- !binary")] tag: &str,')
    append('tests/binary.rs', r'''
#[rstest::rstest]
fn local_binary_tag_does_not_decode_null_like_text(
    #[values("null", "Null", "NULL", "")] payload: &str,
) {
    let yaml = format!("!binary {payload}");
    assert!(serde_saphyr::from_str::<ByteBuf>(&yaml).is_err());
    assert_eq!(serde_saphyr::from_str::<Option<ByteBuf>>(&yaml).unwrap(), None);
}

#[rstest::rstest]
#[case("!binary")]
#[case("!<tag:yaml.org,2002:!binary>")]
fn application_binary_names_do_not_request_base64_decoding(#[case] tag: &str) {
    let yaml = format!("{tag} AQID");
    assert!(serde_saphyr::from_str::<ByteBuf>(&yaml).is_err());
    assert_eq!(
        serde_saphyr::from_str::<serde_json::Value>(&yaml).unwrap(),
        serde_json::json!("AQID"),
    );
}
''')
    replace('tests/enum_with_tags_2.rs', '''#[test]
fn local_map_compatibility_tag_remains_a_map_in_untagged_enum() {
    let actual: UntaggedMapping = serde_saphyr::from_str(r#"!map {"value": 10}"#)
        .expect("compatibility-tagged mapping should deserialize");''', '''#[rstest]
#[case(r#"!!map {"value": 10}"#)]
#[case("%TAG ! tag:yaml.org,2002:\\n--- !map {value: 10}")]
fn standard_map_tag_remains_a_map_in_untagged_enum(#[case] yaml: &str) {
    let actual: UntaggedMapping = serde_saphyr::from_str(yaml)
        .expect("standard-tagged mapping should deserialize");''')
    append('tests/enum_with_tags_2.rs', r'''
#[test]
fn local_map_tag_survives_untagged_enum_buffering() {
    let yaml = "!map {value: 10}";
    let actual: UntaggedTaggedMapping = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(
        actual,
        UntaggedTaggedMapping::Tagged(TaggedMapping::Map(BTreeMap::from([
            ("value".into(), 10),
        ]))),
    );
    // The application tag is preserved, not treated as a standard mapping annotation.
    assert!(serde_saphyr::from_str::<UntaggedMapping>(yaml).is_err());
}
''')
    append('tests/local_enum_tag_serialization.rs', r'''
#[rstest]
#[case("tag:yaml.org,2002:!str", Payload::Str(7))]
#[case("tag:yaml.org,2002:!int", Payload::Int("not an integer".into()))]
#[case("tag:yaml.org,2002:!binary", Payload::Binary("not base64!".into()))]
fn nonstandard_yaml_uri_tags_do_not_select_builtin_types(
    #[case] tag: &str,
    #[case] payload: Payload,
) {
    let value = Tagged(payload, Some(tag.to_owned()));
    let yaml = to_string(&value).unwrap();
    assert!(yaml.starts_with(&format!("!<{tag}> ")), "{yaml}");
    assert_eq!(from_str::<Tagged<Payload>>(&yaml).unwrap(), value);
}
''')
    replace('README.md',
        'not standard YAML type annotations, and impose no built-in payload shape. For example, a variant renamed `timestamp` can\n',
        'not standard YAML type annotations, and impose no built-in payload shape.\nFor example, a variant renamed `timestamp` can\n')
    replace('README.md',
        '- Multi-document APIs retain explicitly local-tagged documents, but `Option<E>` still applies the limitation above: the listed scalar forms produce retained `None` entries. Use `E` directly or an externally tagged mapping to select the variant.',
        '- Multi-document APIs retain explicitly local-tagged documents. For `Option<E>`, plain null-like payloads produce retained `None` entries; non-null payloads reach the inner enum. Use `E` directly or an externally tagged mapping to select a unit variant.')
    replace('CHANGELOG.md',
        '  Null-like selectors inside optional enums retain their existing limitations,\n  documented in the README\'s unsupported features section.',
        '  Plain null-like payloads inside optional enums are resolved before the inner\n  variant is known; this limitation is documented in the README. Non-null payloads\n  such as `!null payload` now reach the inner enum rather than becoming `None`.')
    replace('CHANGELOG.md',
        '  null documents are still skipped; optional targets retain their existing null handling.',
        '  null documents are still skipped. Optional targets retain local-tagged entries\n  and resolve plain null-like payloads before selecting inner variants.')
    Path('/tmp/pr246-changed.json').write_text(json.dumps(sorted(CHANGED)))
    print('ADJUSTED', json.dumps(sorted(CHANGED)))


if __name__ == '__main__':
    prepare()
