#![no_main]

use std::collections::BTreeMap;

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use serde::{Deserialize, Serialize};

const MAX_DEPTH: u32 = 3;
const MAX_BREADTH: usize = 4;

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

fn gen_node(u: &mut Unstructured, depth: u32) -> arbitrary::Result<Node> {
    // At depth 0, only scalar variants are allowed
    let max_variant: u32 = if depth == 0 { 3 } else { 7 };
    Ok(match u.int_in_range(0..=max_variant)? {
        0 => Node::Null,
        1 => Node::Bool(bool::arbitrary(u)?),
        2 => Node::Int(i64::arbitrary(u)?),
        3 => {
            let s = String::arbitrary(u)?;
            // TODO: We need to trim here because trailing newlines and spaces are intentionally
            // not preserved by the serializer according to `is_auto_block_scalar_readable`
            Node::Str(s.trim().to_string())
        }
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
                let k = String::arbitrary(u)?;
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

fuzz_target!(|node: Node| {
    let opts = serde_saphyr::ser_options! {};
    // serialization is panic-free and from valid input, so may never return an `Err`
    let text = match serde_saphyr::to_string_with_options(&node, opts) {
        Ok(text) => text,
        Err(e) => panic!("{e}"),
    };

    // anything we emit must be parseable YAML for this model.
    let _: Node = match serde_saphyr::from_str(&text) {
        Ok(back) => back,
        Err(e) => {
            panic!(
                "serializer emitted YAML that fails to parse back:\n---error---\n{e}\n--- yaml ---\n{text}\n---debug---\n{node:#?}"
            )
        }
    };

    // todo: equality, roundtrip, indepotance
});
