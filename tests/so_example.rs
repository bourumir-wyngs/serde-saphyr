use serde::{Deserialize, Serialize};
use std::rc::Rc;
use serde_saphyr::RcAnchor;

#[derive(Serialize, Deserialize, Clone)]
struct Node {
    name: String,
}

#[test]
fn test_so_example() {
    let n1 = RcAnchor(Rc::new(Node {
        name: "node one".to_string(),
    }));

    let n2 = RcAnchor(Rc::new(Node {
        name: "node two".to_string(),
    }));

    let data = vec![n1.clone(), n1.clone(), n1.clone(), n2.clone(), n1.clone(), n2.clone()];
    let serialized = serde_saphyr::to_string(&data).expect("Must serialize");
    // println!("{}", serialized);

    let deserialized: Vec<RcAnchor<Node>> = serde_saphyr::from_str(&serialized).unwrap();

    let first: Rc<Node> = deserialized[0].0.clone(); // first reference is n1
    let second: Rc<Node> = deserialized[1].0.clone(); // second reference is also n1

    // they point to the same object
    assert!(std::rc::Rc::ptr_eq(&first, &second));
}


