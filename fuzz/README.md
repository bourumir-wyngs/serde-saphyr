# Fuzzing

Run commands from the repository root with Rust nightly and `cargo-fuzz` installed.
Targets use AddressSanitizer, debug assertions, and overflow checks by default.

| Target | Checks |
| --- | --- |
| `raw_deserialize` | Arbitrary bytes, typed and generic values, duplicate/schema/tag options, chunked and faulting readers, stream recovery, resource limits. |
| `duplicate_keys` | Exact duplicate-policy outcomes, integer spelling equivalence, nested composite keys, and null/string distinctions. |
| `aliases_merges` | Alias values and merge precedence across duplicate policies and reader/string entry points. |
| `document_streams` | Exact document values/counts across string, slice, and chunked-reader APIs; single-document rejection; block styles, chomping, null filtering, and document markers. |
| `serialize` | Round-trip equality and stable reserialization for untrimmed strings, Unicode, wrappers, recursive values, and varied serializer options. |
| `flow_collections` | Malformed flow mappings/sequences and nested typed collections. |
| `large_scalars` | Plain and block scalar handling at sizes from 256 bytes to 1 MiB, including short seed inputs. |
| `properties` | Property syntax, nested operators, and expansion-budget handling. |

Malformed-input targets accept ordinary parsing, type, and budget errors. The
structured targets generate bounded valid YAML or Rust values and assert the
expected result, so silent value changes also become fuzz findings. Do not catch
panics or discard unexpected errors in these assertions.

## Run and reproduce

```sh
cargo +nightly fuzz build
mkdir -p fuzz/corpus/document_streams
cargo +nightly fuzz run document_streams \
  fuzz/corpus/document_streams fuzz/seeds/document_streams -- \
  -max_total_time=60 -rss_limit_mb=1024 -max_len=65536 \
  -timeout=15 -dict=fuzz/yaml.dict
```

`fuzz/seeds/<target>` contains small tracked starting inputs. The ignored
`fuzz/corpus/<target>` directory comes first because libFuzzer writes generated
inputs only to the first corpus directory. Keep both when starting a campaign.
`fuzz/yaml.dict` supplies YAML punctuation, tags, anchors, escapes, and property
operators. Scheduled CI discovers and runs every registered target for five
minutes each, retaining crash artifacts even when a target fails.

For a deterministic smoke run, replace `-max_total_time=60` with
`-runs=1000 -seed=1`. Set `CARGO_NET_OFFLINE=true` if dependencies are already
cached. Reduce compile parallelism with `CARGO_BUILD_JOBS=2` on smaller machines.
If LeakSanitizer reports a sandbox/ptrace failure at shutdown, use
`ASAN_OPTIONS=detect_leaks=0` for that local run; AddressSanitizer remains enabled.
Do not use this workaround for ordinary sanitizer findings.

```sh
cargo +nightly fuzz run TARGET fuzz/artifacts/TARGET/crash-HASH
cargo +nightly fuzz tmin TARGET fuzz/artifacts/TARGET/crash-HASH
```

Before promoting a crash, verify whether the generated input violates a harness
assumption or exposes a library bug. Preserve the minimized input and add a
normal regression test for a library fix.

### Newline-only serializer regression

The stronger round-trip checks found that serializing a newline-only string lost
its newlines. The serializer now preserves these values. The single-newline
reproducer is retained and also included in the starting seeds:

```sh
cargo +nightly fuzz run serialize fuzz/reproducers/serialize_newline.txt
cargo test --test test_block_str newline_only_strings_roundtrip
```

Both commands must pass; the normal regression and fuzz assertion remain active.

### Enum-indentation regression

A follow-up campaign found a nested-enum round-trip failure with
`indent_step: 1`. The serializer now accounts for the two-column `- ` prefix
when indenting enum payloads. The original crash input
`crash-c22b1997aa76cea333b69ac9e77d1e4a3a5d6e69` is retained byte-for-byte in
`fuzz/seeds/serialize/enum_indent.txt`. Another minimized input is saved in
`fuzz/reproducers/serialize_enum_indent.hex` as two hex-encoded bytes (`1c 65`);
it generates `EnumTuple(EnumStruct { field: Null }, Null)`.

The serializer target also supports normal tests, reusing its complete round-trip
checks for the original input and both minimized inputs (`F%%` and `1c 65`):

```sh
cargo test --manifest-path fuzz/Cargo.toml --bin serialize
cargo test --test en_structs
cargo +nightly fuzz run serialize fuzz/seeds/serialize/enum_indent.txt
```

The main crate's tests cover the generated enum shapes, empty variants, nested
sequences, and compact/non-compact layouts with one-, two-, and four-space
indentation.

## Input formats

`serialize` uses the entire input as an untrimmed UTF-8-lossy string, and also
attempts bounded recursive value generation. Its corpus can contain readable
strings, including whitespace and Unicode. Older binary tree seeds remain useful
mutation inputs, but their recursive interpretation has changed.

`duplicate_keys` and `aliases_merges` use leading control bytes to choose a
scenario; remaining bytes generate values. `document_streams` uses five control
bytes for line endings, block style/chomping/indentation, content, blank lines,
and separators, followed by up to 16 scalar selectors. Missing bytes default to
zero, so even an empty input reaches the document-boundary regression. These
structured targets' seed files are generator inputs, not literal YAML fixtures.
