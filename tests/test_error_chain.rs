#[cfg(feature = "include")]
use serde::Deserialize;
#[cfg(feature = "include")]
use serde_saphyr::{IncludeResolveError, InputSource, ResolvedInclude, Options, from_str_with_options};

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct Foo {
    bar: String,
}

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    foo: Foo,
}

#[cfg(feature = "include")]
#[test]
fn test_error_chain() {
    let yaml = "foo: !include foo.yaml\n";
    
    let options = Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        let s = req.spec;
        if s == "foo.yaml" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
                source: InputSource::Text("bar: !include bar.yaml\n".to_string())
            })
        } else if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
                source: InputSource::Text("]\n".to_string())
            })
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let res = from_str_with_options::<Config>(
        yaml,
        options,
    );
    
    println!("RESULT: {:?}", res);
    if let Err(e) = res {
        println!("ERROR MESSAGE:\n{}", e);
    }
}

#[cfg(feature = "include")]
#[test]
fn test_resolve_error_chain() {
    let yaml = "foo: !include missing.yaml\n";
    
    let options = Options::default().with_include_resolver(|_req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        Err(IncludeResolveError::Message("File not found".to_string()))
    });

    let res = from_str_with_options::<Config>(
        yaml,
        options,
    );
    
    if let Err(e) = res {
        println!("RESOLVE ERROR MESSAGE:\n{}", e);
    }
}
