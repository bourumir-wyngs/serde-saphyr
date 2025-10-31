use serde_saphyr::RcAnchor;
use std::rc::Rc;
use serde::{Deserialize, Serialize};

// Let's assume here we have megabytes of information about
// the city so we went to share it
#[derive(Clone, Serialize, Deserialize)]
struct City {
    name: String,
    population: usize,
}

#[derive(Serialize, Deserialize)]
struct Doc {
    trains: Vec<Vec<RcAnchor<City>>>,
}

fn city(name: &str, population: usize) -> RcAnchor<City> {
    RcAnchor::from(Rc::new(City {
        name: name.to_string(),
        population,
    }))
}

fn main() -> anyhow::Result<()> {
    let zurich = city("Zurich", 436000);
    let bern = city("Bern", 134000);
    let basel = city("Basel", 178000);

    let zurich_bern_shuttle = vec![zurich.clone(), bern.clone()];
    let three_city_express = vec![zurich.clone(), bern.clone(), basel.clone()];

    let doc = Doc { trains: vec![zurich_bern_shuttle, three_city_express] };

    let yaml = serde_saphyr::to_string(&doc)?;
    println!("{}", yaml);

    let deserialized_doc: Doc = serde_saphyr::from_str(&yaml)?;

    // Assert that the first city (Zurich) in both trains points to the same shared Rc value.
    let zurich_first = &deserialized_doc.trains[0][0].0; // first city of first train
    let zurich_second = &deserialized_doc.trains[1][0].0; // first city of second train
    assert!(Rc::ptr_eq(zurich_first, zurich_second), "Zurich entries are not the same Rc allocation");

    // Also assert the city data itself
    assert_eq!(zurich_first.name, "Zurich");
    assert_eq!(zurich_first.population, 436000);

    println!(
        "OK: {} trains, shared first city = {} (pop {})",
        deserialized_doc.trains.len(),
        zurich_first.name,
        zurich_first.population
    );

    Ok(())
}