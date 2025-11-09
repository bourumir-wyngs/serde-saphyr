use serde::Serialize;

use serde_saphyr::{to_string, FoldStr, LitStr};

#[test]
fn litstr_top_level() {
    let out = to_string(&LitStr("line 1\nline 2")).unwrap();
    assert_eq!(out, "|\n  line 1\n  line 2\n");
}

#[test]
fn litstr_as_map_value() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: LitStr<'a>,
    }
    let d = Doc { note: LitStr("a\nb") };
    let out = to_string(&d).unwrap();
    assert_eq!(out, "note: |\n  a\n  b\n");
}

#[test]
fn litstr_in_block_sequence_item() {
    let v = vec![LitStr("alpha\nbeta")];
    let out = to_string(&v).unwrap();
    assert_eq!(out, "- |\n  alpha\n  beta\n");
}

#[test]
fn foldstr_top_level() {
    let out = to_string(&FoldStr("line 1\nline 2")).unwrap();
    assert_eq!(out, ">\n  line 1\n  line 2\n");
}

#[test]
fn foldstr_as_map_value() {
    #[derive(Serialize)]
    struct Doc<'a> { note: FoldStr<'a> }
    let d = Doc { note: FoldStr("a\nb") };
    let out = to_string(&d).unwrap();
    assert_eq!(out, "note: >\n  a\n  b\n");
}

#[test]
fn foldstr_in_block_sequence_item() {
    let v = vec![FoldStr("alpha\nbeta")];
    let out = to_string(&v).unwrap();
    assert_eq!(out, "- >\n  alpha\n  beta\n");
}
