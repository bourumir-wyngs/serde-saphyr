# serde-saphyr

![panic-free](https://img.shields.io/badge/panic--free-%E2%9C%94%EF%B8%8F-brightgreen)
[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/bourumir-wyngs/serde-saphyr/rust.yml)](https://github.com/bourumir-wyngs/serde-saphyr/actions)
[![crates.io](https://img.shields.io/crates/v/serde-saphyr.svg)](https://crates.io/crates/serde-saphyr)
[![crates.io](https://img.shields.io/crates/l/serde-saphyr.svg)](https://crates.io/crates/serde-saphyr)
[![crates.io](https://img.shields.io/crates/d/serde-saphyr.svg)](https://crates.io/crates/serde-saphyr)
[![docs.rs](https://docs.rs/serde-saphyr/badge.svg)](https://docs.rs/serde-saphyr)
[![Fuzz & Audit](https://github.com/bourumir-wyngs/serde-saphyr/actions/workflows/ci.yml/badge.svg)](https://github.com/bourumir-wyngs/serde-saphyr/actions/workflows/ci.yml)

**serde-saphyr** is a strongly typed YAML deserializer built on
[`saphyr-parser`](https://crates.io/crates/saphyr-parser). It aims to be **panic-free** on malformed input and to avoid `unsafe` code in library code. The crate deserializes YAML *directly into your Rust types* without constructing an intermediate tree of “abstract values.” 

### Why this approach?

- **Light on resources:** Having almost no intermediate data structures should result in more efficient parsing, especially if anchors are used only lightly.
- **Also simpler:** No code to support intermediate Values of all kinds.
- **Type-driven parsing:** YAML that doesn’t match the expected Rust types is rejected early.
- **Safer by construction:** No dynamic “any” objects; common YAML-based code-execution [exploits](https://www.arp242.net/yaml-config.html) do not apply.

### Project relationship
`serde-saphyr` is not a fork of the older [`serde-yaml`](https://crates.io/crates/serde_yaml) crate and shares no code with it (apart from some reused tests). It is also not part of the [`saphyr`](https://crates.io/crates/saphyr) project. The crate simply builds a Serde-based YAML deserialization layer **around** Saphyr’s public parser and is maintained independently. The name was historically chosen to reflect the use of Saphyr’s parser at a time when the Saphyr project did not provide its own Serde integration.

### Benchmarking

In our [benchmarking project](https://github.com/bourumir-wyngs/serde-saphyr-benchmark), we tested the following crates:

| Crate | Version | Merge Keys | Nested Enums | Duplicate key rejection | Notes |
|------:|:---------|:-----------|:--------------|:------------------------|:-------|
| [serde-saphyr](https://crates.io/crates/serde-saphyr) | 0.0.4 | ✅ Native | ✅ | ✅ Configurable          | No `unsafe`, no [unsafe-libyaml](https://crates.io/crates/unsafe-libyaml) |
| [serde-yaml-bw](https://crates.io/crates/serde-yaml_bw) | 2.4.1 | ✅ Native | ✅ | ✅ Configurable          | Slow due Saphyr doing budget check first upfront of libyaml |
| [serde-yaml-ng](https://crates.io/crates/serde-yaml-ng) | 0.10.0 | ⚠️ partial | ❌ | ❌                       |  |
| [serde-yaml](https://crates.io/crates/serde-yaml) | 0.9.34 + deprecated | ⚠️ partial | ❌ | ❌                       | Original, deprecated, repo archived |
| [serde-norway](https://crates.io/crates/serde-norway) | 0.9 | ⚠️ partial | ❌ | ❌                       |  |
| [serde-yml](https://crates.io/crates/serde-yml) | 0.0.12 | ⚠️ partial | ❌ | ❌                       | Repo archived |


Benchmarking was done with [Criterion](https://crates.io/crates/criterion), giving the following results:

<p align="center">
<img src="https://github.com/bourumir-wyngs/serde-saphyr-benchmark/blob/master/figures/yaml_parse/relative_vs_baseline.png?raw=true"
alt="Relative median time vs baseline"
width="60%">
</p>

As seen, serde-saphyr exceeds others by performance, even with budget check enabled. 

## Testing
The test suite currently includes 656 passing tests, most of them originating from the fully converted [yaml-test-suite](https://github.com/yaml/yaml-test-suite), with additional cases taken from the original serde-yaml tests. The remaining 18 failing corner cases (marked as ignored) have been reviewed, and their causes are well understood. To the best of our assessment, these failures stem from the saphyr parser. They represent extremely rare edge cases that are unlikely to appear in real-world use.

## Other features
- **Configurable budgets:** Enforce input limits to mitigate resource exhaustion (e.g., deeply nested structures or very large arrays); see [`Budget`](https://docs.rs/serde-saphyr/latest/serde_saphyr/budget/struct.Budget.html).
- **Serializer supports emitting anchors** (Rc, Arc, Weak) if they properly wrapped (see below).
- **serde_json::Value** is supported when parsing without target structure defined.
- **robotic extensions** to support YAML dialect common in robotics (see below).

### Duplicate keys
Duplicate key handling is configurable. By default it’s an error; “first wins”  and “last wins” strategies are available via [`Options`](https://docs.rs/serde-saphyr/latest/serde_saphyr/options/struct.Options.html). Duplicate key policy applies not just to strings but also to other types (when deserializing into map).

### Unsupported features
- **Tabs in indentation** YAML disallows tabs for indentation, including indentation of block scalar by tab.
- **Invalid indentation of the closing bracket.** The code like
```yaml
    key: [
    ]
```

For those who want to retain compatibility with serde-yaml, even where it might deviate from the standard, serde-yaml-bw can be better choice. This crate uses saphyr-parser for budget pre-check only when unsafe-libyaml later does the final parsing. 

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

## Multiple documents
YAML streams can contain several documents separated by `---`/`...` markers. When deserializing with [serde_saphyr::from_multiple](https://docs.rs/serde-saphyr/latest/serde_saphyr/fn.from_multiple.html)`, you still need to supply the vector element type up front (`Vec<T>`). That does **not** lock you into a single shape: make the element an enum and each document will deserialize into the matching variant. This lets you mix different payloads in one stream while retaining strong typing on the Rust side.

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
enum Document {
    #[serde(rename = "person")]
    Person { name: String, age: u8 },
    #[serde(rename = "pet")]
    Pet { kind: String },
}

fn main() {
    let input = r#"---
 person:
   name: Alice
   age: 30
---
 pet:
  kind: cat
---
 person:
   name: Bob
   age: 25
"#;
    let docs = serde_saphyr::from_multiple(input).expect("valid YAML stream");
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
There are two variants of the deserialization functions: from_* and from_*_with_options. The latter accepts an [Options](https://docs.rs/serde-saphyr/latest/serde_saphyr/options/struct.Options.html)
object that allows you to configure budget and other aspects of parsing. For larger projects that require consistent parsing behavior, we recommend defining a wrapper function so that all option and budget settings are managed in one place (see examples/wrapper_function.rs).

Tagged enums written as `!!EnumName VARIANT` are also supported, but only for single-level scalar variants. YAML itself cannot nest such tagged enums, so use mapping-based representations (`EnumName: RED`) if you need to embed enums within other enums.

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

## Booleans
By default, if the target field is boolean, serde-saphyr will attempt to interpret standard YAML 1.1 values as boolean (not just 'false' but also 'no', etc).
If you do not want this (or you are parsing into a JSON Value where it is wrongly inferred), enclose the value in quotes or set `strict_booleans` to true in [`Options`](https://docs.rs/serde-saphyr/latest/serde_saphyr/options/struct.Options.html).

## Deserializing into abstract JSON Value
If you must work with abstract types, you can also deserialize YAML into [`serde_json::Value`](https://docs.rs/serde_json/latest/serde_json/value/index.html). Serde will drive the process through [`deserialize_any`](src/de.rs) because `Value` does not fix a Rust primitive type ahead of time. You lose strict type control by Rust `struct` data types.

## Binary scalars
`!!binary`-tagged YAML values are base64-decoded when deserializing into `Vec<u8>` or `String` (reporting an error if it is not valid UTF-8)

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

## Merge keys
`serde-saphyr` supports merge keys, which reduce redundancy and verbosity by specifying shared key-value pairs once and then reusing them across multiple mappings. Here is an example with merge keys (inherited properties):

```rust
use serde::Deserialize;

/// Configuration to parse into. Does not include "defaults"
#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    development: Connection,
    production: Connection,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Connection {
    adapter: String,
    host: String,
    database: String,
}

fn main() {
    let yaml_input = r#"
# Here we define "default configuration"  
defaults: &defaults
  adapter: postgres
  host: localhost

development:
  <<: *defaults
  database: dev_db

production:
  <<: *defaults
  database: prod_db
"#;

    // Deserialize YAML with anchors, aliases and merge keys into the Config struct
    let parsed: Config = serde_saphyr::from_str(yaml_input).expect("Failed to deserialize YAML");

    // Define expected Config structure explicitly
    let expected = Config {
        development: Connection {
            adapter: "postgres".into(),
            host: "localhost".into(),
            database: "dev_db".into(),
        },
        production: Connection {
            adapter: "postgres".into(),
            host: "localhost".into(),
            database: "prod_db".into(),
        },
    };

    // Assert parsed config matches expected
    assert_eq!(parsed, expected);
}
```
Merge keys are standard in YAML 1.1. Although YAML 1.2 no longer includes merge keys in its specification, it doesn't explicitly disallow them either, and many parsers implement this feature.

## Rust types as schema

To address the “Norway problem,” the target Rust types serve as an explicit schema. Because the parser knows whether a field expects a string or a boolean, it can correctly accept `1.2` either as a number or as the string `"1.2"`, and interpret the common YAML boolean shorthands (`y`, `on`, `n`, `off`) as actual booleans when appropriate (can be disabled). Likewise, `0x2A` is parsed as a hexadecimal integer when the target field is numeric, and as a string when the target is `String`. As with [StrictYAML](https://hitchdev.com/strictyaml/why/implicit-typing-removed/), **serde-saphyr** avoids inferring types from values — one of the most heavily criticized aspects of YAML. The Rust type system already provides all the necessary schema information.

Schema based parsing can be disabled by setting `no_schema` to true in  [`Options`](https://docs.rs/serde-saphyr/latest/serde_saphyr/struct.Options.html). In this case all *unquoted* values that are parsed into strings, but can be understood as something else, are rejected. This can be used for enforcing compatibility with another YAML parser that reads the same content and requires this quoting. Default setting if false.

Legacy octal notation such as `0052` can be enabled via `Options`, but it is disabled by default.


## Pathological inputs & budgets

Fuzzing shows that certain adversarial inputs can make YAML parsers consume excessive time or memory, enabling denial-of-service scenarios. To counter this, `serde-saphyr` offers a fast, configurable pre-check via a [`Budget`](https://docs.rs/serde-saphyr/latest/serde_saphyr/budget/struct.Budget.html), available through [`Options`](https://docs.rs/serde-saphyr/latest/serde_saphyr/struct.Options.html). Defaults are conservative; tighten them when you know your input shape, or disable the budget if you only parse YAML you generate yourself.
During [reader](https://docs.rs/serde-saphyr/latest/serde_saphyr/fn.from_reader_with_options.html)-based deserialization, serde-saphyr does not buffer the entire payload; it parses incrementally, counting bytes and enforcing configured budgets. This design blocks denial-of-service attempts via excessively large inputs. When [streaming](https://docs.rs/serde-saphyr/latest/serde_saphyr/fn.read_with_options.html) from the reader through the iterator, other budget limits apply on a per-document basis, since such a reader may be expected to stream indefinitely. The total size of input is not limited in this case.
To find the typical budget requirements for you file, run the main() executable of this library, providing a YAML file path as the program parameter. You can also fetch the budget programmatically by registering a handle in Options.

## Serialization

```rust
use serde::Serialize;

#[derive(Serialize)]
struct User { name: String, active: bool }

let yaml = serde_saphyr::to_string(&User { name: "Ada".into(), active: true }).unwrap();
assert!(yaml.contains("name: Ada"));
```

#### Anchors (Rc/Arc/Weak)
Serde-saphyr can conceptually connect YAML anchors with Rust shared references (Rc, Weak and Arc). You need to use wrappers to activate this feature:

- `RcAnchor<T>` and `ArcAnchor<T>` emit anchors like `&a1` on first occurrence and may emit aliases `*a1` later.
- `RcWeakAnchor<T>` and `ArcWeakAnchor<T>` serialize a weak ref: if the strong pointer is gone, it becomes `null`.

```rust
     #[derive(Deserialize, Serialize)]
    struct Doc {
        a: RcAnchor<Node>,
        b: RcAnchor<Node>,
    }

    #[derive(Deserialize, Serialize)]
    struct Bigger {
        primary_a: RcAnchor<Node>,
        doc: Doc,
    }

    let the_a = RcAnchor::from(Rc::new(Node {
        name: "primary_a".to_string(),
    }));

    let data = Bigger {
        primary_a: the_a.clone(),
        doc: Doc {
            a: the_a.clone(),
            b: RcAnchor::from(Rc::new(Node {
                name: "the_b".to_string(),
            })),
        },
    };

    let serialized = serde_saphyr::to_string(&data)?;
    assert_eq!(serialized, String::from(
        indoc! {
            r#"primary_a: &a1
                  name: primary_a
                doc:
                  a: *a1
                  b: &a2
                    name: the_b
            "#}));

    let deserialized: Bigger = serde_saphyr::from_str(&serialized)?;

    assert_eq!(&deserialized.primary_a.name, &deserialized.doc.a.name);
    assert_eq!(&deserialized.doc.b.name, &data.doc.b.name);
    assert!(Rc::ptr_eq(&deserialized.primary_a.0, &deserialized.doc.a.0));

    Ok(())
}
```

When anchors are highly repetitive and also large, packing them into references can make YAML more human-readable. 

To support round trip, library can also deserialize into these anchor structures, this serialization is identity-preserving. A field or structure that is defined once and subsequently referenced will exist as a single instance in memory, with all anchor fields pointing to it. This is crucial when the topology of references itself constitutes important information to be transferred.

### Controlling text deserialization ###
To make long text blocks look tidy, you can use wrappers such as FoldString to enable YAML scalar presentation. You can also use Commented to append a comment string after a scalar or an anchor reference. For LitStr, the serializer will automatically pick between |- (string with no \n at the end), | (\n) and |+ (multiple \n's).
Empty maps are serialized as {} and empty lists as [] by default. This can be changed in SerializerOptions.

## Robotics ##
The feature-gated "robotics" capability enables parsing of YAML extensions commonly used in robotics ([ROS](https://www.ros.org/blog/why-ros/) These extensions support conversion functions (deg, rad) and simple mathematical expressions such as deg(180), rad(pi), 1 + 2*(3 - 4/5), or rad(pi/2). This capability is gated behind the [robotics] feature and is not enabled by default. Additionally, **angle_conversions** must be set to true in the [Options](https://docs.rs/serde-saphyr/latest/serde_saphyr/options/struct.Options.html). Just adding robotics feature is not sufficient to activate this mode of parsing. This parser is still just a simple expression calculator implemented directly in Rust, not a some hook to a language interpreter.

```yaml
rad_tag: !radians 0.15 # value in radians, stays in radians
deg_tag: !degrees 180 # value in degrees, converts to radians
expr_complex: 1 + 2*(3 - 4/5) # simple expressions supported
func_deg: deg(180) # value in degrees, converts to radians
func_rad: rad(pi) # value in radians (stays in radians)
hh_mm_secs: -0:30:30.5 # Time
longitude: !radians 8:32:53.2 # Nautical, ETH Zürich Main Building (8°32′53.2″ E)
```

```rust
let options = Options {
    angle_conversions: true, // enable robotics angle parsing
    .. Options::default()
};

let v: RoboFloats = from_str_with_options(yaml, options).expect("parse robotics YAML");
```
Safety hardening with this feature enabled include (maximal expression depth, maximal number of digits, strict underscore placement and fraction parsing limits to precision-relevant digit).
