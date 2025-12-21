use serde::Serialize;

use serde_saphyr::{to_string, FoldStr, FoldString, LitStr, LitString, RcAnchor};

#[test]
fn litstr_top_level() {
    let out = to_string(&LitStr("line 1\nline 2")).unwrap();
    assert_eq!(out, "|-\n  line 1\n  line 2\n");
}

#[test]
fn litstr_no_trailing_newline() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: LitStr<'a>,
        other: usize
    }
    let d = Doc {
        note: LitStr("a\nb"), other: 0
    };
    let out = to_string(&d).unwrap();
    assert_eq!(out, "note: |-\n  a\n  b\nother: 0\n");
}

#[test]
fn litstr_trailing_newline() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: LitStr<'a>,
        other: usize
    }
    let d = Doc { note: LitStr("hello\nworld\n"), other: 0 };
    let out = to_string(&d).unwrap();
    assert_eq!(out, "note: |\n  hello\n  world\nother: 0\n");
}

#[test]
fn litstr_empty_string() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: LitStr<'a>,
        other: usize,
    }

    let d = Doc {
        note: LitStr(""),
        other: 0,
    };
    let out = to_string(&d).unwrap();

    // Empty string encoded as an empty literal block with strip chomping.
    // Value: ""
    assert_eq!(out, "note: |-\nother: 0\n");
}

#[test]
fn litstr_only_newline() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: LitStr<'a>,
        other: usize,
    }

    let d = Doc {
        note: LitStr("\n"),
        other: 0,
    };
    let out = to_string(&d).unwrap();

    // One empty content line, clip chomping.
    // Value: "\n"
    assert_eq!(out, "note: |\n  \nother: 0\n");
}

#[test]
fn litstr_single_line_no_trailing_newline() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: LitStr<'a>,
        other: usize,
    }

    let d = Doc {
        note: LitStr("hello"),
        other: 0,
    };
    let out = to_string(&d).unwrap();

    // Single line, no trailing '\n' → strip chomping.
    // Value: "hello"
    assert_eq!(out, "note: |-\n  hello\nother: 0\n");
}

#[test]
fn litstr_single_line_trailing_newline() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: LitStr<'a>,
        other: usize,
    }

    let d = Doc {
        note: LitStr("hello\n"),
        other: 0,
    };
    let out = to_string(&d).unwrap();

    // Single line, one trailing '\n' → clip chomping.
    // Value: "hello\n"
    assert_eq!(out, "note: |\n  hello\nother: 0\n");
}

#[test]
fn litstr_two_trailing_newlines() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: LitStr<'a>,
        other: usize,
    }

    let d = Doc {
        note: LitStr("a\nb\n\n"),
        other: 0,
    };
    let out = to_string(&d).unwrap();

    // Content lines: "a", "b", "" plus keep chomping.
    // Value: "a\nb\n\n"
    assert_eq!(out, "note: |+\n  a\n  b\n  \nother: 0\n");
}

#[test]
fn litstr_inner_blank_line_and_trailing_newline() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: LitStr<'a>,
        other: usize,
    }

    let d = Doc {
        note: LitStr("a\n\nb\n"),
        other: 0,
    };
    let out = to_string(&d).unwrap();

    // Inner blank line must be preserved as content; one trailing '\n'.
    // Value: "a\n\nb\n"
    assert_eq!(out, "note: |\n  a\n  \n  b\nother: 0\n");
}

#[test]
fn litstr_in_block_sequence_item() {
    let v = vec![LitStr("alpha\nbeta")];
    let out = to_string(&v).unwrap();
    assert_eq!(out, "- |-\n  alpha\n  beta\n");
}

#[test]
fn foldstr_top_level() {
    let out = to_string(&FoldStr("line 1\nline 2")).unwrap();
    assert_eq!(out, ">\n  line 1\n  line 2\n");
}

