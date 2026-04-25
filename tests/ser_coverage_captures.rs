use serde::Serialize;
use serde_saphyr::to_string;
use std::collections::BTreeMap;

#[derive(Serialize)]
#[serde(rename = "__yaml_anchor")]
struct AnchorPayload<T>(T, &'static str);

#[derive(Serialize)]
#[serde(rename = "__yaml_weak_anchor")]
struct WeakAnchorPayload<T>(usize, T, &'static str);

#[derive(Serialize)]
#[serde(rename = "__yaml_commented")]
struct CommentedPayload<T>(T, &'static str);

#[derive(Serialize)]
struct UnitStruct;

#[derive(Serialize)]
enum TestEnum {
    Unit,
    Newtype(u32),
    Tuple(u32, u32),
    Struct { x: u32 },
}

#[derive(Serialize)]
struct TupleStruct(u32, u32);

#[derive(Serialize)]
struct NormalStruct { x: u32 }

fn test_capture<T: Serialize>(val: T) {
    let _ = to_string(&AnchorPayload(&val, "x"));
    let _ = to_string(&WeakAnchorPayload(1, &val, "x"));
    let _ = to_string(&CommentedPayload(&val, "x"));
}

#[test]
fn test_all_captures_for_coverage() {
    test_capture(42i8);
    test_capture(42i16);
    test_capture(42i32);
    test_capture(42i64);
    test_capture(42u8);
    test_capture(42u16);
    test_capture(42u32);
    test_capture(42u64);
    test_capture(42u128);
    test_capture(42.0f32);
    test_capture(42.0f64);
    test_capture(true);
    test_capture(false);
    test_capture('c');
    test_capture("string");
    test_capture(b"bytes".as_slice());
    test_capture(None::<u32>);
    test_capture(Some(42u32));
    test_capture(());
    test_capture(UnitStruct);
    test_capture(TestEnum::Unit);
    test_capture(TestEnum::Newtype(42));
    test_capture(TestEnum::Tuple(1, 2));
    test_capture(TestEnum::Struct { x: 42 });
    test_capture(vec![1, 2, 3]);
    test_capture((1, 2));
    test_capture(TupleStruct(1, 2));
    let mut map = BTreeMap::new();
    map.insert("k", "v");
    test_capture(map);
    test_capture(NormalStruct { x: 42 });
}
