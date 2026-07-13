# serde-saphyr correctness and robustness review

## Review scope

I reviewed the current `master` branch at commit `a06feaf35ab131aa0c4006b0a52e42245e517ecd`, dated July 8, 2026. This is two commits beyond the 0.0.29 release.

Source repository: <https://github.com/bourumir-wyngs/serde-saphyr>

This was a static source audit. I could not run `cargo test`, Miri, or the fuzzers in the available execution environment, so I distinguish existing tested behavior from source-path conclusions that need a regression test in CI.

## Overall assessment

| Area | Assessment |
|---|---|
| Panics from ordinary malformed YAML | Good |
| Depth and alias-bomb resistance | Good foundation |
| Single-document stream correctness | One high-priority defect |
| Explicit YAML tag correctness | Needs a focused rewrite |
| Reader error propagation | High-priority concern |
| Serde-version stability | Brittle in two places |
| Fuzz coverage of raw malformed input | Incomplete |
| Public behavior stability | Generally careful, but some questionable behavior is already regression-tested |

The core is much better defended than the average Serde format implementation. I did not find an ordinary malformed-input route to an obvious `unwrap`, indexing panic, or unconditional `panic!` in the main event/deserialization path. Existing tests cover malformed escapes, invalid flow structures, undefined or cyclic aliases, EOF conditions, and extremely deep nesting, while the README explicitly documents budgets, fuzzing, Miri, and panic resistance.

Relevant test file: <https://github.com/bourumir-wyngs/serde-saphyr/blob/a06feaf35ab131aa0c4006b0a52e42245e517ecd/tests/no_panic.rs>

The most important problems are not classic panics. They are **successful parsing after an error** and **inconsistent handling of recognized YAML core tags**.

---

# 1. High: errors after `...` are deliberately suppressed

The shared single-document completion function currently does this:

1. Parse the first document.
2. Call `src.peek()` to determine whether more input exists.
3. Return `MultipleDocuments` if another valid event appears.
4. When `peek()` returns an error:
   - return it before a document-end marker;
   - ignore it after an explicit `...`.
5. Call `finish()`.

The repository has a regression test explicitly expecting malformed content following `...` to be ignored. Therefore, this is current documented-by-test behavior rather than an accidental uncovered branch.

Relevant source: <https://github.com/bourumir-wyngs/serde-saphyr/blob/a06feaf35ab131aa0c4006b0a52e42245e517ecd/src/de/with_deserializer.rs>

There are two problems.

## 1.1 Malformed suffixes are accepted, but valid suffixes are rejected

This input is rejected as multiple documents:

```yaml
id: 7
...
---
id: 8
```

This input is accepted, even though the complete YAML stream is not valid:

```yaml
id: 7
...
@
```

For an API named `from_str`, `from_slice`, or `from_reader`, successful return should normally mean that the complete supplied stream was accepted. YAML explicitly permits implementations to recover from malformed streams, but errors must remain reportable; silently changing a malformed complete stream into a successful prefix parse is not a good recovery model.

YAML specification: <https://yaml.org/spec/1.2.2/>

## 1.2 A late reader error appears to be swallowed

This is the more serious consequence.

By source-level inspection:

- `LiveEvents::peek()` checks the stored reader error.
- That check removes the error with `take()`.
- `enforce_single_document_and_finish()` discards the error when `seen_doc_end()` is true.
- The following `finish()` no longer sees the removed I/O error.

That means a reader that successfully returns:

```yaml
id: 7
...
```

and then fails while the parser checks for EOF can appear to parse successfully.

The malformed-suffix behavior is confirmed by the existing regression test. The I/O case is a source-path conclusion; the following test should establish it directly.

## Recommended fix

Make single-document entry points strict. Return every `peek()` error:

```rust
pub(crate) fn enforce_single_document_and_finish<'de, W>(
    src: &mut LiveEvents<'de>,
    multiple_docs_hint: &'static str,
    wrap_err: W,
) -> Result<(), Error>
where
    W: Fn(Error, &LiveEvents<'de>) -> Error,
{
    match src.peek() {
        Ok(Some(_)) => Err(wrap_err(
            Error::multiple_documents(multiple_docs_hint)
                .with_location(src.last_location()),
            src,
        )),
        Ok(None) => src.finish().map_err(|error| wrap_err(error, src)),
        Err(error) => Err(wrap_err(error, src)),
    }
}
```

