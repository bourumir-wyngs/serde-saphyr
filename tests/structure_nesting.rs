// Put this in any Rust source file in your crate. It converts your snippet into a unit test
// that parses the YAML and prints both the debug view and the serialized output.

use serde_json::Value;

const DATA: &str = r#"
- name: adelie
  links:
    - type: PACKAGE_SOURCES
      aligned_a: null
    - type: PACKAGE_RECIPE
      aligned_b: null
    - type: PACKAGE_RECIPE_RAW
      aligned_c: null
  groups: [all]
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_serialized_structure() {
        /// Asserts that the lines containing `aligned_a`, `aligned_b`, and `aligned_c`
        /// all start at the same indentation (same number of leading whitespace chars)
        fn aligned_keys_have_same_indentation(data: &String) {
            let find_indent_for = |key: &str| -> (usize, usize) {
                data.lines()
                    .enumerate()
                    .find_map(|(i, line)| {
                        if line.contains(key) {
                            let indent = line.chars().take_while(|c| c.is_whitespace()).count();
                            Some((i + 1, indent))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| panic!("Did not find a line containing `{key}`"))
            };

            let (la, ia) = find_indent_for("aligned_a");
            let (lb, ib) = find_indent_for("aligned_b");
            let (lc, ic) = find_indent_for("aligned_c"); // matches even if the line has `aligned_c::`

            // All three must have identical indentation from the beginning of their lines.
            assert_eq!(
                ia, ib,
                "`aligned_a` (line {la}) and `aligned_b` (line {lb}) indentation differ: {ia} vs {ib}"
            );
            assert_eq!(
                ia, ic,
                "`aligned_a` (line {la}) and `aligned_c` (line {lc}) indentation differ: {ia} vs {ic}"
            );

            // Additionally, ensure each key is the first token after its indentation.
            for (ln, key) in [(la, "aligned_a"), (lb, "aligned_b"), (lc, "aligned_c")] {
                let line = DATA
                    .lines()
                    .find(|l| l.contains(key))
                    .expect("line should exist");
                let trimmed = line.trim_start();
                assert!(
                    trimmed.starts_with(key),
                    "`{key}` on line {ln} is not the first token after indentation: `{trimmed}`"
                );
            }
        }

        let v: Value = serde_saphyr::from_str(DATA).expect("YAML should parse into JSON Value");
        // Pretty debug view of the parsed structure:
        //eprintln!("{:#?}", v);

        // Serialize back to YAML using serde_saphyr and print it:
        let vv = serde_saphyr::to_string(&v).expect("Value should serialize back to YAML");
        //eprintln!("{}", vv);

        // Minimal assertion to keep the test meaningful:
        assert!(!vv.is_empty(), "Serialized YAML should not be empty");

        aligned_keys_have_same_indentation(&vv);
    }
}
