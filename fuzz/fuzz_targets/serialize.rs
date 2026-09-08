#![cfg_attr(not(test), no_main)]

use std::collections::BTreeMap;

use arbitrary::{Arbitrary, Unstructured};
#[cfg(not(test))]
use libfuzzer_sys::fuzz_target;
use serde::{Deserialize, Serialize};
use serde_saphyr::{DoubleQuoted, FlowMap, FlowSeq, LitString, SerializerOptions};

const MAX_INPUT_BYTES: usize = 16 * 1024;
const MAX_STRING_BYTES: usize = 512;
const MAX_DEPTH: u32 = 3;
const MAX_BREADTH: usize = 4;

// Give mutations short routes to quoting, chomping, Unicode escaping, and
// YAML-looking string keys. Arbitrary strings alone rarely reach these cases.
const STRINGS: &[&str] = &[
    "",
    " ",
    "\t",
    "\n",
    "\n\n",
    " leading",
    "trailing ",
    "line one \n  line two\t\n\n",
    "\r\n",
    "\0\u{7}\u{8}\u{b}\u{c}\u{1b}\u{7f}\u{9f}",
    "\u{85}\u{a0}\u{2028}\u{2029}\u{feff}",
    "\u{fffe}\u{ffff}",
    "\u{10000}\u{10ffff}",
    "'\"\\",
    "null",
    "~",
    "true",
    "yes",
    "0x10",
    ".nan",
    "<<",
    "---\n...\n",
    "a: b # comment",
    "[a, {b: c}]",
];

/// A small, recursive data model covering the YAML node kinds
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
enum Node {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    EnumTuple(Box<Node>, Box<Node>),
    EnumStruct { field: Box<Node> },
    Seq(Vec<Node>),
    Map(BTreeMap<String, Node>),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct StringStyles {
    literal: LitString,
    quoted: DoubleQuoted<String>,
    sequence: FlowSeq<Vec<String>>,
    mapping: FlowMap<BTreeMap<String, String>>,
}

fn gen_string(u: &mut Unstructured<'_>) -> arbitrary::Result<String> {
    let style = u.int_in_range(0..=3)?;
    if style == 0 {
        return Ok((*u.choose(STRINGS)?).to_owned());
    }

    let mut value = String::arbitrary(u)?;
    // Truncate only to bound work, preserving all whitespace and valid UTF-8.
    if value.len() > MAX_STRING_BYTES {
        let mut end = MAX_STRING_BYTES;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
    }
    match style {
        2 => value = format!("  {value} \t\n\n"),
        3 => value.push_str(u.choose(STRINGS)?),
        _ => {}
    }
    Ok(value)
}

/// Generate a map key which occasionally is overly-long to ensure `? key` paths are triggered.
fn gen_key(u: &mut Unstructured<'_>) -> arbitrary::Result<String> {
    let key = gen_string(u)?;
    if u.ratio(1, 8)? {
        let seed = if key.is_empty() { "k" } else { key.as_str() };
        // YAML's simple-key limit is measured in characters, not UTF-8 bytes.
        Ok(seed.repeat(1024 / seed.chars().count() + 2))
    } else {
        Ok(key)
    }
}

fn gen_node(u: &mut Unstructured<'_>, depth: u32) -> arbitrary::Result<Node> {
    // At depth 0, only scalar variants are allowed
    let max_variant: u32 = if depth == 0 { 3 } else { 7 };
    Ok(match u.int_in_range(0..=max_variant)? {
        0 => Node::Null,
        1 => Node::Bool(bool::arbitrary(u)?),
        2 => Node::Int(i64::arbitrary(u)?),
        3 => Node::Str(gen_string(u)?),
        4 => Node::EnumTuple(
            Box::new(gen_node(u, depth - 1)?),
            Box::new(gen_node(u, depth - 1)?),
        ),
        5 => Node::EnumStruct {
            field: Box::new(gen_node(u, depth - 1)?),
        },
        6 => {
            let n = u.int_in_range(0..=MAX_BREADTH)?;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(gen_node(u, depth - 1)?);
            }
            Node::Seq(v)
        }
        _ => {
            let n = u.int_in_range(0..=MAX_BREADTH)?;
            let mut m = BTreeMap::new();
            for _ in 0..n {
                let k = gen_key(u)?;
                let val = gen_node(u, depth - 1)?;
                m.insert(k, val);
            }
            Node::Map(m)
        }
    })
}