This intentionally changes the currently tested suffix-ignoring behavior. I would treat it as a correctness fix and call it out in release notes.

Should prefix parsing remain desirable, it should be exposed as a distinct API that returns the consumed position or remainder. Silently giving prefix semantics to the ordinary `from_*` APIs makes application-level validation unreliable.

## Regression tests

```rust
use std::io::{self, Read};

use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Simple {
    id: u8,
}

#[test]
fn rejects_invalid_content_after_explicit_document_end() {
    let result = serde_saphyr::from_str::<Simple>("id: 7\n...\n@\n");

    assert!(
        result.is_err(),
        "a successful single-document parse must consume a valid complete stream"
    );
}

struct ErrorAfterPrefix {
    prefix: &'static [u8],
    position: usize,
    emitted_error: bool,
}

impl Read for ErrorAfterPrefix {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }

        if self.position < self.prefix.len() {
            let remaining = &self.prefix[self.position..];
            let count = remaining.len().min(output.len());

            output[..count].copy_from_slice(&remaining[..count]);
            self.position += count;

            return Ok(count);
        }

        if !self.emitted_error {
            self.emitted_error = true;

            return Err(io::Error::new(
                io::ErrorKind::Other,
                "injected read failure after document end",
            ));
        }

        Ok(0)
    }
}

#[test]
fn does_not_swallow_io_error_after_explicit_document_end() {
    let reader = ErrorAfterPrefix {
        prefix: b"id: 7\n...\n",
        position: 0,
        emitted_error: false,
    };

    let error = serde_saphyr::from_reader::<_, Simple>(reader)
        .expect_err("a late reader failure must be returned");

    assert!(matches!(
        error.without_snippet(),
        serde_saphyr::Error::IOError { .. }
    ));
}
```

This fix should be applied to every entry point sharing this completion helper, including the `with_deserializer_*` variants.

---

# 2. High: recognized core tags are not authoritative

Tag behavior is currently asymmetric between `deserialize_any`, typed scalar methods, sequences, and mappings.

The relevant current behavior is approximately:

| YAML | Target | Current source path implies | Correct strict result |
|---|---|---|---|
| `!!str null` | `serde_json::Value` | Null | String `"null"` |
| `!!null not-null` | `Option<String>` | `None` | Invalid tagged value |
| `!!str 42` | `u32` | `42` | Tag/type mismatch |
| `!!str true` | `bool` | `true` | Tag/type mismatch |
| `!!int 42` | `serde_json::Value` | Rejected/string-oriented path | Number `42` |
| `!!bool true` | `serde_json::Value` | Rejected/string-oriented path | Boolean `true` |
| `!!map [1, 2]` | `Vec<u8>` | Sequence accepted by node kind | Invalid recognized tag |
| `!!seq {a: 1}` | map | Mapping accepted by node kind | Invalid recognized tag |

The causes are visible in several places:

- `deserialize_any` checks explicit `!!null` and then implicit null-like spelling before allowing `!!str` to force string semantics.
- Integer and boolean typed methods consume the tag but discard it.
- Float methods retain the tag, primarily for extension-specific angle conversions.
- `expect_seq_start()` and `expect_map_start()` validate only the parser event kind.
- Sequence events preserve tag information, but mapping-start conversion currently discards its tag.
- An existing multi-document regression test treats `!!null not-null` as a null document that is skipped.

Relevant source: <https://github.com/bourumir-wyngs/serde-saphyr/blob/a06feaf35ab131aa0c4006b0a52e42245e517ecd/src/de/deserializer.rs>

For recognized YAML tags, the tag identifies the intended native type. Recognized tag content must be constructible as that type; `!!null not-null` should not silently construct null, and `!!str null` must stay a string. The YAML core schema defines separate null, boolean, integer, float, and string resolution rules.

YAML specification: <https://yaml.org/spec/1.2.2/>

## Recommended resolution order

For scalar nodes:

