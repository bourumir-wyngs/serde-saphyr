use serde::{Deserialize, Serialize};
use serde_saphyr::Quoted;

fn assert_serialize<T: Serialize>() {}
fn assert_deserialize<'de, T: Deserialize<'de>>() {}

fn main() {
    assert_serialize::<Quoted<i32>>();
    assert_serialize::<Quoted<f64>>();
    assert_serialize::<Quoted<Vec<String>>>();
    assert_deserialize::<Quoted<i32>>();
}
