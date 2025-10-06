use serde_saphyr::{from_str, Value};

#[test]
fn test_error_includes_location_in_formats() {
    // YAML starting with a literal block followed by invalid character to cause an error
    let yaml = ">\n@";
    let err = from_str::<Value>(yaml).unwrap_err();
    let loc = err.location().expect("location not available");
    assert_eq!(loc.line(), 2);
    assert_eq!(loc.column(), 1);

    let display = format!("{}", err);
    let debug = format!("{:?}", err);
    let pos_display = format!("line {} column {}", loc.line(), loc.column());
    assert!(display.contains(&pos_display), "Display output missing location: {display}");

    let pos_debug = format!("line: {}, column: {}", loc.line(), loc.column());
    assert!(debug.contains(&pos_debug), "Debug output missing location: {debug}");
}