1. Inspect the explicit recognized tag.
2. When it is `!!str`, return a string without implicit null, boolean, or numeric resolution.
3. When it is `!!null`, validate that its contents are a permitted null representation.
4. When it is `!!bool`, parse as a boolean or return an invalid-scalar error.
5. When it is `!!int`, parse as an integer or return an invalid-scalar error.
6. When it is `!!float`, parse as a float or return an invalid-scalar error.
7. When it is a recognized sequence or mapping tag on a scalar, reject it.
8. Only apply implicit schema heuristics when no authoritative explicit core tag is present.

For containers:

- `!!seq` is valid only on a sequence.
- `!!map` is valid only on a mapping.
- Scalar core tags are invalid on containers.
- Preserve current custom-tag and enum semantics separately; unknown application tags do not necessarily need the same handling as recognized core tags.

Mapping events need to retain tag information just as sequence events do. Conceptually:

```rust
MapStart {
    anchor: usize,
    tag: SfTag,
    raw_tag: Option<Cow<'a, str>>,
    location: Location,
}
```

Then `expect_seq_start()` and `expect_map_start()` should validate compatible recognized tags before consuming the node.

## Core tag regression matrix

```rust
use std::collections::BTreeMap;

use serde_json::json;

#[test]
fn explicit_core_scalar_tags_drive_typeless_deserialization() {
    assert_eq!(
        serde_saphyr::from_str::<serde_json::Value>("!!str null").unwrap(),
        json!("null")
    );

    assert_eq!(
        serde_saphyr::from_str::<serde_json::Value>("!!int 42").unwrap(),
        json!(42)
    );

    assert_eq!(
        serde_saphyr::from_str::<serde_json::Value>("!!bool true").unwrap(),
        json!(true)
    );

    assert_eq!(
        serde_saphyr::from_str::<serde_json::Value>("!!float 1.5").unwrap(),
        json!(1.5)
    );
}

#[test]
fn invalid_explicit_null_content_is_rejected() {
    assert!(
        serde_saphyr::from_str::<Option<String>>("!!null not-null").is_err()
    );
}

#[test]
fn incompatible_scalar_core_tags_are_rejected() {
    assert!(serde_saphyr::from_str::<u32>("!!str 42").is_err());
    assert!(serde_saphyr::from_str::<bool>("!!str true").is_err());
    assert!(serde_saphyr::from_str::<String>("!!int 42").is_err());
}

#[test]
fn incompatible_container_core_tags_are_rejected() {
    assert!(serde_saphyr::from_str::<Vec<u8>>("!!map [1, 2]").is_err());

    assert!(
        serde_saphyr::from_str::<BTreeMap<String, u8>>(
            "!!seq {a: 1}"
        )
        .is_err()
    );
}

#[test]
fn explicit_string_null_is_not_collection_null() {
    assert!(serde_saphyr::from_str::<Vec<String>>("!!str null").is_err());

    assert!(
        serde_saphyr::from_str::<BTreeMap<String, String>>(
            "!!str null"
        )
        .is_err()
    );
}
```

I would build this into a table-driven test covering:

- every recognized core tag;
- scalar, sequence, and mapping nodes;
- `deserialize_any`;
- every typed scalar family;
- `Option<T>`;
- strings and borrowed strings;
- sequences, tuples, maps, and structs.

This is the largest correctness project in the parser, but it is bounded and testable.

---

# 3. Medium: production behavior depends on Serde internals and English error text

There are two brittle compatibility mechanisms.

## 3.1 Detecting Serde internal visitors from `type_name`

`deserialize_any` contains logic equivalent to:

```rust
std::any::type_name::<V>().contains("::private::de::")
```

This is used to distinguish Serde's internal buffering visitors from ordinary typeless visitors. Serde's `private` module naming is deliberately not a stable public interface. A Serde update can therefore silently change tagged-enum behavior without causing a compilation error. A user-defined type whose module path happens to contain the same fragment can also receive the internal behavior.

Relevant source: <https://github.com/bourumir-wyngs/serde-saphyr/blob/a06feaf35ab131aa0c4006b0a52e42245e517ecd/src/de/deserializer.rs>

There is no good stable reflection API that identifies Serde's internal content visitors. The better long-term solution is to make tagged-node capture independent of visitor identity.

Until that redesign, isolate this in one `serde_compat` module and test against:

- the minimum supported Serde version;
- the locked version;
- latest Serde on a scheduled CI job.

## 3.2 Matching `"expected a borrowed string"`

`deserialize_str` currently distinguishes a real visitor error from a transformed-`&str` failure with:

```rust
err.to_string().contains("expected a borrowed string")
```

This occurs in two branches. A custom visitor can return an unrelated application error containing those words and have it replaced by `CannotBorrowTransformedString`. Serde could also revise the wording.

Relevant source: <https://github.com/bourumir-wyngs/serde-saphyr/blob/a06feaf35ab131aa0c4006b0a52e42245e517ecd/src/de/deserializer.rs>

The crate already has structured Serde error variants, so the immediate improvement is straightforward:

```rust
fn expected_borrowed_string(error: &Error) -> bool {
    matches!(
        error.without_snippet(),
        Error::SerdeInvalidType { expected, .. }
            if expected == "a borrowed string"
    )
}
```

Use it in both branches:

```rust
match visitor.visit_string(value) {
    Ok(value) => Ok(value),

    Err(error) if expected_borrowed_string(&error) => {
        Err(
            Error::cannot_borrow_transformed(reason)
                .with_location(location)
        )
    }

    Err(error) => Err(error.with_location(location)),
}
```

This still depends on Serde's `Expected` text, but only for the structured `invalid_type` category. It no longer intercepts arbitrary `Error::custom` messages.

A regression test should create a custom visitor whose error text contains the phrase and assert that the original `Message` remains intact.

---

# 4. Medium policy risk: `null` becomes an empty collection or struct

The implementation deliberately permits null-like scalar values where Serde asks for a sequence or mapping:

```yaml
plugins: null
```

can deserialize into:

```rust
struct Config {
    plugins: Vec<Plugin>,
}
```

as an empty vector.

Likewise, a null can be exposed as an empty map or struct. This is covered by regression tests, so it is intentional compatibility behavior.

Relevant source: <https://raw.githubusercontent.com/bourumir-wyngs/serde-saphyr/a06feaf35ab131aa0c4006b0a52e42245e517ecd/src/de/deserializer.rs>

This is convenient for permissive configuration loading, but weakens the useful property that the Rust type acts as a schema. An explicit malformed or incorrectly generated value becomes indistinguishable from a deliberately empty collection. That sits uneasily beside the project's documented early type-mismatch behavior.

Project repository: <https://github.com/bourumir-wyngs/serde-saphyr>

I recommend making this an explicit policy:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum NullCollectionPolicy {
    #[default]
    Reject,

    Empty,
}
```

For compatibility, the migration can be:

1. Add `NullCollectionPolicy` while retaining `Empty` as the effective default.
2. Add a strict preset that uses `Reject`.
3. Announce the future default change.
4. Change the default in a planned compatibility-breaking release.

Irrespective of that policy, `!!str null` must never count as YAML null.

---

# 5. Medium-low: alias replay undercounts tag bytes

Alias replay is budgeted, which is good. It uses checked arithmetic for replay-event totals and has separate limits for expansion counts and replay stack depth.

Relevant source: <https://github.com/bourumir-wyngs/serde-saphyr/blob/a06feaf35ab131aa0c4006b0a52e42245e517ecd/src/de/live_events.rs>

However, the replay-budget adapter reconstructs events with tags removed:

```rust
Event::Scalar(..., None)
Event::SequenceStart(..., None)
Event::MappingStart(..., None)
```

The normal budget observer counts scalar and tag bytes. Replayed scalar contents are therefore charged repeatedly, but their tag spellings are not. This creates an inconsistency in `max_total_scalar_bytes` accounting and is especially visible for long local tag names repeated through aliases. Mapping tags cannot be charged correctly because they are discarded earlier.

This is bounded by the alias-event limits, so I do not consider it a major denial-of-service hole under default options. It is nevertheless a breach of the apparent budget contract.

Recommended implementation:

- Preserve the recognized tag and raw tag length on every node start.
- Let the budget enforcer observe an `Ev`-level normalized record directly.
- Avoid synthesizing a parser `Event` that loses metadata.
- Store `raw_tag_len: usize` when full raw text is not otherwise needed.

Add a threshold test where:

1. An anchored tagged node is within `max_total_scalar_bytes`.
2. One alias remains within the limit.
3. Another alias exceeds it only when replayed tag bytes are correctly included.

---

# 6. Medium-low panic risk: reentrant callbacks use `RefCell::borrow_mut`

The include resolver and owned budget-report callback are shared through `Rc<RefCell<_>>` and invoked with `borrow_mut()`.

A callback that starts a nested parse using cloned options containing the same callback can trigger `BorrowMutError`, which panics. This requires application extension code and is not reachable from malformed YAML alone, but it is avoidable library-originated panic behavior.

Relevant source: <https://github.com/bourumir-wyngs/serde-saphyr/blob/a06feaf35ab131aa0c4006b0a52e42245e517ecd/src/de/deserializer.rs>

For the include resolver, convert it to a normal error:

```rust
Box::new(move |request| {
    let mut resolver = rc_refcell.try_borrow_mut().map_err(|_| {
        crate::input_source::IncludeResolveError::Message(
            "include resolver was invoked reentrantly".to_owned(),
        )
    })?;

    resolver(request)
})
```

For the budget callback, add a structured error or define that reentrant reports are skipped. Returning an error is safer:

```rust
let mut callback = callback.try_borrow_mut().map_err(|_| {
    Error::ReentrantBudgetReportCallback {
        location: self.last_location,
    }
})?;

