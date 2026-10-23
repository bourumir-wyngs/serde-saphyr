"""Prepare the 3.0 release metadata and migration documentation without moving refs."""
from pathlib import Path
import copy
import json
import os
import re
import subprocess
import tarfile
import tomllib
from urllib.request import Request, urlopen

REPO = "bourumir-wyngs/serde-saphyr"
BASE_TREE = "958a8525814c70cb27771d3b59a17b8b4e9eb287"
VERSION = "3.0.0-rc.1"
FILES = ("Cargo.toml", "Cargo.lock", "fuzz/Cargo.lock", "src/ser/options.rs", "README.md", "CHANGELOG.md")

MIGRATION = r'''### YAML changes in 3.0.0

Version 3.0 changes YAML interpretation and serialization, even where the same Rust
code still compiles. Existing documents can produce different values or errors,
and some serialized output changes. These rules are the defaults in
`3.0.0-rc.1`; there is no legacy local-type-alias mode. Test existing configuration
files and stored YAML, not just compilation, before upgrading from 2.x.

The examples below assume the default tag handles unless a `%TAG` directive is
shown. [YAML tag handles](https://yaml.org/spec/1.2.2/#6821-tag-handles) are expanded
before interpretation; counting exclamation marks alone is not sufficient.

#### Local type aliases are removed

Earlier releases treated the following local application tags as aliases of
standard YAML types. Version 3.0 removes those aliases:

| Former local alias | Use for a standard type annotation |
| --- | --- |
| `!str` | `!!str`, or quote the string |
| `!int` | `!!int` |
| `!float` | `!!float` |
| `!bool` | `!!bool` |
| `!null` | `!!null`, or an untagged null |
| `!seq` | `!!seq`, or an untagged sequence |
| `!map` | `!!map`, or an untagged mapping |
| `!timestamp` | `!!timestamp` |
| `!binary` | `!!binary` |

The old spellings remain valid application tags; they are not invalid YAML.
They no longer request the corresponding built-in conversion or node-shape check.
By default they follow the same policy as other application tags: they can select
enum variants, are captured by `Tagged<T>`, and are otherwise ignored where possible.
With `reject_unsupported_tags: true`, these newly unsupported aliases are rejected,
including when used as enum selectors. Strict mode already rejects unknown custom
enum tags; this change adds the former aliases to that category.

This can silently change successfully parsed values. For example, when the target
is `serde_json::Value` with default options:

| Input | Previous alias behavior | 3.0 behavior |
| --- | --- | --- |
| `!str 123` | String `"123"` | Number `123` |
| `!int '123'` | Number `123` | String `"123"` |
| `!null 123` | Null | Number `123` |
| `!!str 123` | String `"123"` | String `"123"` |

For concrete Rust targets the target type still controls deserialization, so an
input can instead produce a type error. For example, use `!!binary AQID` to request
base64 decoding; local `!binary AQID` no longer requests that conversion.

Nonstandard global aliases such as `!<tag:yaml.org,2002:!str>` are also removed.
The standard URI is `tag:yaml.org,2002:str`, without the extra `!` after the colon.
The same distinction applies to all nine type names above, on both input and output.

Replace local tags only where they were intended as type annotations. Do not change
an intentional enum selector such as `!timestamp` into `!!timestamp`. Likewise,
inspect `%TAG` directives rather than performing a blanket text replacement:

```yaml
%TAG ! tag:yaml.org,2002:
---
value: !str 123  # Still a standard string: the directive changes the resolved tag.
```

Conversely, `%TAG !! !` makes `!!str` resolve to the local application tag `!str`.
Verbatim tags and named handles use the same resolved-identity rule.

Standalone `!` remains a separate non-specific tag: `! 123` is a string, and
`! [123, ! 123]` is a sequence containing a number and a string. It does not select
an enum variant and is not changed by `%TAG` remapping. Bare `!!` is not an alias
for standalone `!`; a shorthand using the `!!` handle requires a suffix.

Intentional extensions `!include`, `!time`, `!degrees`, and `!radians` remain
available under their feature and configuration requirements. Their supported
extension aliases are not removed with the standard-type aliases.

#### Enum selection precedes payload interpretation

A resolved local tag matching a Rust enum variant selects that variant before
its payload is interpreted, including names such as `str`, `int`, `null`, `seq`,
`map`, and `timestamp`. The selector is then removed and the payload is deserialized
into the variant's actual field type. A local `!int` can therefore select a variant
whose field is a string or a collection; it does not impose an integer conversion.

Do not depend on a former type alias overriding a matching enum selector, or on
an unmatched local alias being ignored as a built-in string annotation. Recognized
global built-in tags remain type annotations, not selectors for similarly named
variants. Use local selectors or externally tagged mappings for those variants.
`Tagged<T>` serialization follows the same distinction; a recognized global built-in
tag cannot be used to suppress a payload variant's external representation.

Serde's buffering for untagged enums does not provide the direct enum's variant
list. In particular, a local `!map` is no longer a standard mapping annotation
through that buffering, so an ordinary mapping arm that accepted it can stop
matching. Use `!!map` or no tag for an ordinary mapping, and a compatible tagged
enum representation when the tag is part of the data. The existing
`!!EnumName VARIANT` syntax for supported single-level scalar enums is unchanged;
it does not make recognized standard tags into variant selectors.

#### Tagged values are present inside options

Previously, some application-tagged empty or null-like scalars were consumed as
`None` before the inner target could see the tag. An explicit application tag now
denotes a present value: `Option<T>` passes the intact node to `T` and wraps a
successful result in `Some`. This does not depend on whether `T` is an enum.

For an enum with unit variant `Timestamp`, newtype variant `Count(u32)`, and
newtype variant `Maybe(Option<u32>)`, using their lowercase local selectors:

| Input | Result as `Option<E>` in 3.0 |
| --- | --- |
| Untagged empty scalar, `null`, or `~` | `None` |
| `!!null` | `None` |
| `!timestamp`, `!timestamp null`, or `!timestamp ~` | `Some(E::Timestamp)` |
| `!count 42` | `Some(E::Count(42))` |
| `!maybe`, `!maybe null`, or `!maybe ~` | `Some(E::Maybe(None))` |
| `!maybe 42` | `Some(E::Maybe(Some(42)))` |
| `!count` or an unknown selector | The same error as for the direct enum target |

A null payload is not an absent enum value. Local `!null payload` no longer forces
absence either: it can select a `Null(String)` variant with payload `"payload"`.
Nested options, optional fields and keys, `NullableTilde<T>`, and tagged wrappers
follow the same presence rule. Untagged nulls and resolved standard null tags still
represent absence; `%TAG` and verbatim forms of the standard null tag work as before.

One formerly supported round-trip intentionally changes:
`Tagged(None::<String>, Some("!foo".into()))` still writes `!foo null`, but reading
that node as `Tagged<Option<String>>` now errors instead of producing `None`.
Reading it as `Tagged<Option<()>>` produces a tagged `Some(())`. Likewise, wrapping
an absent `NullableTilde<T>` in an application tag writes `!foo ~`, which is now a
present tagged node. For absence, use an untagged or standard-tagged null. For a
present tagged value with an absent payload, use an enum variant with an optional
field. Do not attach an application tag to outer `None` and expect to preserve
absence on reading.

#### Mapping-key identity includes local tags

Duplicate detection and merge expansion now preserve resolved local tag identities,
including inside composite keys. Local `!str a` is distinct from standard `!!str a`
and from untagged `a`; different local tags also remain distinct. For example:

```yaml
{!str a: 1, a: 2}
```

These are two distinct YAML keys, rather than an alias-tagged duplicate. Similarly,
local `!int 0xB` and `!int 11` retain their payload spellings and are distinct.
Standard `!!int 0xB` and `!!int 11` still compare numerically, as do equivalent
untagged integer keys. Equivalent spellings of the same local tag, including
verbatim and directive-expanded forms, still compare equally.

This changes which entries are duplicates and which entries survive `FirstWins`,
`LastWins`, or merge expansion. YAML key equality is separate from Rust key equality:
if conversion discards a tag, a receiving `HashMap` or `BTreeMap` can still combine
otherwise distinct YAML keys. Preserve the distinction in the target key type when
it matters; do not rely on the previous alias-based duplicate detection.

#### Multi-document APIs retain tagged null-like documents

Application-tagged scalar documents with empty or null-like payloads now reach
the target deserializer instead of being skipped as null. This includes local tags
and global application-tag URIs, through string, byte, reader, and validation APIs.
Only implicit null scalar documents and resolved standard-null scalar documents
are skipped by the multi-document null filter.

A stream containing a document `--- !timestamp` can therefore yield an enum value
where it previously yielded no entry. With `Option<E>`, that document becomes a
present variant rather than a skipped value or `None`. A retained document with an
unknown selector or invalid payload now reports an error. Review assumptions about
result counts, omitted documents, and ignored invalid data when upgrading.

#### Tagged unit variants omit the written null payload

A matching local tag on a unit enum variant now writes only the tag:

```yaml
# Previously written for Tagged(E::Timestamp, Some("!timestamp".into())):
!timestamp null
```

```yaml
# Written in 3.0:
!timestamp
```

This also applies through optional and tagged wrappers and inside containers.
Reading still accepts omitted content and explicit plain null spellings, including
`null`, `Null`, `NULL`, and `~`. Non-null content is rejected for a unit variant,
not silently discarded: quoted `"null"`, numbers, and collections are not unit
payloads. Use a variant with fields when the content is meaningful.

Actual fields keep their serialized payloads: `!count 42`, `!maybe null`, and
`!maybe 42` are not shortened. Empty tuple and struct variants retain `[]` and `{}`
respectively. Ordinary enum serialization without an explicit matching local tag
is unchanged. Update exact-output snapshots for tag-only units, and test old readers
before exchanging new output with applications still using 2.x.

#### Additional scalar and collection corrections since 2.0

Recognized scalar tags are checked consistently for numeric, boolean, character,
and enum targets. Incompatible tags no longer disappear during conversion. For
example, do not use standard `!!timestamp` as a numeric duration annotation or
`!time` on a boolean. The robotics tags `!time`, `!degrees`, and `!radians` require
the `robotics` feature and `angle_conversions: true` for floating-point conversions
and untyped reads. Enabled conversions also run for skipped and flattened values;
invalid tagged payloads can therefore fail in paths that previously ignored them.
Use `!time` for durations in seconds, not `!!timestamp`, and remove annotations
where an ordinary number or boolean is intended. Local robotics tags can still
select matching typed enum variants before their payloads are interpreted; include
processing and strict tag rejection retain their separate requirements.

YAML null no longer constructs empty sequences, mappings, or mapping-based structs.
This covers blank values, `~`, `null`, `Null`, `NULL`, and standard-tagged nulls.
Write the intended collection explicitly:

```yaml
items: []     # Not items: or items: null
settings: {}  # Not settings: or settings: null
```

A mapping-based struct must still satisfy its field requirements. Ordinary null
continues to produce `None` for `Option<T>`. `#[serde(default)]` still handles an
absent field; it does not make a present null valid for a collection field.
`SerializerOptions::empty_as_braces` is deprecated since `3.0.0-rc.1`: leave it at
its default `true` to write `[]` and `{}`. The retained `false` setting emits blank
values for empty block collections without tags or anchors; those values are null
and no longer round-trip into collection types.

Implicit null recognition uses only the standard plain spellings: empty, `~`,
`null`, `Null`, and `NULL`. Mixed capitalization such as `NulL` remains a string,
not null, so it is not skipped as a null document or treated as outer `None`.
With `strict_booleans: true`, only `true`, `True`, `TRUE`, `false`, `False`, and
`FALSE` are boolean spellings. Mixed forms such as `tRuE` remain strings during
inference, are allowed as unquoted text with `no_schema`, and are rejected for
boolean targets. Use standard spellings for nulls and booleans, and quote text
whose exact spelling must be preserved.

Before upgrading, check type-like local tags and `%TAG` directives, tagged optional
values, duplicate and merged keys, document streams, empty collections, and exact
serialized-output assertions. Test with both the intended Rust types and generic
values where your application uses them; those paths need not coerce scalars alike.

'''


