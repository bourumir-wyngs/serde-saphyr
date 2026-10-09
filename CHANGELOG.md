# Changelog

## 2.0.0 Unreleased

### YAML changes
- Composite mapping keys are not compared regardless of entry order (thanks @yuxi-liu-wired)  .
- Commented<T> now writes its output replacing characters not allowed in comments (non printable) by spaces.

### Added

- Added `PropertySyntax::DockerCompose` for Compose-compatible property interpolation.
- Added the default-enabled `comments` feature, forwarding to granit-parser's matching
  feature. Builds with `default-features = false` must enable it to use `Commented<T>`,
  `CommentPosition`, comment options, or comment budget fields and reports. Without it,
  YAML comments are still accepted and validated, but their text is not retained.
- Added `Options::non_finite_float_policy` with `NonFiniteFloatPolicy::{PassThrough, Reject, AsString}`
  to configure non-finite floats delivered through `deserialize_any`. Explicit policies override
  the legacy boolean; leaving the field unconfigured preserves its behavior.
- Added the `borrowed_context` example showing recursive `Serialize` adapters that borrow
  an existing value tree and per-call formatting context, composing with `Tagged<T>` and
  style wrappers without building a second container tree or using thread-local state.
- Added `SerializerOptions::no_lang_directive` (default `false`) to suppress the
  `%YAML 1.2` directive and its leading `---` marker independently of `yaml_12`'s quoting. 
- Added `from_str_multiple` and `from_str_multiple_with_options` to deserialize multiple YAML
  documents into values that can borrow from the input string. The existing `from_multiple`
  and `from_multiple_with_options` APIs retain their `DeserializeOwned` bounds for compatibility.
- Added `from_bytes_multiple` and `from_bytes_multiple_with_options` to deserialize multiple YAML
  documents into values that can borrow from a UTF-8 byte slice. The existing `from_slice_multiple`
  and `from_slice_multiple_with_options` APIs retain their `DeserializeOwned` bounds for compatibility.
- The new string and byte multi-document APIs collect into any `C: Default + Extend<T>`,
  including vectors, queues, sets, and custom accumulators. Explicit type arguments now specify
  both the document and collection types, for example `from_str_multiple::<String, Vec<_>>(input)`.
  The deprecated APIs continue returning `Vec<T>` with their original signatures.