callback(report);
```

This does not catch a panic deliberately raised by user callback code. That should remain ordinary Rust callback behavior and be documented separately.

---

# 7. Lower-priority stability hardening

## Structured upstream parser errors

Some public error classification still appears to be derived from parser text, for example identifying an unknown anchor by checking whether external text contains `"unknown anchor"`.

That means an upstream wording change can alter the public `serde_saphyr::Error` category without a compiler failure. The durable fix is a structured error kind from `granit-parser`. Until then:

- centralize all text-to-kind mapping in one adapter;
- test it against the exact dependency version;
- always retain the original external message as a source or field.

## Counter overflow

Several budget counters use ordinary `+= 1`, while alias replay already uses checked arithmetic.

With normal limits, reaching `usize::MAX` is not realistic. Nevertheless, a deliberately unlimited or unusual long-running reader could:

- panic in debug builds;
- wrap in release builds;
- bypass a limit after wrapping.

A small helper would make the invariant uniform:

```rust
fn increment_counter(
    counter: &mut usize,
    location: Location,
) -> Result<(), Error> {
    *counter = counter.checked_add(1).ok_or(
        Error::BudgetCounterOverflow { location },
    )?;

    Ok(())
}
```

This is primarily defensive consistency rather than an urgent vulnerability.

---

# Raw malformed-input fuzz target

The existing fuzz targets are useful, but the reviewed ones construct mostly valid YAML scaffolding around lossy UTF-8 input. This biases exploration toward scalar and collection semantics rather than arbitrary lexer states, raw invalid bytes, stream completion, reader chunking, and I/O failures.

Example existing target: <https://github.com/bourumir-wyngs/serde-saphyr/blob/master/fuzz/fuzz_targets/flow_collections.rs>

I would add a raw target that exercises both typeless and typed visitors and multiple reader behaviors.

```rust
#![no_main]

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read};

use libfuzzer_sys::fuzz_target;
use serde::de::IgnoredAny;

struct Chunked<'a> {
    input: &'a [u8],
    position: usize,
    chunk_size: usize,
}

impl Read for Chunked<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() || self.position == self.input.len() {
            return Ok(0);
        }

        let count = output
            .len()
            .min(self.chunk_size)
            .min(self.input.len() - self.position);

        output[..count].copy_from_slice(
            &self.input[self.position..self.position + count],
        );

        self.position += count;

        Ok(count)
    }
}

struct Faulting<'a> {
    input: &'a [u8],
    position: usize,
    fail_at: usize,
    emitted_error: bool,
}

