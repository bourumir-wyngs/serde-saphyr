# Changelog

## 1.1.0 Unreleased

### Added

- Added granit-parser resource limits to `Budget`:
  - `max_buffered_comment_events` (default: 32)
  - `simple_key_max_lookahead` (default: 1,024 characters)
  - `flow_nesting_limit` (default: 255)

  The limits are applied to parsers created for strings, readers, standalone budget checks, and
  included YAML sources. When the `serde_derived_types` feature is enabled, deserializing an older
  `Budget` representation that omits these fields uses the documented defaults.