- Added an `error: Box<Error>` field to `Error::AliasError`, which keeps the original error
  when an aliased value fails to deserialize. A custom `MessageFormatter` or `Localizer`
  now applies to that error, and `std::error::Error::source` returns it
  ([#199](https://github.com/bourumir-wyngs/serde-saphyr/issues/199)).

### Changed

- Updated to granit-parser 2.0.0 and disabled its default features so comment support follows
  serde-saphyr's `comments` feature. Serialization-only builds remain independent of
  granit-parser, including when comment emission is enabled.
- Non-finite floats remain rejected by default in `deserialize_any`, including overflowing
  literals such as `1e999`. Opt into `PassThrough` to preserve them in float-capable visitors,
  untagged enums, and flattened float fields, or `AsString` to receive canonical strings.
  With `PassThrough`, `serde_json::Value` converts them to `Null` without an error.
  Direct `f32`/`f64` deserialization is unchanged.
- **Breaking:** `Error::AliasError` has a new mandatory `error: Box<Error>` field.
  Manual constructors must supply it, and patterns listing every field must add `error` or `..`.
  The variant name is unchanged. Rendering and `source()` use the original error; the
  deserializer still populates `msg` with an `error.to_string()` snapshot for existing handlers.

### Deprecated

- Deprecated `Options::reject_non_finite_typeless_float` in favor of `non_finite_float_policy`.
  When the new field is unconfigured, the boolean retains its original behavior: `true`
  (the default) rejects and `false` converts to strings. Use `Reject` or `AsString`, respectively,
  when migrating. Explicit policies override the boolean. Serialized options that omit both
  fields retain the 1.3.0 behavior of converting non-finite floats to strings;
  `Options::default()` continues to reject them.
- Deprecated `from_multiple` and `from_multiple_with_options` in favor of `from_str_multiple`
  and `from_str_multiple_with_options`, which support both owned and borrowed values. The old
  functions remain available with their original signatures for compatibility. When migrating
  function pointers or callbacks, wrap the new functions in forwarding closures if needed.
- Deprecated `from_slice_multiple` and `from_slice_multiple_with_options` in favor of
  `from_bytes_multiple` and `from_bytes_multiple_with_options`, which support both owned and borrowed
  values. The old signatures remain available; the same callback migration guidance applies.
- Updated internal callers, examples, tests, fuzz targets, and error hints to use the new APIs.
- Deprecated the `msg` field of `Error::AliasError` in favor of its structured `error` field.
  Existing `Error::AliasError { msg, .. }` patterns still work with a deprecation warning.

### Fixed

- Alias diagnostics preserve the precise location of a failing value inside an anchored mapping,
  including fields outside the anchor's snippet window. Plain messages, snippets, and `miette`
  reports include the distinct failing location; equal locations are not repeated.
- Alias error rendering passes the original error to the active `MessageFormatter`, including
  formatters that delegate other variants to the built-in formatter. Plain messages report
  the alias definition and use locations once through the active `Localizer`; snippet and
  `miette` diagnostics use location labels without embedding plain-text location suffixes.
  Custom location formatting and suppression now apply throughout aliased values
  ([#199](https://github.com/bourumir-wyngs/serde-saphyr/issues/199)).
- `std::error::Error::source` exposes the inner error for both `Error::AliasError` and
  `Error::WithSnippet`, preserving the source chain through the default snippet wrapper.
- Made `huge_documents` and `serde_derived_types` enable `deserialize`, fixing isolated feature
  builds with `--no-default-features`, including autopkgtests of Debian team (as [observed](https://dfsg-new-queue.debian.org/reviews/rust-serde-saphyr)).

## 1.3.0 Maintenance and performance release

### Fixed

- Integer mapping keys now compare by their parsed numeric value for duplicate detection and merge
  resolution, including inside composite keys (`0xB` and `11` compare equal). Deserializing to strings
  preserves the original spelling; `FirstWins` and `LastWins` retain the selected key's spelling
  and associated value.
- Composite keys containing null-like strings now preserve both the key and its associated value;
  for example, `{{"null": 1}: 2}` now round-trips correctly.
- Null mapping keys now remain distinct from null-like string keys during duplicate detection
  and merge resolution, including inside composite keys.
- Null detection now respects string tags and scalar styles, preserving null-like strings such as
  `!!str null`, including when deserializing to `Option<String>`.
- Explicit numeric tags (`!!int` and `!!float`) now deserialize correctly through `deserialize_any`,
  including quoted and block scalars.
- Floating-point deserialization now rejects incompatible core tags; for example,
  `from_str::<f64>("!!str 1.5")` now returns an error.
- Serialization now escapes U+FFFE and U+FFFF as `\uFFFE` and `\uFFFF`.
- Serialization now preserves newline-only strings in literal block scalars, including
  `LitStr` and `LitString`, by using keep chomping and retaining every empty line.
- Updated the granit-parser revision to preserve document boundaries after zero-indented root
  literal and folded block scalars, including empty strings.
- Preserve string scalars across YAML 1.1 readers (PR #90, thanks @NiklasRosenstein)
- The version increased to 1.3 because it uses granit-parser 1.3, and that is because granit-parser
  added two methods to `Input` for performance improvements. As serde-saphyr re-exports it,
   the version number must be increased to 1.3 as well, even if no new features are added to 
   this crate itself.

## 1.2.0 Maintenance release

### Changed

- Folded property-interpolation depth and work limits into `Budget`; property resource-limit
  failures are now reported through `Error::Budget` and `BudgetBreach`.
- Added the opt-in `Options::reject_unsupported_tags` strict mode. It rejects explicitly tagged
  scalar, sequence, and mapping nodes when their tag is unknown to serde-saphyr; the default remains
  permissive for compatibility with custom tagged enums. YAML 1.1 `!!merge` and `!!value` are
  accepted in this mode only as the exact scalar mapping keys `<<` and `=`, respectively, while
  robotics-only `!degrees` and `!radians` require both the `robotics` crate feature and
  `angle_conversions`, and `!include` requires both the `include` crate feature and a configured
  resolver.
- Enforced the scalar, sequence, or mapping node kinds required by recognized tags even when
  `reject_unsupported_tags` is disabled.
- Hardened serializer indentation handling: `indent_step` is now limited to `1..=64`, all
  serializer entry points validate it, and indentation arithmetic returns an error instead of
  overflowing. We do not consider this breaking because values outside this range does not look sane.
- Validated custom anchor-generator names before emission. Names must be 1–256 bytes and cannot
  contain whitespace, control characters, or YAML flow punctuation; unsupported names now return
  a serialization error.

### Fixes

- Avoided unnecessary quotes around string keys and values containing an inline `#`, such as
  `a#b`, while retaining quotes for leading or whitespace-separated hashes. Borrowed from
  [commit 1119a54](https://github.com/bourumir-wyngs/serde-saphyr/commit/1119a54cc184d2151d53089f6be1b311b0f57e5a)
  under the terms of the Apache/MIT licenses. When property interpolation is configured,
  newly plain values such as `${NAME}#fragment` can interpolate; use `quote_all` to preserve
  literal values.
- Recognized explicit YAML 1.1 `!!merge` keys, including verbatim tags and `%TAG`-expanded
  handles, everywhere implicit `<<` merge keys are supported.
- Recognized the YAML 1.1 `!!value` tag while intentionally treating it as a no-op annotation.
- Accepted valid zero-indented root folded block scalars, including `#`-prefixed content lines.
- Fixed externally tagged `typetag` trait-object deserialization by consuming the closing mapping
  event when a Serde map visitor returns after its final key/value pair, preventing a false
  "multiple YAML documents" error.
- Rejected non-UTF-8 canonical include and root-file paths before resolver policy checks and source
  identity handling, preventing lossy path collisions and policy bypasses on Unix.
- Reported alias-use locations as primary for unsupported-tag and budget failures during replay,
  while retaining the anchor-definition locations as secondary context.

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