impl Read for Faulting<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }

        if !self.emitted_error && self.position >= self.fail_at {
            self.emitted_error = true;

            return Err(io::Error::new(
                io::ErrorKind::Other,
                "fuzz-injected reader failure",
            ));
        }

        if self.position == self.input.len() {
            return Ok(0);
        }

        let bytes_before_failure = if self.emitted_error {
            self.input.len() - self.position
        } else {
            self.fail_at.saturating_sub(self.position)
        };

        let count = output
            .len()
            .min(bytes_before_failure)
            .min(self.input.len() - self.position);

        if count == 0 {
            return Ok(0);
        }

        output[..count].copy_from_slice(
            &self.input[self.position..self.position + count],
        );

        self.position += count;

        Ok(count)
    }
}

macro_rules! exercise_slice_type {
    ($data:expr, $target:ty) => {
        let _ = serde_saphyr::from_slice::<$target>($data);
    };
}

fuzz_target!(|data: &[u8]| {
    /*
     * Bound per-input work independently of serde-saphyr's own budgets.
     * This prevents large corpus entries from reducing fuzz throughput.
     */
    if data.len() > 64 * 1024 {
        return;
    }

    exercise_slice_type!(data, IgnoredAny);
    exercise_slice_type!(data, bool);
    exercise_slice_type!(data, i64);
    exercise_slice_type!(data, u64);
    exercise_slice_type!(data, f64);
    exercise_slice_type!(data, String);
    exercise_slice_type!(data, Option<String>);
    exercise_slice_type!(data, Vec<IgnoredAny>);
    exercise_slice_type!(data, BTreeMap<String, IgnoredAny>);

    let _ = serde_saphyr::from_slice_multiple::<IgnoredAny>(data);

    let _ = serde_saphyr::from_reader::<_, IgnoredAny>(
        Cursor::new(data),
    );

    let chunk_size = data
        .first()
        .map_or(1, |byte| usize::from(*byte % 32) + 1);

    let _ = serde_saphyr::from_reader::<_, IgnoredAny>(Chunked {
        input: data,
        position: 0,
        chunk_size,
    });

    let fail_at = data.get(1).map_or(data.len(), |byte| {
        usize::from(*byte) % (data.len() + 1)
    });

    let _ = serde_saphyr::from_reader::<_, IgnoredAny>(Faulting {
        input: data,
        position: 0,
        fail_at,
        emitted_error: false,
    });

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = serde_saphyr::from_str::<IgnoredAny>(text);
        let _ = serde_saphyr::from_multiple::<IgnoredAny>(text);
    }
});
```

Add it to `fuzz/Cargo.toml`:

```toml
[[bin]]
name = "raw_deserialize"
path = "fuzz_targets/raw_deserialize.rs"
test = false
doc = false
bench = false
```

Useful initial corpus entries:

```text
id: 7
...
@
```

```text
!!null not-null
```

```text
!!str null
```

```text
!!int 42
```

```text
!!map [1, 2]
```

```text
!!seq {a: 1}
```

Also include:

- truncated UTF-8 sequences;
- UTF-8, UTF-16LE, and UTF-16BE BOMs with truncated code units;
- unterminated quoted scalars;
- truncated escape sequences;
- document markers split at every byte boundary;
- aliases split at every reader boundary;
- reader failures at every offset from zero through one byte beyond the input.

The strongest deterministic reader property is:

> Any injected `Read` error encountered before confirmed physical EOF must result in `Error::IOError`, regardless of document-end markers or chunk boundaries.

---

# Recommended pull-request order

1. **Strict stream completion:** stop suppressing errors after `...`; add malformed-suffix and late-I/O tests.
2. **Core tag matrix:** make recognized tags authoritative and preserve mapping tags.
3. **Serde compatibility cleanup:** remove error-string matching and quarantine private visitor detection.
4. **Raw fuzzing:** arbitrary bytes, typed visitors, chunked readers, and faulting readers.
5. **Resource accounting:** include tag bytes during alias replay and check all counters.
6. **Policy/API work:** make null-to-empty-collection behavior explicit and configurable.
7. **Extension hardening:** replace callback `borrow_mut()` panics with structured reentrancy errors.

The first two items materially affect whether invalid input can be accepted as valid. The rest improve long-term robustness around already strong defensive foundations.

The highest-value areas outside this pass are serializer/deserializer round-trip invariants, include-resolver trust and path boundaries, validation-wrapper equivalence across features, and behavior under dependency version changes.