def replace(path, old, new, count=1):
    """Replace old with new in path exactly count times; raise on unexpected source."""
    p = Path(path)
    source = p.read_text()
    if source.count(old) != count:
        raise RuntimeError(f"{path}: expected {count} matches, got {source.count(old)}: {old[:100]!r}")
    p.write_text(source.replace(old, new))


def bump_lock(path):
    """Change only this crate's local lockfile version; verify other entries are unchanged."""
    p = Path(path)
    before = tomllib.loads(p.read_text())
    source, count = re.subn(
        r'(\[\[package\]\]\nname = "serde-saphyr"\nversion = ")2\.1\.0(")',
        rf'\g<1>{VERSION}\2', p.read_text(),
    )
    if count != 1:
        raise RuntimeError(f"{path}: expected one local package entry, found {count}")
    p.write_text(source)
    expected = copy.deepcopy(before)
    packages = [package for package in expected['package'] if package['name'] == 'serde-saphyr']
    if len(packages) != 1 or 'source' in packages[0]:
        raise RuntimeError(f"{path}: expected exactly one local serde-saphyr")
    packages[0]['version'] = VERSION
    assert tomllib.loads(source) == expected, f"{path}: unrelated lockfile changes"


def prepare():
    """Make only version and documentation edits; export review files, not a release."""
    replace('Cargo.toml', 'name = "serde-saphyr"\nversion = "2.1.0"',
            f'name = "serde-saphyr"\nversion = "{VERSION}"')
    for path in ('Cargo.lock', 'fuzz/Cargo.lock'):
        bump_lock(path)
    replace('src/ser/options.rs', 'since = "2.1.0"', f'since = "{VERSION}"')

    p = 'README.md'
    replace(p, '### Usage\n\n', '''### Usage

This branch targets the `3.0.0-rc.1` prerelease. To test this exact candidate after
it is published, opt in explicitly:

```toml
[dependencies]
serde-saphyr = "=3.0.0-rc.1"
```

Ordinary `"2"` and `"3"` requirements do not select this prerelease. See
[Cargo's prerelease rules](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#pre-releases)
and [YAML changes in 3.0.0](#yaml-changes-in-300) before upgrading.

''')
    replace(p, 'serde-saphyr = { version = "2",',
            'serde-saphyr = { version = "=3.0.0-rc.1",', 2)
    replace(p, 'cargo install serde-saphyr --features',
            'cargo install serde-saphyr --version 3.0.0-rc.1 --features', 2)
    source = Path(p).read_text()
    start = source.index('### Migrating from 2.0\n')
    end = source.index('### Migrating from 1.x\n', start)
    replace(p, source[start:end], MIGRATION)
    source = Path(p).read_text()
    start = source.index('A bug fix removes the implicit conversion of YAML null')
    end = source.index('Version 2.0 lets `from_multiple`', start)
    replace(p, source[start:end],
            'When upgrading from 1.x, also apply the [YAML changes in 3.0.0](#yaml-changes-in-300)\n'
            'above. The following API migration notes describe the earlier 2.0 transition.\n\n')
    replace(p, 'Migration: replace former local type aliases with standard tags',
            'See [YAML changes in 3.0.0](#yaml-changes-in-300) for the complete migration guide.\n\n'
            'Migration: replace former local type aliases with standard tags')

    p = 'CHANGELOG.md'
    moved = [
        '- Tags selecting unit enum variants now reject non-null payloads instead of silently\n'
        '  discarding them (#236). Use a bare variant name, a tag with empty content, or a\n'
        '  tag followed by `~` or `null`; quoted strings and collections are rejected.\n',
        '- Recognize only the standard plain null spellings: empty, `~`, `null`, `Null`, and `NULL`.\n'
        '  Other capitalization such as `NulL` remains a string instead of being rejected, converted\n'
        '  to an empty value, or skipped as a document ([#234](https://github.com/bourumir-wyngs/serde-saphyr/issues/234)).\n',
        '- `strict_booleans: true` now recognizes only the six YAML 1.2 Core boolean spellings.\n'
        '  Other capitalization such as `tRuE` remains a string during inference, is accepted\n'
        '  as unquoted text with `no_schema`, and is rejected for boolean targets.\n',
    ]
    for note in moved:
        replace(p, note, '')
    replace(p, '## 2.1.0 Unreleased\n',
            '## 3.0.0-rc.1 Unreleased\n\n### Breaking YAML changes\n\n'
            'This prerelease changes tag interpretation, optional tagged values, mapping-key\n'
            'identity, document filtering, and tagged unit output. These are behavioral\n'
            'incompatibilities even when existing Rust code compiles. The corrected semantics\n'
            'are the defaults; no legacy alias mode is provided. See\n'
            '[YAML changes in 3.0.0](README.md#yaml-changes-in-300) for all migration details.\n\n'
            + ''.join(moved))
    replace(p, '  Write `[]` for an empty sequence or `{}` for an empty mapping. `Option<T>` null handling\n'
            '  and defaults for absent fields are unchanged; see the README migration notes.',
            '  Write `[]` for an empty sequence or `{}` for an empty mapping. Untagged and\n'
            '  standard-tagged nulls remain `None` for `Option<T>`; defaults for absent fields\n'
            '  are unchanged. See the README for the separate tagged-value presence change.')

    assert tomllib.loads(Path('Cargo.toml').read_text())['package']['version'] == VERSION
    for path in FILES:
        assert '2.1.0' not in Path(path).read_text(), f"stale unreleased version in {path}"
    readme = Path('README.md').read_text()
    assert readme.count('### YAML changes in 3.0.0\n') == 1
    assert readme.count('```') % 2 == 0
    assert '#yaml-changes-in-300' in readme
    assert 'Migrating from 2.0\n' not in readme
    subprocess.run(['git', 'diff', '--check'], check=True)
    changed = subprocess.check_output(['git', 'diff', '--name-only'], text=True).splitlines()
    assert set(changed) == set(FILES), changed
    diff = subprocess.check_output(['git', 'diff', '--', *FILES], text=True)
    Path('/tmp/pr246-release-docs.patch').write_text(diff)
    Path('/tmp/pr246-yaml-changes-3.0.0.md').write_text(MIGRATION)
    with tarfile.open('/tmp/pr246-release-docs-source.tar.gz', 'w:gz') as archive:
        for path in FILES:
            archive.add(path, arcname=path)
    subprocess.run(['git', 'diff', '--stat', '--', *FILES], check=True)
    print('CHECKS: manifest/lockfile versions consistent; unrelated lock entries unchanged; '
          'no stale 2.1.0 labels; Markdown fences balanced; whitespace clean; edit scope verified.', flush=True)


def api(path, payload):
    """Create a Git object in the fixed repository and return the decoded response."""
    request = Request(
        f'https://api.github.com/repos/{REPO}/{path}',
        data=json.dumps(payload).encode(),
        headers={'Authorization': f"Bearer {os.environ['GH_TOKEN']}",
                 'Accept': 'application/vnd.github+json', 'Content-Type': 'application/json'},
        method='POST',
    )
    with urlopen(request, timeout=60) as response:
        return json.load(response)


def export_tree():
    """Upload only the six reviewed files; do not create commits, refs, tags, or releases."""
    entries = []
    for path in FILES:
        blob = api('git/blobs', {'content': Path(path).read_text(), 'encoding': 'utf-8'})
        entries.append({'path': path, 'mode': '100644', 'type': 'blob', 'sha': blob['sha']})
    tree = api('git/trees', {'base_tree': BASE_TREE, 'tree': entries})
    result = {'tree': tree['sha'], 'version': VERSION, 'files': entries}
    Path('/tmp/pr246-release-docs-tree.json').write_text(json.dumps(result, indent=2))
    print('PREPARED_TREE', json.dumps(result), flush=True)


if __name__ == '__main__':
    prepare()
    export_tree()
