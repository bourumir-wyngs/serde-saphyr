use serde::Deserialize;
#[cfg(feature = "include")]
use serde_saphyr::{IncludeResolveError, InputSource, ResolvedInclude, Options, from_str_with_options};

#[derive(Debug, Deserialize, PartialEq)]
struct Foo {
    bar: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    foo: Foo,
}

#[test]
fn test_error_chain() {
    let yaml = "foo: !include foo.yaml\n";
    
    let options = Options::default().with_include_resolver(|s: std::borrow::Cow<str>| -> Result<ResolvedInclude, IncludeResolveError> {
        if s == "foo.yaml" {
            Ok(ResolvedInclude {
                id: s.clone().into_owned(),
                name: s.into_owned(),
                source: InputSource::Text("bar: !include bar.yaml\n".to_string())
            })
        } else if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.clone().into_owned(),
                name: s.into_owned(),
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

#[test]
fn test_resolve_error_chain() {
    let yaml = "foo: !include missing.yaml\n";
    
    let options = Options::default().with_include_resolver(|_s: std::borrow::Cow<str>| -> Result<ResolvedInclude, IncludeResolveError> {
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
