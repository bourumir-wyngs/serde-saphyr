// SYW4: Mapping with comments that should be ignored by the parser.

#[test]
fn y_syw4_mapping_scalars_to_scalars() {
    #[derive(Debug, serde::Deserialize)]
    struct Stats {
        hr: i64,
        avg: f64,
        rbi: i64,
    }

    let y = "hr:  65    # Home runs\navg: 0.278 # Batting average\nrbi: 147   # Runs Batted In\n";

    let stats: Stats = serde_saphyr::from_str(y).expect("Failed to parse mapping with comments");
    assert_eq!(stats.hr, 65);
    assert_eq!(stats.avg, 0.278);
    assert_eq!(stats.rbi, 147);
}
