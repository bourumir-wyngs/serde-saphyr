# Changelog

## 1.2.0 Maintenance release

### Changed

- Added the opt-in `Options::reject_unsupported_tags` strict mode. It rejects explicitly tagged
  scalar, sequence, and mapping nodes when their tag is unknown to serde-saphyr; the default remains
  permissive for compatibility with custom tagged enums.
- Hardened serializer indentation handling: `indent_step` is now limited to `1..=64`, all
  serializer entry points validate it, and indentation arithmetic returns an error instead of
  overflowing. We do not consider this breaking because values outside this range does not look sane.
- Validated custom anchor-generator names before emission. Names must be 1–256 bytes and cannot
  contain whitespace, control characters, or YAML flow punctuation; unsupported names now return
  a serialization error.

### Fixes

- Recognized explicit YAML 1.1 `!!merge` keys, including verbatim tags and `%TAG`-expanded
  handles, everywhere implicit `<<` merge keys are supported.
- Recognized the YAML 1.1 `!!value` tag while intentionally treating it as a no-op annotation.
- Accepted valid zero-indented root folded block scalars, including `#`-prefixed content lines.
- Fixed externally tagged `typetag` trait-object deserialization by consuming the closing mapping
  event when a Serde map visitor returns after its final key/value pair, preventing a false
  "multiple YAML documents" error.
- Rejected non-UTF-8 canonical include and root-file paths before resolver policy checks and source
  identity handling, preventing lossy path collisions and policy bypasses on Unix.

### Testing

- Reviewed yaml test suite, made sure all 350 active IDs and all 402 active cases are represented and documented
  we use  [YAML Test Suite v2022-01-17](https://github.com/yaml/yaml-test-suite/releases/tag/v2022-01-17).
- property test with 1,024 generated cases to check the round trip.
- added tests for [typetag](https://crates.io/crates/typetag).

## 1.1.0 Maintenance release

### Added

- Added granit-parser resource limits to `Budget` (#172):
  - `max_buffered_comment_events` (default: 32)
  - `simple_key_max_lookahead` (default: 1,024 characters)
  - `flow_nesting_limit` (default: 255)

  The limits are applied to parsers created for strings, readers, standalone budget checks, and
  included YAML sources. When the `serde_derived_types` feature is enabled, deserializing an older
  `Budget` representation that omits these fields uses the documented defaults.

### Fixes

- Fixed enums tags for struct variants (#177).
- Improved error message wording (#178).
