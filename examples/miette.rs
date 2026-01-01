fn main() {
    //   cargo run --example miette --features miette
    let yaml = "definitely\n";

    let err = serde_saphyr::from_str::<bool>(yaml).expect_err("bool parse error expected");
    let report = serde_saphyr::miette::to_miette_report(&err, yaml, "config.yaml");

    // `Debug` formatting uses miette's graphical reporter.
    eprintln!("{report:?}");

    // Show a garde validation error too.
    // cargo run --example miette --features "garde miette"
    #[cfg(feature = "garde")]
    {
        use serde::Deserialize;
        use garde::Validate;

        #[derive(Debug, Deserialize, Validate)]
        #[allow(dead_code)]
        struct Cfg {
            #[serde(rename = "firstString")]
            #[garde(skip)]
            first_string: String,

            #[serde(rename = "secondString")]
            #[garde(length(min = 2))]
            second_string: String,
        }

        // The second value is an alias to the first, so the error can label both:
        // - where the value is used (`secondString: *A`)
        // - where it is defined (`firstString: &A "x"`)
        let yaml = r#"
firstString: &A "x"
secondString: *A
"#;

        let err = serde_saphyr::from_str_valid::<Cfg>(yaml)
            .expect_err("validation error expected");
        let report = serde_saphyr::miette::to_miette_report(&err, yaml, "config.yaml");
        eprintln!("{report:?}");
    }
}