#[test]
fn foldstr_as_map_value() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: FoldStr<'a>,
    }
    let d = Doc {
        note: FoldStr("a\nb"),
    };
    let out = to_string(&d).unwrap();
    assert_eq!(out, "note: >\n  a\n  b\n");
}

#[test]
fn foldstr_in_block_sequence_item() {
    let v = vec![FoldStr("alpha\nbeta")];
    let out = to_string(&v).unwrap();
    assert_eq!(out, "- >\n  alpha\n  beta\n");
}

#[test]
fn lit_string_top_level() {
    let out = to_string(&LitString("line 1\nline 2".to_string())).unwrap();
    assert_eq!(out, "|-\n  line 1\n  line 2\n");
}

#[test]
fn lit_string_as_map_value() {
    #[derive(Serialize)]
    struct Doc {
        note: LitString,
    }
    let d = Doc {
        note: LitString("a\nb".to_string()),
    };
    let out = to_string(&d).unwrap();
    assert_eq!(out, "note: |-\n  a\n  b\n");
}

#[test]
fn lit_string_in_block_sequence_item() {
    let v = vec![LitString("alpha\nbeta".to_string())];
    let out = to_string(&v).unwrap();
    assert_eq!(out, "- |-\n  alpha\n  beta\n");
}

#[test]
fn fold_string_top_level() {
    let out = to_string(&FoldString("line 1\nline 2".to_string())).unwrap();
    assert_eq!(out, ">\n  line 1\n  line 2\n");
}

#[test]
fn fold_string_as_map_value() {
    #[derive(Serialize)]
    struct Doc {
        note: FoldString,
    }
    let d = Doc {
        note: FoldString("a\nb".to_string()),
    };
    let out = to_string(&d).unwrap();
    assert_eq!(out, "note: >\n  a\n  b\n");
}

#[test]
fn fold_string_in_block_sequence_item() {
    let v = vec![FoldString("alpha\nbeta".to_string())];
    let out = to_string(&v).unwrap();
    assert_eq!(out, "- >\n  alpha\n  beta\n");
}

#[test]
fn verdanta_case_fold() -> anyhow::Result<()> {
    #[derive(Debug, Serialize, Clone)]
    pub struct Node2 {
        /// Id of this node, used in annotations.
        #[serde(skip)]
        #[allow(dead_code)]
        pub id: usize,

        /// Name of the node, this can be arbitrary string.
        pub name: String,

        /// Longer description of this node
        #[serde(skip_serializing_if = "Option::is_none")]
        pub description: RcAnchor<Option<FoldString>>,

        /// Children of this node.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub children: Vec<RcAnchor<Node2>>,
    }

    let node = Node2 {
        id: 0,
        name: "name".to_string(),
        description: RcAnchor::wrapping(Some(FoldString(
            "00This is very very very long description. \
        This is very very very long description. This is very very very long description."
                .to_string(),
        ))),
        children: vec![RcAnchor::wrapping(Node2 {
            id: 0,
            name: "child".to_string(),
            description: RcAnchor::wrapping(Some(FoldString(
                "01This is very very very long description. \
        This is very very very long description. This is very very very long description."
                    .to_string(),
            ))),
            children: vec![RcAnchor::wrapping(Node2 {
                id: 0,
                name: "".to_string(),
                description: RcAnchor::wrapping(Some(FoldString(
                    "02This is very very very long description. \
        This is very very very long description. This is very very very long description."
                        .to_string(),
                ))),
                children: vec![],
            })],
        })],
    };

    let object = RcAnchor::wrapping(node);

    let yaml = to_string(&object)?;
    println!("{}", yaml);

    // Block scalar bodies must be indented deeper than the `description: >` header.
    assert!(yaml.contains("description: >\n  00This is"), "Top-level description body must be indented");
    assert!(
        yaml.contains("    description: >\n      01This is"),
        "Nested description body must be indented deeper than its header"
    );
    assert!(
        yaml.contains("        description: >\n          02This is"),
        "Deeply nested description body must be indented deeper than its header"
    );
    Ok(())
}
