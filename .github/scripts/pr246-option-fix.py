"""Prepare and validate the optional tagged-value fix without updating repository refs."""
from pathlib import Path
import json
import os
import sys
from urllib.request import Request, urlopen

REPO = "bourumir-wyngs/serde-saphyr"
BASE_TREE = "24eff1f1ac1739d135a91fd123f2badf59597c7f"
CHANGED = set()


def replace(path, old, new, count=1):
    """Replace old with new in path, requiring exactly count matches; return nothing."""
    p = Path(path)
    source = p.read_text()
    if source.count(old) != count:
        raise RuntimeError(f"{path}: expected {count} matches, got {source.count(old)}: {old[:120]!r}")
    p.write_text(source.replace(old, new))
    CHANGED.add(path)


def write(path, content):
    """Replace the contents of a known source file and record it for export."""
    if not Path(path).is_file():
        raise RuntimeError(f"expected existing source file: {path}")
    Path(path).write_text(content)
    CHANGED.add(path)


def prepare():
    """Apply the targeted source fix, regression coverage, and migration documentation."""
    p = "src/de/deserializer.rs"
    replace(p, '''    /// tagged null, or a plain empty / `~` / `null` scalar permitting implicit null resolution.
    /// Explicit non-null core, binary, timestamp, and robotics tags are passed to the inner
    /// value's deserializer.''', '''    /// tagged standard null, or an untagged plain empty / `~` / `null` scalar.
    /// Every other explicit tag denotes a present value and reaches the inner
    /// deserializer unchanged, even when its payload is empty or null-like.''')
    source = Path(p).read_text()
    start = source.index("    fn deserialize_option<V:")
    end = source.index("    /// Deserialize the unit type", start)
    old = source[start:end]
    new = old.replace(
        '''        if let Some((value, tag, style, _location)) = self.peek_effective_scalar()?
            && scalar_is_null(&tag, &value, &style)''',
        '''        // An application tag denotes a present value, not an absent payload.
        // The inner deserializer interprets the intact node; an enum removes its
        // selector before deserializing a payload that may itself be optional.
        if let Some((value, tag, style, _location)) = self.peek_effective_scalar()?
            && matches!(tag, SfTag::None | SfTag::Null)
            && scalar_is_null(&tag, &value, &style)''',
    )
    if old == new:
        raise RuntimeError("option null check not found")
    replace(p, old, new)

    p = "src/de/api.rs"
    replace(p, "use crate::parse_scalars::scalar_is_null;\n", "use crate::parse_scalars::scalar_is_null;\nuse crate::tags::SfTag;\n")
    replace(p, '''/// Specific local tags must reach the target type: even an empty scalar can select an enum
/// variant. `raw_tag` already contains the resolved identity, including directive expansion.''',
            '''/// Only implicit nulls and resolved standard null tags may be omitted. Every other
/// explicit tag must reach the target type, even when its scalar payload is empty.''')
    replace(p, '''        tag,
        raw_tag,
        ..
    } = event''', '''        tag,
        ..
    } = event''')
    replace(p, '''    !raw_tag
        .as_deref()
        .is_some_and(|tag| tag.starts_with('!') && tag != "!")
        && scalar_is_null(tag, value, style)''',
            '''    matches!(tag, SfTag::None | SfTag::Null) && scalar_is_null(tag, value, style)''')

    write("tests/option_tag_compatibility.rs", r'''#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{Error, NullableTilde, Tagged, from_str, from_str_with_options, options};

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[serde(rename_all = "lowercase")]
enum Choice {
    Null(String),
    Now,
    Timestamp,
    Count(u32),
    Maybe(Option<u32>),
    List(Vec<u8>),
}

/// `yaml` supplies the tagged node, `expected` its enum value, and `no_schema` the mode.
/// Returns normally when outer options preserve both the selector and payload semantics.
#[rstest]
#[case("!now", Choice::Now)]
#[case("!now null", Choice::Now)]
#[case("!now Null", Choice::Now)]
#[case("!now NULL", Choice::Now)]
#[case("!now ~", Choice::Now)]
#[case("!timestamp", Choice::Timestamp)]
#[case("!<!timestamp>", Choice::Timestamp)]
#[case("%TAG !local! !\n--- !local!timestamp", Choice::Timestamp)]
#[case("%TAG !! !\n--- !!timestamp", Choice::Timestamp)]
#[case("!null payload", Choice::Null("payload".into()))]
#[case("!null 'null'", Choice::Null("null".into()))]
#[case("!count 42", Choice::Count(42))]
#[case("!maybe", Choice::Maybe(None))]
#[case("!maybe null", Choice::Maybe(None))]
#[case("!maybe ~", Choice::Maybe(None))]
#[case("!maybe 42", Choice::Maybe(Some(42)))]
#[case("!list [1, 2]", Choice::List(vec![1, 2]))]
#[case("{now: null}", Choice::Now)]
#[case("{maybe: null}", Choice::Maybe(None))]
fn optional_values_preserve_enum_selectors(
    #[case] yaml: &str,
    #[case] expected: Choice,
    #[values(false, true)] no_schema: bool,
) {
    let opts = options! { no_schema: no_schema };
    assert_eq!(from_str_with_options::<Choice>(yaml, opts.clone()).unwrap(), expected);
    assert_eq!(
        from_str_with_options::<Option<Choice>>(yaml, opts.clone()).unwrap(),
        Some(expected),
    );
    let direct = from_str_with_options::<Choice>(yaml, opts.clone()).unwrap();
    assert_eq!(
        from_str_with_options::<Option<Option<Choice>>>(yaml, opts).unwrap(),
        Some(Some(direct)),
    );
}

#[rstest]
#[case("")]
#[case("---")]
#[case("null")]
#[case("Null")]
#[case("NULL")]
#[case("~")]
#[case("!!null")]
#[case("!!null null")]
#[case("!!null 'null'")]
#[case("!<tag:yaml.org,2002:null> ~")]
#[case("%TAG ! tag:yaml.org,2002:\n--- !null")]
#[case("%TAG !n! tag:yaml.org,2002:nu\n--- !n!ll")]
fn actual_null_nodes_remain_none(#[case] yaml: &str) {
    assert_eq!(from_str::<Option<Choice>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<String>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<u8>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<bool>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<Vec<u8>>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<()>>(yaml).unwrap(), None);
}

#[rstest]
#[case("!unknown")]
#[case("!unknown null")]
#[case("!count")]
#[case("!count null")]
#[case("!count ~")]
#[case("!count nope")]
#[case("!null")]
#[case("!null null")]
#[case("!!int")]
#[case("!!int null")]
#[case("!!str")]
fn optional_enums_preserve_inner_errors(#[case] yaml: &str) {
    let direct = from_str::<Choice>(yaml).unwrap_err();
    let optional = from_str::<Option<Choice>>(yaml).unwrap_err();
    assert_eq!(optional.without_snippet().to_string(), direct.without_snippet().to_string());
}

#[rstest]
#[case("!null")]
#[case("!null null")]
#[case("!foo ~")]
#[case("!<tag:example.org,2026:unit> null")]
fn tagged_presence_does_not_depend_on_an_enum_target(#[case] yaml: &str) {
    assert_eq!(from_str::<()>(yaml).unwrap(), ());
    assert_eq!(from_str::<Option<()>>(yaml).unwrap(), Some(()));
    assert!(from_str::<u8>(yaml).is_err());
    assert!(from_str::<Option<u8>>(yaml).is_err());
    assert!(from_str::<String>(yaml).is_err());
    assert!(from_str::<Option<String>>(yaml).is_err());
}

#[test]
fn nested_fields_and_aliases_preserve_tagged_values() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Config {
        absent: Option<Choice>,
        empty: Option<Choice>,
        value: Option<Choice>,
        payload: Option<Choice>,
    }
    let value: Config = from_str("empty:\nvalue: !timestamp\npayload: !maybe\n").unwrap();
    assert_eq!(value, Config {
        absent: None,
        empty: None,
        value: Some(Choice::Timestamp),
        payload: Some(Choice::Maybe(None)),
    });
    assert_eq!(
        from_str::<Vec<Option<Choice>>>("[&value !timestamp, *value]").unwrap(),
        [Some(Choice::Timestamp), Some(Choice::Timestamp)],
    );
}

#[test]
fn optional_enum_keys_preserve_selectors() {
    use std::collections::BTreeMap;
    let value: BTreeMap<Option<Choice>, u8> = from_str("!now : 1\n!maybe : 2\n").unwrap();
    assert_eq!(value, BTreeMap::from([
        (Some(Choice::Now), 1),
        (Some(Choice::Maybe(None)), 2),
    ]));
}

#[rstest]
#[case("!timestamp")]
#[case("!timestamp null")]
fn wrappers_preserve_present_enum_values(#[case] yaml: &str) {
    assert_eq!(
        from_str::<NullableTilde<Choice>>(yaml).unwrap(),
        NullableTilde(Some(Choice::Timestamp)),
    );
    assert_eq!(
        from_str::<Tagged<Option<Choice>>>(yaml).unwrap(),
        Tagged(Some(Choice::Timestamp), Some("!timestamp".into())),
    );
    assert_eq!(
        from_str::<Option<Tagged<Choice>>>(yaml).unwrap(),
        Some(Tagged(Choice::Timestamp, Some("!timestamp".into()))),
    );
}

#[test]
fn strict_tag_rejection_is_unchanged() {
    let opts = options! { reject_unsupported_tags: true };
    let error = from_str_with_options::<Option<Choice>>("!timestamp", opts).unwrap_err();
    assert!(matches!(error.without_snippet(), Error::UnsupportedTag { .. }));
}

#[cfg(feature = "serialize")]
#[rstest]
#[case(Choice::Timestamp)]
#[case(Choice::Maybe(None))]
#[case(Choice::Maybe(Some(42)))]
fn present_optional_enums_round_trip(#[case] expected: Choice) {
    let value = Some(expected);
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(from_str::<Option<Choice>>(&yaml).unwrap(), value);
}

#[cfg(feature = "serialize")]
#[test]
fn tagged_optional_enum_round_trips() {
    let value = Tagged(Some(Choice::Timestamp), Some("!timestamp".into()));
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(from_str::<Tagged<Option<Choice>>>(&yaml).unwrap(), value);
}

#[cfg(feature = "serialize")]
#[rstest]
#[case("!null")]
#[case("!now")]
#[case("!foo")]
fn application_tags_do_not_encode_absent_values(#[case] tag: &str) {
    let value = Tagged(None::<String>, Some(tag.into()));
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml, format!("{tag} null\n"));
    // The tag denotes a present value. Its null payload is not a valid String.
    assert!(from_str::<Tagged<Option<String>>>(&yaml).is_err());
    assert_eq!(
        from_str::<Tagged<Option<()>>>(&yaml).unwrap(),
        Tagged(Some(()), Some(tag.into())),
    );
    let tilde = Tagged(NullableTilde::<String>(None), Some(tag.into()));
    let yaml = serde_saphyr::to_string(&tilde).unwrap();
    assert_eq!(yaml, format!("{tag} ~\n"));
    assert!(from_str::<Tagged<NullableTilde<String>>>(&yaml).is_err());
}
''')

    p = "tests/local_tagged_documents.rs"
    replace(p, '''    Null(String),
}''', '''    Null(Option<String>),
    #[serde(rename = "now")]
    Now,
}''')
    replace(p, 'Payload::Null("payload".to_owned())', 'Payload::Null(Some("payload".to_owned()))')
    replace(p, 'Payload::Null("payload".into())', 'Payload::Null(Some("payload".into()))', 5)
    replace(p, "fn retained_documents_keep_option_null_resolution()", "fn retained_documents_preserve_present_optional_values()")
    replace(p, '[None, None, Some(Payload::Null(Some("payload".into())))]',
            '[Some(Payload::Null(None)), Some(Payload::Now), Some(Payload::Null(Some("payload".into())))]')
    replace(p, '''                Validated(None),
                Validated(None),''',
            '''                Validated(Some(Payload::Null(None))),
                Validated(Some(Payload::Now)),''', 2)
    replace(p, "fn garde_keeps_payloads_and_option_none_documents()", "fn garde_preserves_present_optional_documents()")
    replace(p, "fn validator_keeps_payloads_and_option_none_documents()", "fn validator_preserves_present_optional_documents()")
    with Path(p).open("a") as output:
        output.write(r'''
#[rstest]
#[case("!<tag:example.org,2026:unit> null")]
#[case("%TAG !e! tag:example.org,2026:\n--- !e!unit")]
fn global_application_tags_are_not_skipped_as_null(#[case] yaml: &str) {
    for result in documents::<Option<()>>(yaml) {
        assert_eq!(result.unwrap(), [Some(())]);
    }
}

#[rstest]
#[case("!unknown")]
#[case("!unknown null")]
fn optional_document_errors_are_not_hidden_by_null_resolution(#[case] yaml: &str) {
    for result in documents::<Option<Command>>(yaml) {
        assert!(result.is_err());
    }
}
''')

    p = "tests/binary.rs"
    replace(p, 'assert_eq!(serde_saphyr::from_str::<Option<ByteBuf>>(&yaml).unwrap(), None);',
            'assert!(serde_saphyr::from_str::<Option<ByteBuf>>(&yaml).is_err());')

    p = "README.md"
    replace(p, '''See [unsupported features](#unsupported-features) for null-like selectors inside
options. Multi-document APIs retain explicitly local-tagged documents and pass them
to the target deserializer; untagged and global null documents are still skipped.''',
            '''`Option<E>` preserves the same selector and payload: `!timestamp` becomes
`Some(E::Timestamp)`, and `!maybe null` becomes `Some(E::Maybe(None))` when `Maybe`
has an optional payload. Untagged empty scalars, `~`, `null`, and the standard `!!null`
tag still represent `None`. Unknown selectors and invalid payloads produce the same
errors as they do without the outer `Option`.

Multi-document APIs retain application-tagged documents and pass them to the target
deserializer; only untagged and standard-tagged null documents are skipped.''')
    replace(p, '''existing meanings. Matching local robotics tags can still select typed enum variants.
Serde's generic buffering''',
            '''existing meanings. Matching local robotics tags can still select typed enum variants.

An explicit application tag denotes a present value, including inside `NullableTilde<T>`
and `Tagged<Option<T>>`. Attaching `!foo` to `None` still emits `!foo null`, but that
node no longer deserializes as absence: its payload must be accepted by the inner type.
For example, `Tagged<Option<String>>` rejects it rather than returning `None`;
`Tagged<Option<()>>` reads it as `Some(())`. Use an untagged or standard-tagged null
for absence, or an enum variant with an optional payload for a present tagged value
whose payload is absent.

Serde's generic buffering''')
    lines = Path(p).read_text().splitlines(keepends=True)
    removed = [line for line in lines if line.startswith((
        "- Null resolution happens before the inner enum is known in `Option<E>`",
        "- Multi-document APIs retain explicitly local-tagged documents. For `Option<E>`",
    ))]
    if len(removed) != 2:
        raise RuntimeError("expected both obsolete Option limitations")
    Path(p).write_text("".join(line for line in lines if line not in removed))

    p = "CHANGELOG.md"
    replace(p, '''  Plain null-like payloads inside optional enums are resolved before the inner
  variant is known; this limitation is documented in the README. Non-null payloads
  such as `!null payload` now reach the inner enum rather than becoming `None`.''',
            '''  Optional targets preserve the tagged value before interpreting its payload:
  `!timestamp` can select a present unit variant, and `!maybe null` can select a
  present variant with an absent optional payload. Invalid payloads and unknown
  selectors report the same errors as direct enum targets instead of returning `None`.''')
    replace(p, '''- Preserve explicitly local-tagged documents in multi-document APIs instead of
  skipping them as null before deserializing the target type. Untagged and global
  null documents are still skipped. Optional targets retain local-tagged entries
  and resolve plain null-like payloads before selecting inner variants.''',
            '''- Preserve application-tagged documents in multi-document APIs instead of
  skipping them as null before deserializing the target type. Only untagged and
  standard-tagged null documents are skipped. Optional targets retain tagged values,
  including null-like enum payloads, as present values.
- An application-tagged `None`, such as `Tagged(None::<String>, Some("!foo".into()))`,
  still emits `!foo null`, but no longer reads back as absence. Its tag denotes a
  present value and its payload must be valid for the inner type. Use untagged or
  standard-tagged null for `None`; use an optional enum payload for a present value
  whose payload is absent.''')
    Path("/tmp/pr246-option-files.json").write_text(json.dumps(sorted(CHANGED)))
    print("CHANGED", json.dumps(sorted(CHANGED)), flush=True)


def api(path, payload):
    """Create a Git object in the fixed repository and return its decoded JSON."""
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
    """Export only changed source files, without creating commits or moving refs."""
    entries = []
    paths = json.loads(Path("/tmp/pr246-option-files.json").read_text())
    for path in paths:
        if not path.startswith(("src/", "tests/")) and path not in ("README.md", "CHANGELOG.md"):
            raise RuntimeError(f"unexpected export path: {path}")
        blob = api("git/blobs", {"content": Path(path).read_text(), "encoding": "utf-8"})
        entries.append({"path": path, "mode": "100644", "type": "blob", "sha": blob["sha"]})
    tree = api("git/trees", {"base_tree": BASE_TREE, "tree": entries})
    print("PREPARED_TREE", tree["sha"], flush=True)
    Path("/tmp/pr246-option-tree.json").write_text(json.dumps({"tree": tree["sha"], "paths": paths}))


if __name__ == "__main__":
    if sys.argv[1:] == ["export"]:
        export_tree()
    elif not sys.argv[1:]:
        prepare()
    else:
        raise SystemExit("expected no arguments or 'export'")
