use serde::{Deserialize, Serialize};
use serde_saphyr::Quoted;

fn assert_serialize<T: Serialize>() {}
fn assert_deserialize<'de, T: Deserialize<'de>>() {}

fn main() {
    assert_serialize::<Quoted<String>>();
    assert_serialize::<Quoted<&'static str>>();
    assert_deserialize::<Quoted<String>>();
}
