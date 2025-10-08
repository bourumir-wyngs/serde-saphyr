// 6VJK: Folded newlines preserved for more-indented and blank lines
#[test]
fn yaml_6vjk_folded_newlines_preserved() {
    let y = ">
 Sammy Sosa completed another
 fine season with great stats.

   63 Home Runs
   0.288 Batting Average

 What a year!
";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse 6VJK");
    assert_eq!(s, "Sammy Sosa completed another fine season with great stats.\n\n  63 Home Runs\n  0.288 Batting Average\n\nWhat a year!\n");
}
