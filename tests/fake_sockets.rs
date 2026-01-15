#![allow(clippy::upper_case_acronyms)]

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[test]
fn test() {
    #[derive(Serialize, Deserialize)]
    pub struct Ivp4Platz {
        octets: [u8; 4],
    }

    #[derive(Serialize, Deserialize)]
    pub enum OcketPlatz {
        V4 { pi: Ivp4Platz },
    }

    #[derive(Serialize, Deserialize)]
    pub struct Listener {
        pub endpoint: Vec<Endpoint>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Endpoint {
        pub id: Option<u16>,
        pub tag: Option<Arc<str>>,
        pub item: EndpointItem,
    }

    #[derive(Serialize, Deserialize)]
    pub enum EndpointItem {
        PCT { platz: OcketPlatz },
        Unknown,
    }

    let ep = Listener {
        endpoint: vec![Endpoint {
            id: None,
            tag: None,
            item: EndpointItem::PCT {
                platz: OcketPlatz::V4 {
                    pi: Ivp4Platz {
                        octets: [127, 0, 0, 1],
                    },
                },
            },
        }],
    };

    let s = serde_saphyr::to_string(&ep).unwrap();

    // Round trip: deserialize back from string and assert values
    let ep2: Listener = serde_saphyr::from_str(s.as_str()).unwrap();

    // Assert endpoints
    assert_eq!(ep2.endpoint.len(), 1);
    let e0 = &ep2.endpoint[0];
    assert!(e0.id.is_none());
    assert!(e0.tag.is_none());
}