impl<'a> Arbitrary<'a> for Node {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        gen_node(u, MAX_DEPTH)
    }
}

fn roundtrip<T>(value: &T, opts: &SerializerOptions)
where
    T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let text = serde_saphyr::to_string_with_options(value, opts.clone())
        .expect("bounded values with valid serializer settings must serialize");

    // These bounded, alias-free values fit the default deserialization budget.
    // Do not silently discard errors: even a budget error for this small output
    // is unexpected. Explicit FoldString is deliberately excluded because its
    // documented folding semantics can change the original string's newlines.
    let back: T = match serde_saphyr::from_str(&text) {
        Ok(back) => back,
        Err(e) => {
            panic!(
                "serializer emitted YAML that fails to parse back:\n---error---\n{e}\n--- yaml ---\n{text}\n--- value ---\n{value:#?}\n--- options ---\n{opts:#?}"
            )
        }
    };

    // equality
    assert_eq!(
        value, &back,
        "round-trip changed the value\n--- yaml ---\n{text}\n--- options ---\n{opts:#?}"
    );

    // idempotence
    let text2 = serde_saphyr::to_string_with_options(&back, opts.clone()).unwrap();
    assert_eq!(
        text, text2,
        "serialization is not idempotent\n--- first ---\n{text}\n--- second ---\n{text2}\n--- value ---\n{value:#?}\n--- options ---\n{opts:#?}"
    );
}

fn check_input(data: &[u8]) {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    // Plain UTF-8 seed files exercise strings directly; no binary header or
    // successful recursive Arbitrary decoding is needed to reach assertions.
    let value = String::from_utf8_lossy(data).into_owned();
    let styles = StringStyles {
        literal: LitString(value.clone()),
        quoted: DoubleQuoted(value.clone()),
        sequence: FlowSeq(vec![value.clone(), "after".to_owned()]),
        mapping: FlowMap(BTreeMap::from([(value.clone(), value.clone())])),
    };
    let node = Node::arbitrary(&mut Unstructured::new(data)).ok();

    let flags = data.first().copied().unwrap_or(0);
    let width = data.get(1).copied().unwrap_or(0) as usize;
    let threshold = data.get(2).copied().unwrap_or(0) as usize;
    let varied = serde_saphyr::ser_options! {
        indent_step: [1, 2, 4, 8, 9, 10, 64][usize::from(flags) % 7],
        compact_list_indent: flags & 1 != 0,
        prefer_block_scalars: flags & 2 != 0,
        quote_all: flags & 4 != 0,
        tagged_enums: flags & 8 != 0,
        yaml_12: flags & 16 != 0,
        folded_wrap_chars: [1, 2, 8, 31, 32, 79, 80, 81][width % 8],
        min_fold_chars: [0, 1, 31, 32, 33, 64][threshold % 6],
    };
    // Keep empty_as_braces enabled: disabling it intentionally represents empty
    // collections as null and therefore is not an exact round-trip oracle.
    for opts in [SerializerOptions::default(), varied] {
        roundtrip(&value, &opts);
        roundtrip(&styles, &opts);
        if let Some(node) = &node {
            roundtrip(node, &opts);
        }
    }
}

#[cfg(not(test))]
fuzz_target!(|data: &[u8]| check_input(data));

#[cfg(test)]
mod tests {
    use super::check_input;

    #[test]
    fn enum_indentation_original_input() {
        // crash-c22b1997aa76cea333b69ac9e77d1e4a3a5d6e69, retained byte-for-byte.
        check_input(include_bytes!("../seeds/serialize/enum_indent.txt"));
    }

    #[test]
    fn enum_indentation_minimized_inputs() {
        check_input(b"F%%");
        // Decoded fuzz/reproducers/serialize_enum_indent.hex.
        check_input(&[0x1c, 0x65]);
    }
}
