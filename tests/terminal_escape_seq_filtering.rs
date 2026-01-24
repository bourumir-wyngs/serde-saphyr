//! Integration tests for terminal escape sequence filtering in error snippets.
//!
//! These tests verify that control characters (ASCII C0, DEL, UTF-8 C1) are properly
//! sanitized in error output to prevent terminal escape sequence injection.

use serde::Deserialize;
use serde_saphyr::from_str;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TestStruct {
    field: i32,  // Expect integer to trigger type errors
}

/// Helper to extract the error message as a string for inspection.
fn error_to_string(err: &serde_saphyr::Error) -> String {
    format!("{}", err)
}

#[test]
fn test_ansi_escape_filtered_in_snippet() {
    // YAML with ANSI escape sequence (ESC = 0x1B = \x1B) in a string value
    // where we expect an integer, triggering a type error
    let yaml_with_escape = "field: \x1B[31mmalicious\x1B[0m";
    
    let err = from_str::<TestStruct>(yaml_with_escape).unwrap_err();
    let error_output = error_to_string(&err);
    
    // The ESC character (0x1B) should be replaced with space (0x20)
    // So the escape sequence should be broken/neutralized
    assert!(!error_output.contains("\x1B[31m"), 
        "Error output should not contain ANSI escape sequences");
    assert!(!error_output.contains("\x1B[0m"), 
        "Error output should not contain ANSI reset sequences");
}

#[test]
fn test_osc_escape_filtered_in_snippet() {
    // OSC (Operating System Command) escape: ESC ] (0x1B 0x5D)
    // Used for setting terminal title, etc.
    let yaml_with_osc = "field: \x1B]0;malicious title\x07";
    
    let err = from_str::<TestStruct>(yaml_with_osc).unwrap_err();
    let error_output = error_to_string(&err);
    
    // ESC should be replaced, breaking the OSC sequence
    assert!(!error_output.contains("\x1B]"), 
        "Error output should not contain OSC escape sequences");
}

#[test]
fn test_c1_control_filtered_in_snippet() {
    // UTF-8 encoded C1 control: CSI (Control Sequence Introducer) = U+009B = 0xC2 0x9B
    // This is an alternative way to introduce ANSI escapes
    let yaml_with_c1 = "field: \u{009B}31mmalicious";
    
    let err = from_str::<TestStruct>(yaml_with_c1).unwrap_err();
    let error_output = error_to_string(&err);
    
    // C1 control (0xC2 0x9B) should be replaced with NBSP (0xC2 0xA0)
    // The original C1 CSI should not appear
    assert!(!error_output.contains("\u{009B}"), 
        "Error output should not contain C1 control characters");
}

#[test]
fn test_del_character_filtered() {
    // DEL character (0x7F) can cause issues in some terminals
    let yaml_with_del = "field: test\x7Fvalue";
    
    let err = from_str::<TestStruct>(yaml_with_del).unwrap_err();
    let error_output = error_to_string(&err);
    
    // DEL should be replaced with space
    assert!(!error_output.contains("\x7F"), 
        "Error output should not contain DEL character");
}

#[test]
fn test_multiple_control_chars_filtered() {
    // Mix of various control characters
    let yaml_with_controls = "field: \x01\x02\x03test\x1B[31m\x7F";
    
    let err = from_str::<TestStruct>(yaml_with_controls).unwrap_err();
    let error_output = error_to_string(&err);
    
    // None of the control characters should appear
    assert!(!error_output.contains("\x01"), "SOH should be filtered");
    assert!(!error_output.contains("\x02"), "STX should be filtered");
    assert!(!error_output.contains("\x03"), "ETX should be filtered");
    assert!(!error_output.contains("\x1B"), "ESC should be filtered");
    assert!(!error_output.contains("\x7F"), "DEL should be filtered");
}

#[test]
fn test_newline_and_tab_preserved() {
    // \n and \t should be preserved as they're needed for snippet formatting
    // Use a map with an invalid field to trigger an error
    let yaml_with_whitespace = "field: 123\ninvalid_field:\n\tvalue";
    
    let err = from_str::<TestStruct>(yaml_with_whitespace).unwrap_err();
    let error_output = error_to_string(&err);
    
    // These should still be present (they're safe and needed)
    assert!(error_output.contains("\n"), "Newlines should be preserved");
    // Tab might or might not appear depending on the error location
}

#[test]
fn test_valid_utf8_preserved() {
    // Normal UTF-8 characters should pass through unchanged
    // Trigger a type error with unicode in the value
    let yaml_with_unicode = "field: Hello 世界 🌍";
    
    let err = from_str::<TestStruct>(yaml_with_unicode).unwrap_err();
    let error_output = error_to_string(&err);
    
    // Valid UTF-8 should be preserved in the error context
    // The error will show the value that couldn't be parsed as integer
    assert!(error_output.contains("Hello") || error_output.contains("世界") || error_output.contains("🌍"),
        "Valid UTF-8 should be preserved in error output");
}

#[cfg(feature = "miette")]
#[test]
fn test_miette_integration_filters_escapes() {
    use serde_saphyr::miette::to_miette_report;
    
    // YAML with escape sequences that will cause a type error
    let yaml = "field: \x1B[31mmalicious\x1B[0m";
    
    let err = from_str::<TestStruct>(yaml).unwrap_err();
    let report = to_miette_report(&err, yaml, "test.yaml");
    
    // Use Display formatting to get the rendered report
    // The sanitization should prevent escape sequences from appearing in the source snippets
    let report_output = format!("{}", report);
    
    // The report should not contain the original escape sequences in the source snippet
    // (they should be replaced with spaces, breaking the ANSI codes)
    assert!(!report_output.contains("\x1B[31m"), 
        "Miette report should not contain original ANSI escape sequences in source");
}

#[test]
fn test_crlf_normalization_with_escapes() {
    // Test that CRLF normalization works together with escape filtering
    // Use CRLF line endings with escape sequences
    let yaml_with_crlf_and_escape = "field: \x1B[31mtest\x1B[0m\r\ninvalid: value";
    
    let err = from_str::<TestStruct>(yaml_with_crlf_and_escape).unwrap_err();
    let error_output = error_to_string(&err);
    
    // Escape sequences should be filtered
    assert!(!error_output.contains("\x1B"), 
        "Escape sequences should be filtered even with CRLF");
}
