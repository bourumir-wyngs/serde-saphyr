# serde-saphyr

![panic-free](https://img.shields.io/badge/panic--free-%E2%9C%94%EF%B8%8F-brightgreen)
[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/bourumir-wyngs/serde-saphyr/rust.yml)](https://github.com/bourumir-wyngs/serde-saphyr/actions)
[![crates.io](https://img.shields.io/crates/v/serde-saphyr.svg)](https://crates.io/crates/serde-saphyr)
[![crates.io](https://img.shields.io/crates/l/serde-saphyr.svg)](https://crates.io/crates/serde-saphyr)
[![crates.io](https://img.shields.io/crates/d/serde-saphyr.svg)](https://crates.io/crates/serde-saphyr)
[![docs.rs](https://docs.rs/serde-saphyr/badge.svg)](https://docs.rs/serde-saphyr)
[![Fuzz & Audit](https://github.com/bourumir-wyngs/serde-saphyr/actions/workflows/ci.yml/badge.svg)](https://github.com/bourumir-wyngs/serde-saphyr/actions/workflows/ci.yml)

**serde-saphyr** is a strongly typed YAML deserializer built on
[`saphyr-parser`](https://crates.io/crates/saphyr-parser). It aims to be **panic-free** on malformed input and to avoid `unsafe` code in library paths. The crate deserializes YAML *directly into your Rust types* without constructing an intermediate tree of “abstract values.” This way it is not a fork of the older [`serde-yaml`](https://crates.io/crates/serde_yaml) and does not share any code with it.

### Why this approach?

- **Light on resources:** Having almost no intermediate data structures should result in more efficient parsing, especially if anchors are used only lightly.
- **Also simpler:** No code to support intermediate Value's of all kinds.
- **Type-driven parsing:** YAML that doesn’t match the expected Rust types is rejected early.
- **Safer by construction:** No dynamic “any” objects; common YAML-based code-execution [exploits](https://www.arp242.net/yaml-config.html) do not apply.

### Benchmarking

Parsing Generated YAML, size 25.00 MiB, release build.


| Crate                                                   | Time (ms) | Notes                                                                     |
| ------------------------------------------------------- |-----------|---------------------------------------------------------------------------|
| [serde-saphyr](https://crates.io/crates/serde-saphyr)   | 290.54 | No `unsafe`, no [unsafe-libyaml](https://crates.io/crates/unsafe-libyaml) |
| [serde-yaml-ng](https://crates.io/crates/serde-yaml-ng) | 470.72    |                                                                           |
| [serde-yaml](https://crates.io/crates/serde-yaml)       | 477.33    | Original, deprecated, repo archived                                       |
| [serde-norway](https://crates.io/crates/serde-norway)   | 479.57    |                                                                           |
| [serde-yml](https://crates.io/crates/serde-yml)         | 490.92    | Repo archived                                                             |
| [serde-yaml_bw](https://crates.io/crates/serde-yaml_bw) | 702.99    | Slow due Saphyr doing budget check first upfront of libyaml               |

### Other features

- **Configurable budgets:** Enforce input limits to mitigate resource-exhaustion
  (e.g., deeply nested structures or very large arrays); see
  [budget constraints](https://docs.rs/serde_yaml_bw/latest/serde_yaml_bw/budget/struct.Budget.html) and
  [`Budget`](https://docs.rs/serde-saphyr/latest/serde_saphyr/budget/struct.Budget.html).
- **Scope:** Currently the crate provides a **deserializer**. YAML merge keys are **not supported**.

### Duplicate keys

Duplicate key handling is configurable. By default it’s an error; “first wins”  and “last wins” strategies are available via
[`Options`](https://docs.rs/serde-saphyr/latest/serde_saphyr/struct.Options.html). Duplicate key policy applies not just to strings but also to other types (when deserializing into map).

### Unsuported features
- **Tagged enums** (`!!EnumName RED`) are not supported. Use mapping base enums (EnumName: RED). This allows to define nested enums if needed. 
- **Merge keys** (feature of YAML 1.1 but not 1.2) are not supported because it’s not feasible with the current streaming design to support YAML merge keys without buffering extra data. Use serde-yaml-bw for merge keys.

---

## Usage

Parse YAML into a Rust structure with proper error handling. The crate name on crates.io is
`serde-saphyr`, and the import path is `serde_saphyr`.

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
  name: String,
  enabled: bool,
  retries: i32,
}

fn main() {
let yaml_input = r#"
  name: "My Application"
  enabled: true
  retries: 5
...
"#;

    let config: Result<Config, _> = serde_saphyr::from_str(yaml_input);

    match config {
        Ok(parsed_config) => {
            println!("Parsed successfully: {:?}", parsed_config);
        }
        Err(e) => {
            eprintln!("Failed to parse YAML: {}", e);
        }
    }
}
```

## Nested enums

Externally tagged enums nest naturally in YAML as maps keyed by the variant name.
This enables strict, expressive models (enums with associated data) instead of generic maps.

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Move {
by: f32,
constraints: Vec<Constraint>,
}

#[derive(Deserialize)]
enum Constraint {
StayWithin { x: f32, y: f32, r: f32 },
MaxSpeed { v: f32 },
}

fn main() {
let yaml = r#"
- by: 10.0
  constraints:
    - StayWithin:
      x: 0.0
      y: 0.0
      r: 5.0
    - StayWithin:
      x: 4.0
      y: 0.0
      r: 5.0
    - MaxSpeed:
      v: 3.5
      "#;

  let robot_moves: Vec<Move> = serde_saphyr::from_str(yaml).unwrap();
  println!("Parsed {} moves", robot_moves.len());
  }
```

## Composite keys

YAML supports complex (non-string) mapping keys. Rust maps can mirror this, allowing you to parse such structures directly.

```rust
use serde::{Deserialize};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, Hash, Deserialize)]
struct Point {
  x: i32,
  y: i32
}

#[derive(Debug, PartialEq, Deserialize)]
struct Transform {
    // Transform between locations
    map: HashMap<Point, Point>,
}

fn main() {
let yaml = r#"
map:
  {x: 1, y: 2}: {x: 3, y: 4}
  {x: 5, y: 6}: {x: 7, y: 8}
"#;
let transform: Transform = serde_saphyr::from_str(yaml).unwrap();
println!("{} entries", transform.map.len());
}
```

## Binary scalars

`!!binary`-tagged YAML values are base64-decoded when deserializing into `Vec<u8>` or `String` (reporting error if it is not valid UTF-8)

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Blob {
    data: Vec<u8>,
}

fn parse_blob() {
    let blob: Blob = serde_saphyr::from_str("data: !!binary aGVsbG8=").unwrap();
    assert_eq!(blob.data, b"hello");
}
```

## Rust types as schema

The target Rust types act as a schema. Knowing whether a field is a string or a boolean allows the
parser to accept `1.2` as either a number or the string `"1.2"` depending on the target type, and to
interpret common YAML boolean shorthands like `y`, `on`, `n`, or `off` appropriately. Same way, `0x2A` is
the hexadecimal number when parsed into integer field and a string when parsed into `String`. 
Legacy octal format like `0052` can be turned on in `Options` but is off by default.

## Pathological inputs & budgets

Fuzzing shows that certain adversarial inputs can make YAML parsers consume excessive time or memory, enabling denial-of-service scenarios. To counter this, `serde-saphyr` offers a fast, configurable pre-check via a [`Budget`](https://docs.rs/serde-saphyr/latest/serde_saphyr/budget/struct.Budget.html),
available through [`Options`](https://docs.rs/serde-saphyr/latest/serde_saphyr/struct.Options.html).
The budget check uses a separate `saphyr-parser` run that avoids building a syntax tree and stops as soon as any resource limit is exceeded. Defaults are conservative; tighten them when you know your input shape, or disable the budget if you only parse YAML you generate yourself.

---

<!--
Notes for maintainers:
- The "budget constraints" link above intentionally matches the original URL set, which points to serde_yaml_bw.
- If desired, we can add sections on: feature flags, no_std compatibility, performance tips (zero-copy & borrowing),
  and guidance on sandboxing user-controlled inputs in larger systems.
-->
