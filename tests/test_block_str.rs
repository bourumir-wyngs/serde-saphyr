use serde::Serialize;

use serde_saphyr::{to_string, FoldStr, FoldString, LitStr, LitString, RcAnchor};

#[test]
fn litstr_top_level() {
    // String doesn't end with \n, so use strip indicator (|-)
    let out = to_string(&LitStr("line 1\nline 2")).unwrap();
    assert_eq!(out, "|-\n  line 1\n  line 2\n");
}

#[test]
fn litstr_as_map_value() {
    #[derive(Serialize)]
    struct Doc<'a> {
        note: LitStr<'a>,
    }
    let d = Doc {
        note: LitStr("a\nb"),
    };
    // String doesn't end with \n, so use strip indicator (|-)
    let out = to_string(&d).unwrap();
    assert_eq!(out, "note: |-\n  a\n  b\n");
}

#[test]
fn litstr_in_block_sequence_item() {
    // String doesn't end with \n, so use strip indicator (|-)
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
    // String doesn't end with \n, so use strip indicator (|-)
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
    // String doesn't end with \n, so use strip indicator (|-)
    let out = to_string(&d).unwrap();
    assert_eq!(out, "note: |-\n  a\n  b\n");
}

#[test]
fn lit_string_in_block_sequence_item() {
    // String doesn't end with \n, so use strip indicator (|-)
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
    //println!("{}", yaml);

    assert!(yaml.contains("  00This is"), "Must be indentation 1");
    assert!(yaml.contains("    01This is"), "Must be indentation 2");
    assert!(yaml.contains("      02This is"), "Must be indentation 3");
    Ok(())
}
