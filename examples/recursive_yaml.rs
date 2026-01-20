use serde::{Deserialize, Serialize};
use serde_saphyr::{RcRecursion, RcRecursive};
use std::cell::Ref;

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct King {
    reign_starts: usize,
    birth_name: String,
    regal_name: String,
    // there have been several notable cases where rulers crowned themselves
    crowned_by: RcRecursion<King>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct Kingdom {
    kings: Vec<RcRecursive<King>>,
}

fn main() -> anyhow::Result<()> {
    let yaml = r#"
kings:
  - &markus
    reign_starts: 1920
    birth_name: "Aurelian Markus"
    regal_name: "Aurelian I"
    crowned_by: *markus   # self-crowned (recursive reference)

  - &orlan
    reign_starts: 1950
    birth_name: "Benedict Orlan"
    regal_name: "Benedict I"
    crowned_by: *markus   # crowned by the self-crowned predecessor

  - &valerius
    reign_starts: 1978
    birth_name: "Cassian Valerius"
    regal_name: "Cassian I"
    crowned_by: *orlan

  - &severin
    reign_starts: 2009
    birth_name: "Lucian Severin"
    regal_name: "Lucian I"
    crowned_by: *valerius

  - &octavian
    reign_starts: 2036
    birth_name: "Darius Octavian"
    regal_name: "Darius I"
    crowned_by: *severin

  - &faelan
    reign_starts: 2064
    birth_name: "Marcus Faelan"
    regal_name: "Marcus II"
    crowned_by: *octavian
"#;

    let kingdom = serde_saphyr::from_str::<Kingdom>(yaml)?;
    for king in &kingdom.kings {
        // We cannot just call "borrow" after .expect(..), we need to retain this variable.
        // Some optional type declarations added for clarity.
        let rc_coronator: RcRecursive<King> = king
            .borrow()
            .crowned_by
            .upgrade()
            .expect("each king has a coronator");

        // Fully access the coronator.
        let coronator: Ref<King> = rc_coronator.borrow();

        // Check also who coronated the coronator. For Aurelian, it will be the same king.
        let rc_coronator_of_coronator: RcRecursive<King> = coronator
            .crowned_by
            .upgrade()
            .expect("coronator king always has coronator");

        // Fully access the coronator of coronator
        let coronator_of_coronator = rc_coronator_of_coronator.borrow();

        // Go one step more (coronator of coronator of coronator). In our case,
        let k3k3_name = coronator_of_coronator
            .crowned_by
            .with(|next| next.birth_name.clone())
            .expect("k3.k3 should be alive");
        // We have infinite recursion here, be careful with this.

        println!(
            "{from}: king {regal_name} ({birth_name}), crowned by {coronator_name}, \
            crowned by {coronator_coronator} ({coronator_from}) crowned by {k3k3_name} .",
            regal_name = king.borrow().regal_name,
            birth_name = king.borrow().birth_name,
            from = king.borrow().reign_starts,
            coronator_name = coronator.birth_name,
            coronator_coronator = coronator_of_coronator.birth_name,
            coronator_from = coronator_of_coronator.reign_starts,
            k3k3_name = k3k3_name,
        )
        // Output:
        // 1920: king Aurelian I (Aurelian Markus), crowned by Aurelian Markus, crowned by Aurelian Markus (1920) crowned by Aurelian Markus .
        // 1950: king Benedict I (Benedict Orlan), crowned by Aurelian Markus, crowned by Aurelian Markus (1920) crowned by Aurelian Markus .
        // 1978: king Cassian I (Cassian Valerius), crowned by Benedict Orlan, crowned by Aurelian Markus (1920) crowned by Aurelian Markus .
        // 2009: king Lucian I (Lucian Severin), crowned by Cassian Valerius, crowned by Benedict Orlan (1950) crowned by Aurelian Markus .
        // 2036: king Darius I (Darius Octavian), crowned by Lucian Severin, crowned by Cassian Valerius (1978) crowned by Benedict Orlan .
        // 2064: king Marcus II (Marcus Faelan), crowned by Darius Octavian, crowned by Lucian Severin (2009) crowned by Cassian Valerius .
    }
    Ok(())
}
