use serde::Deserialize;

// U9NS: Spec Example 2.8. Play by Play Feed from a Game (multiple documents)
// Two documents separated by explicit document end markers.

#[derive(Debug, Deserialize, PartialEq)]
struct Play {
    time: String,
    player: String,
    action: String,
}

#[test]
fn yaml_u9ns_multi_documents() {
    let y = "---\n
time: 20:03:20\nplayer: Sammy Sosa\naction: strike (miss)\n...\n---\n
time: 20:03:47\nplayer: Sammy Sosa\naction: grand slam\n...\n";

    let docs: Vec<Play> = serde_saphyr::from_multiple(y).expect("failed to parse U9NS");
    assert_eq!(docs.len(), 2);

    assert_eq!(docs[0].time, "20:03:20");
    assert_eq!(docs[0].player, "Sammy Sosa");
    assert_eq!(docs[0].action, "strike (miss)");

    assert_eq!(docs[1].time, "20:03:47");
    assert_eq!(docs[1].player, "Sammy Sosa");
    assert_eq!(docs[1].action, "grand slam");
}
