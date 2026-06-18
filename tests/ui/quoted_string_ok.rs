use serde::{Deserialize, Serialize};
use serde_saphyr::{DoubleQuoted, SingleQuoted};

fn assert_serialize<T: Serialize>() {}
fn assert_deserialize<'de, T: Deserialize<'de>>() {}

fn main() {
    assert_serialize::<DoubleQuoted<String>>();
    assert_serialize::<DoubleQuoted<&'static str>>();
    assert_deserialize::<DoubleQuoted<String>>();
    assert_serialize::<SingleQuoted<String>>();
    assert_serialize::<SingleQuoted<&'static str>>();
    assert_deserialize::<SingleQuoted<String>>();
}
