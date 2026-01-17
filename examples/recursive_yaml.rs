use serde::{Deserialize, Serialize};
use serde_saphyr::{RcRecursion, RcRecursive};

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
"#;

    let kingdom = serde_saphyr::from_str::<Kingdom>(yaml)?;
    for king in &kingdom.kings {
        let ruler = king.borrow();

        let coronator = ruler
            .crowned_by
            .upgrade()
            .expect("each king has a coronator");
        let coronator_name = ruler
            .crowned_by
            .with(|next| next.birth_name.clone())
            .expect("k3 should be alive");
        assert_eq!(coronator_name, "Aurelian Markus");

        let k3_ref = coronator.borrow();
        assert_eq!(k3_ref.birth_name, "Aurelian Markus");
        assert_eq!(k3_ref.regal_name, "Aurelian I");
        let k3k3 = k3_ref
            .crowned_by
            .upgrade()
            .expect("coronator king always has coronator");

        let k3k3_ref = k3k3.borrow();
        assert_eq!(k3k3_ref.birth_name, "Aurelian Markus");
        assert_eq!(k3k3_ref.regal_name, "Aurelian I");
        let k3k3_name = k3k3_ref
            .crowned_by
            .with(|next| next.birth_name.clone())
            .expect("k3.k3 should be alive");
        assert_eq!(k3k3_name, "Aurelian Markus");
        // We have infinite recursion here, be careful with this.

        println!(
            "King {birth_name} ({from}), regal name {regal_name}, crowned by {coronator_name}, who was crowned by {coronator_coronator} ({coronator_from}).",
            regal_name = ruler.regal_name,
            birth_name = ruler.birth_name,
            from = ruler.reign_starts,
            coronator_name = coronator_name,
            coronator_coronator = k3k3_ref.birth_name,
            coronator_from = k3k3_ref.reign_starts
        )
    }
    Ok(())
}
