use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct StackEntry {
    file: String,
    line: i32,
    code: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct LogDoc {
    #[serde(rename = "Time")]
    time: Option<String>,
    #[serde(rename = "Date")]
    date: Option<String>,
    #[serde(rename = "User")]
    user: String,
    #[serde(rename = "Warning")]
    warning: Option<String>,
    #[serde(rename = "Fatal")]
    fatal: Option<String>,
    #[serde(rename = "Stack")]
    stack: Option<Vec<StackEntry>>,
}

#[test]
fn yaml_rzt7_spec_example_log_file() {
    let y = "---\nTime: 2001-11-23 15:01:42 -5\nUser: ed\nWarning:\n  This is an error message\n  for the log file\n---\nTime: 2001-11-23 15:02:31 -5\nUser: ed\nWarning:\n  A slightly different error\n  message.\n---\nDate: 2001-11-23 15:03:17 -5\nUser: ed\nFatal:\n  Unknown variable \"bar\"\nStack:\n  - file: TopClass.py\n    line: 23\n    code: |\n      x = MoreObject(\"345\\n\")\n  - file: MoreClass.py\n    line: 58\n    code: |-\n      foo = bar\n";

    let reader = std::io::Cursor::new(y.as_bytes());
    let docs: Vec<LogDoc> = serde_saphyr::read(reader)
        .map(|r| r.expect("unexpected parse error"))
        .collect();

    assert_eq!(docs.len(), 3);

    assert_eq!(
        docs[0],
        LogDoc {
            time: Some("2001-11-23 15:01:42 -5".into()),
            date: None,
            user: "ed".into(),
            warning: Some("This is an error message for the log file".into()),
            fatal: None,
            stack: None,
        }
    );

    assert_eq!(
        docs[1],
        LogDoc {
            time: Some("2001-11-23 15:02:31 -5".into()),
            date: None,
            user: "ed".into(),
            warning: Some("A slightly different error message.".into()),
            fatal: None,
            stack: None,
        }
    );

    assert_eq!(docs[2].time, None);
    assert_eq!(docs[2].date.as_deref(), Some("2001-11-23 15:03:17 -5"));
    assert_eq!(docs[2].user, "ed");
    assert_eq!(docs[2].fatal.as_deref(), Some("Unknown variable \"bar\""));

    let stack = docs[2].stack.as_ref().expect("missing stack");
    assert_eq!(stack.len(), 2);
    assert_eq!(stack[0].file, "TopClass.py");
    assert_eq!(stack[0].line, 23);
    assert_eq!(stack[0].code, "x = MoreObject(\"345\\n\")\n");
    assert_eq!(stack[1].file, "MoreClass.py");
    assert_eq!(stack[1].line, 58);
    assert_eq!(stack[1].code, "foo = bar");
}
