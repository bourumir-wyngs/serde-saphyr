use serde::Deserialize;

// 565N: Construct Binary — test both !!binary inline and block forms decode to the same bytes
#[derive(Debug, Deserialize)]
struct BinDoc {
    canonical: Vec<u8>,
    generic: Vec<u8>,
    description: String,
}

#[test]
fn yaml_565n_construct_binary() {
    let y = concat!(
        "canonical: !!binary \"\
",
        " R0lGODlhDAAMAIQAAP//9/X17unp5WZmZgAAAOfn515eXvPz7Y6OjuDg4J+fn5\
",
        " OTk6enp56enmlpaWNjY6Ojo4SEhP/++f/++f/++f/++f/++f/++f/++f/++f/+\
",
        " +f/++f/++f/++f/++f/++SH+Dk1hZGUgd2l0aCBHSU1QACwAAAAADAAMAAAFLC\
",
        " AgjoEwnuNAFOhpEMTRiggcz4BNJHrv/zCFcLiwMWYNG84BwwEeECcgggoBADs=\"\n",
        "generic: !!binary |\n",
        " R0lGODlhDAAMAIQAAP//9/X17unp5WZmZgAAAOfn515eXvPz7Y6OjuDg4J+fn5\n",
        " OTk6enp56enmlpaWNjY6Ojo4SEhP/++f/++f/++f/++f/++f/++f/++f/++f/+\n",
        " +f/++f/++f/++f/++f/++SH+Dk1hZGUgd2l0aCBHSU1QACwAAAAADAAMAAAFLC\n",
        " AgjoEwnuNAFOhpEMTRiggcz4BNJHrv/zCFcLiwMWYNG84BwwEeECcgggoBADs=\n",
        "description: |\n",
        " The binary value above is a tiny arrow encoded as a gif image.\n",
    );

    let d: BinDoc = serde_saphyr::from_str(y).expect("failed to parse 565N");
    assert_eq!(d.canonical, d.generic);
    assert!(!d.canonical.is_empty());
    assert!(d.description.contains("tiny arrow"));
}
