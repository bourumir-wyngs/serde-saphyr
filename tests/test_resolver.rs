#[cfg(feature = "include")]
use serde::Deserialize;
#[cfg(feature = "include")]
use std::cell::RefCell;
#[cfg(feature = "include")]
use std::rc::Rc;
#[cfg(feature = "include")]
use serde_saphyr::{IncludeResolveError, InputSource, ResolvedInclude};
#[cfg(feature = "include")]
use serde_saphyr::Options;

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    foo: String,
}

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct NestedConfig {
    foo: NestedFoo,
}

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct NestedFoo {
    bar: String,
}

#[cfg(feature = "include")]
#[derive(Debug, Deserialize)]
struct QuotaConfig {
    pad: u32,
    inc: std::collections::BTreeMap<String, String>,
}

#[cfg(feature = "include")]
#[test]
fn test_reader_resolver() {
    let yaml = "foo: !include bar.yaml\n";
    let cursor = std::io::Cursor::new(yaml.as_bytes());
    
    let options = Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        let s = req.spec;
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
                source: InputSource::Text("bar_value\n".to_string())
            })
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let config: Config = serde_saphyr::from_reader_with_options(
        cursor,
        options,
    ).unwrap();
    
    assert_eq!(config.foo, "bar_value");
}

#[cfg(feature = "include")]
#[test]
fn test_str_resolver() {
    let yaml = "foo: !include bar.yaml\n";
    
    let options = Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        let s = req.spec;
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
                source: InputSource::Text("bar_value\n".to_string())
            })
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let config: Config = serde_saphyr::from_str_with_options(
        yaml,
        options,
    ).unwrap();
    
    assert_eq!(config.foo, "bar_value");
}

#[cfg(feature = "include")]
#[test]
fn test_slice_resolver() {
    let yaml = b"foo: !include bar.yaml\n";
    
    let options = Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        let s = req.spec;
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
                source: InputSource::Text("bar_value\n".to_string())
            })
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let config: Config = serde_saphyr::from_slice_with_options(
        yaml,
        options,
    ).unwrap();
    
    assert_eq!(config.foo, "bar_value");
}

#[cfg(feature = "include")]
#[test]
fn test_nested_reader_budget() {
    let yaml = "foo: !include bar.yaml\n";
    let cursor = std::io::Cursor::new(yaml.as_bytes());

    let mut options = Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        let s = req.spec;
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
                // A reader that exceeds the budget: 15 bytes long
                source: InputSource::from_reader(std::io::Cursor::new(b"long_bar_value\n"))
            })
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });
    
    // Set a very small reader limit
    if let Some(ref mut b) = options.budget {
        b.max_reader_input_bytes = Some(5);
    } else {
        options.budget = Some(serde_saphyr::budget::Budget {
            max_reader_input_bytes: Some(5),
            ..Default::default()
        });
    }

    let config_res: Result<Config, serde_saphyr::Error> = serde_saphyr::from_reader_with_options(
        cursor,
        options,
    );

    // It should fail due to ExceededReaderInputLimit
    assert!(config_res.is_err());
    let err_msg = config_res.unwrap_err().to_string();
    assert!(err_msg.contains("size limit"), "Expected budget error, got: {}", err_msg);
}


#[cfg(feature = "include")]
#[test]
fn test_cyclic_include() {
    use serde_saphyr::{InputSource, ResolvedInclude};
    let input = "
root: !include self.yaml
";
    let resolver = |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, _> {
        let s = req.spec;
        Ok(ResolvedInclude {
            id: s.to_string(),
            name: s.to_string(),
            source: InputSource::from_string("root2: !include self.yaml".to_string()),
        })
    };
    let options = serde_saphyr::Options::default().with_include_resolver(resolver);
    let result: Result<serde::de::IgnoredAny, _> = serde_saphyr::from_str_with_options(input, options);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("cyclic include detected"), "{}", err_msg);
}

#[cfg(feature = "include")]
#[test]
fn cycle_detection_keys_off_id_not_name() {
    use serde_saphyr::{InputSource, ResolvedInclude};

    let input = "
root: !include aliases/first.yaml
";
    let resolver = |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, _> {
        let (id, name, source) = match req.spec {
            "aliases/first.yaml" => (
                "/canonical/shared.yaml",
                "aliases/first.yaml",
                "root2: !include aliases/second.yaml",
            ),
            "aliases/second.yaml" => (
                "/canonical/shared.yaml",
                "aliases/second.yaml",
                "root3: terminal_value",
            ),
            _ => unreachable!("unexpected include spec: {}", req.spec),
        };

        Ok(ResolvedInclude {
            id: id.to_string(),
            name: name.to_string(),
            source: InputSource::from_string(source.to_string()),
        })
    };
    let options = serde_saphyr::Options::default().with_include_resolver(resolver);
    let result: Result<serde::de::IgnoredAny, _> = serde_saphyr::from_str_with_options(input, options);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("cyclic include detected"), "{}", err_msg);
}

#[cfg(feature = "include")]
#[test]
fn test_repeated_includes_not_cyclic() {
    use serde_saphyr::{InputSource, ResolvedInclude};
    let input = "
list:
  - !include item.yaml
  - !include item.yaml
";
    let resolver = |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, _> {
        let s = req.spec;
        Ok(ResolvedInclude {
            id: s.to_string(),
            name: s.to_string(),
            source: InputSource::from_string("value".to_string()),
        })
    };
    let options = serde_saphyr::Options::default().with_include_resolver(resolver);
    // Should not fail with cyclic include error
    let result: Result<serde::de::IgnoredAny, _> = serde_saphyr::from_str_with_options(input, options);
    assert!(result.is_ok(), "Expected Ok, got {:?}", result.unwrap_err());
}

#[cfg(feature = "include")]
#[test]
fn resolver_request_uses_canonical_from_id_and_display_from_name() {
    use std::cell::RefCell;
    use std::rc::Rc;

    type SeenEntry = (String, String, Option<String>, Vec<String>);

    let input = "foo: !include child.yaml\n";
    let seen: Rc<RefCell<Vec<SeenEntry>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_in_resolver = Rc::clone(&seen);

    let options = serde_saphyr::Options::default().with_include_resolver(
        move |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            seen_in_resolver.borrow_mut().push((
                req.spec.to_string(),
                req.from_name.to_string(),
                req.from_id.map(str::to_string),
                req.stack.clone(),
            ));

            let source = match req.spec {
                "child.yaml" => "bar: !include grand.yaml\n",
                "grand.yaml" => "deep_value\n",
                _ => return Err(IncludeResolveError::Message("unexpected include".to_string())),
            };

            let (id, name) = match req.spec {
                "child.yaml" => ("/workspace/includes/child.yaml", "child.yaml"),
                "grand.yaml" => ("/workspace/includes/grand.yaml", "nested/grand.yaml"),
                _ => unreachable!("already handled unexpected include"),
            };

            Ok(ResolvedInclude {
                id: id.to_string(),
                name: name.to_string(),
                source: InputSource::from_string(source.to_string()),
            })
        },
    );

    let cfg: NestedConfig = serde_saphyr::from_str_with_options(input, options).unwrap();
    assert_eq!(cfg.foo.bar, "deep_value");

    let entries = seen.borrow();
    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].0, "child.yaml");
    assert_eq!(entries[0].2, None);
    assert_eq!(entries[0].3.last().map(String::as_str), Some(entries[0].1.as_str()));
    assert_eq!(entries[0].1, "<input>");

    assert_eq!(entries[1].0, "grand.yaml");
    assert_eq!(entries[1].1, "child.yaml");
    assert_eq!(entries[1].2.as_deref(), Some("/workspace/includes/child.yaml"));
    assert_eq!(entries[1].3, vec![entries[0].1.clone(), "child.yaml".to_string()]);
}

#[cfg(feature = "include")]
#[test]
fn test_unsupported_include_form() {
    let input = "
foo: !include { \"path\": \"file_b.yml\", \"extension\": \"txt\" }
";
    // We shouldn't even reach the resolver, so a dummy one is fine.
    let resolver = |_req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        Err(serde_saphyr::IncludeResolveError::Message("Not reached".to_string()))
    };
    let options = serde_saphyr::Options::default().with_include_resolver(resolver);
    let result: Result<Config, _> = serde_saphyr::from_str_with_options(input, options);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("currently only supports the scalar form"),
        "Expected unsupported include form error, got: {}",
        err_msg
    );
}

#[cfg(feature = "include")]
#[test]
fn test_include_sequence_form_without_resolver_is_not_treated_as_include() {
    let input = "
foo: !include [file_b.yml]
";
    let result: Result<serde::de::IgnoredAny, _> = serde_saphyr::from_str_with_options(input, Options::default());
    assert!(result.is_ok(), "Expected Ok without resolver, got: {:?}", result);
}

#[cfg(feature = "include")]
#[test]
fn test_include_mapping_form_without_resolver_is_not_treated_as_include() {
    let input = "
foo: !include { path: file_b.yml, extension: txt }
";
    let result: Result<serde::de::IgnoredAny, _> = serde_saphyr::from_str_with_options(input, Options::default());
    assert!(result.is_ok(), "Expected Ok without resolver, got: {:?}", result);
}

#[cfg(feature = "include")]
#[test]
fn test_include_fragment_tag_merges_into_spec() {
    let yaml = "foo: !include#user_fragment bar.yaml\n";

    let options = Options::default().with_include_resolver(
        |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            if req.spec == "bar.yaml#user_fragment" {
                Ok(ResolvedInclude {
                    id: req.spec.to_string(),
                    name: req.spec.to_string(),
                    source: InputSource::Text("bar_value\n".to_string()),
                })
            } else {
                Err(IncludeResolveError::Message(format!(
                    "Unexpected include spec: {}",
                    req.spec
                )))
            }
        },
    );

    let config: Config = serde_saphyr::from_str_with_options(yaml, options).unwrap();
    assert_eq!(config.foo, "bar_value");
}

#[cfg(feature = "include")]
#[test]
fn test_include_fragment_tag_and_fragment_in_spec_is_rejected() {
    let yaml = "foo: !include#user_fragment bar.yaml#from_spec\n";

    let options = Options::default().with_include_resolver(
        |_req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            Err(IncludeResolveError::Message("resolver should not be called".to_string()))
        },
    );

    let result: Result<Config, _> = serde_saphyr::from_str_with_options(yaml, options);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("must not contain '#'"),
        "Expected include fragment form validation error, got: {}",
        err
    );
}

#[cfg(feature = "include")]
#[test]
fn test_input_source_convenience_methods() {
    use std::io::Read;
    use serde_saphyr::{InputSource, IncludeResolveError};

    let text_source = InputSource::from_string("hello".to_string());
    match text_source {
        InputSource::Text(s) => assert_eq!(s, "hello"),
        _ => panic!("Expected Text variant"),
    }

    let cursor = std::io::Cursor::new(b"world".to_vec());
    let mut reader_source = InputSource::from_reader(cursor);
    match reader_source {
        InputSource::Reader(ref mut r) => {
            let mut buf = String::new();
            r.read_to_string(&mut buf).unwrap();
            assert_eq!(buf, "world");
        }
        _ => panic!("Expected Reader variant"),
    }

    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let resolve_err = IncludeResolveError::from(io_err);
    match resolve_err {
        IncludeResolveError::Io(e) => assert_eq!(e.kind(), std::io::ErrorKind::NotFound),
        _ => panic!("Expected Io variant"),
    }
}

#[cfg(feature = "include")]
#[test]
fn test_successful_reader_resolver() {
    let yaml = "foo: !include bar.yaml\n";
    let cursor = std::io::Cursor::new(yaml.as_bytes());
    
    let options = serde_saphyr::Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        if req.spec == "bar.yaml" {
            Ok(serde_saphyr::ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: serde_saphyr::InputSource::from_reader(std::io::Cursor::new(b"bar_value\n"))
            })
        } else {
            Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
        }
    });

    let config: Config = serde_saphyr::from_reader_with_options(cursor, options).unwrap();
    assert_eq!(config.foo, "bar_value");
}

#[cfg(feature = "include")]
#[test]
fn test_reader_include_syntax_error() {
    let yaml = "foo: !include bad.yaml\n";
    let cursor = std::io::Cursor::new(yaml.as_bytes());

    let options = serde_saphyr::Options::default().with_include_resolver(
        |req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
            if req.spec == "bad.yaml" {
                Ok(serde_saphyr::ResolvedInclude {
                    id: req.spec.to_string(),
                    name: req.spec.to_string(),
                    source: serde_saphyr::InputSource::from_reader(std::io::Cursor::new(b"'unterminated\n")),
                })
            } else {
                Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
            }
        },
    );

    let err = serde_saphyr::from_reader_with_options::<_, Config>(cursor, options)
        .expect_err("reader include syntax must fail");
    let rendered = err.to_string();

    assert!(rendered.contains("bad.yaml"), "{rendered}");
    assert!(rendered.contains("while parsing") || rendered.contains("did not find expected"), "{rendered}");
}

#[cfg(feature = "include")]
#[test]
fn test_reader_include_type_error() {
    let yaml = "foo: !include bad.yaml\n";
    let cursor = std::io::Cursor::new(yaml.as_bytes());

    let options = serde_saphyr::Options::default().with_include_resolver(
        |req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
            if req.spec == "bad.yaml" {
                Ok(serde_saphyr::ResolvedInclude {
                    id: req.spec.to_string(),
                    name: req.spec.to_string(),
                    source: serde_saphyr::InputSource::from_reader(std::io::Cursor::new(b"nested: true\n")),
                })
            } else {
                Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
            }
        },
    );

    let err = serde_saphyr::from_reader_with_options::<_, Config>(cursor, options)
        .expect_err("reader include type mismatch must fail");
    let rendered = err.to_string();

    assert!(rendered.contains("bad.yaml"), "{rendered}");
    assert!(rendered.contains("invalid type") || rendered.contains("expected"), "{rendered}");
}

#[cfg(feature = "include")]
#[test]
fn test_anchors_in_same_included_content() {
    let yaml = "foo: !include bar.yaml\n";

    let options = serde_saphyr::Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        if req.spec == "bar.yaml" {
            Ok(serde_saphyr::ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: serde_saphyr::InputSource::from_string("a: &anchor value\nb: *anchor\n".to_string())
            })
        } else {
            Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
        }
    });

    let config: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>> = 
        serde_saphyr::from_str_with_options(yaml, options).unwrap();
    
    let foo = config.get("foo").unwrap();
    assert_eq!(foo.get("a").unwrap(), "value");
    assert_eq!(foo.get("b").unwrap(), "value");
}

#[cfg(feature = "include")]
#[test]
fn test_anchors_across_included_content() {
    // Tests defining an anchor in one included content and referencing it in another included content.
    // It should fail because anchors are isolated per file.
    let yaml = "
file1: !include def.yaml
file2: !include ref.yaml
";

    let options = serde_saphyr::Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        if req.spec == "def.yaml" {
            Ok(serde_saphyr::ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: serde_saphyr::InputSource::from_string("&anchor value_from_def\n".to_string())
            })
        } else if req.spec == "ref.yaml" {
            Ok(serde_saphyr::ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: serde_saphyr::InputSource::from_string("*anchor\n".to_string())
            })
        } else {
            Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
        }
    });

    let result: Result<std::collections::BTreeMap<String, String>, _> = 
        serde_saphyr::from_str_with_options(yaml, options);
        
    assert!(result.is_err(), "Expected an error because anchors are isolated per inclusion");
}

#[cfg(feature = "include")]
#[test]
fn test_anchors_parent_to_include() {
    // Tests defining an anchor in the parent file and referencing it in an included file.
    // It should fail because anchors are isolated per file.
    let yaml = "
parent_def: &parent_anchor parent_value
child_ref: !include ref.yaml
";

    let options = serde_saphyr::Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        if req.spec == "ref.yaml" {
            Ok(serde_saphyr::ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: serde_saphyr::InputSource::from_string("*parent_anchor\n".to_string())
            })
        } else {
            Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
        }
    });

    let result: Result<std::collections::BTreeMap<String, String>, _> = 
        serde_saphyr::from_str_with_options(yaml, options);
        
    assert!(result.is_err(), "Expected an error because anchors are isolated per inclusion");
}

#[cfg(feature = "include")]
#[test]
fn test_include_fragment_replays_prerequisite_anchor_definitions() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Selected {
        user: std::collections::BTreeMap<String, String>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct Root {
        cfg: Selected,
    }

    let yaml = "cfg: !include value.yaml#selected\n";

    let options = serde_saphyr::Options::default().with_include_resolver(
        |req: serde_saphyr::IncludeRequest|
         -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
            if req.spec == "value.yaml#selected" {
                Ok(serde_saphyr::ResolvedInclude {
                    id: req.spec.to_string(),
                    name: req.spec.to_string(),
                    source: serde_saphyr::InputSource::AnchoredText {
                        text: "base: &base\n  name: Alice\n\nselected: &selected\n  user: *base\n"
                            .to_string(),
                        anchor: "selected".to_string(),
                    },
                })
            } else {
                Err(serde_saphyr::IncludeResolveError::Message(
                    "Not found".to_string(),
                ))
            }
        },
    );

    let parsed: Root = serde_saphyr::from_str_with_options(yaml, options)
        .expect("fragment include should replay prerequisite anchor definitions");

    let mut expected_user = std::collections::BTreeMap::new();
    expected_user.insert("name".to_string(), "Alice".to_string());

    assert_eq!(
        parsed,
        Root {
            cfg: Selected { user: expected_user }
        }
    );
}

#[cfg(feature = "include")]
#[test]
fn test_include_entire_mapping() {
    let yaml = "
my_mapping: !include some_mapping.yaml
";

    let options = serde_saphyr::Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        if req.spec == "some_mapping.yaml" {
            Ok(serde_saphyr::ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: serde_saphyr::InputSource::from_string("c: 3\nd: 4\n".to_string())
            })
        } else {
            Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
        }
    });

    let result: std::collections::BTreeMap<String, std::collections::BTreeMap<String, i32>> = 
        serde_saphyr::from_str_with_options(yaml, options).unwrap();
        
    let mapping = result.get("my_mapping").unwrap();
    assert_eq!(mapping.get("c").unwrap(), &3);
    assert_eq!(mapping.get("d").unwrap(), &4);
}

#[cfg(feature = "include")]
#[test]
fn test_include_list() {
    let yaml = "
my_list: !include some_list.yaml
";

    let options = serde_saphyr::Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        if req.spec == "some_list.yaml" {
            Ok(serde_saphyr::ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: serde_saphyr::InputSource::from_string("[1, 2, 3]\n".to_string())
            })
        } else {
            Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
        }
    });

    let result: std::collections::BTreeMap<String, Vec<i32>> = 
        serde_saphyr::from_str_with_options(yaml, options).unwrap();
        
    let list = result.get("my_list").unwrap();
    assert_eq!(list, &vec![1, 2, 3]);
}

#[cfg(feature = "include")]
#[test]
fn test_include_with_merge() {
    let yaml = "
base:
  <<: !include child.yaml
  override: 2
";

    let options = serde_saphyr::Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        if req.spec == "child.yaml" {
            Ok(serde_saphyr::ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: serde_saphyr::InputSource::from_string("a: 1\nb: 2\noverride: 1\n".to_string())
            })
        } else {
            Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
        }
    });

    let result: std::collections::BTreeMap<String, std::collections::BTreeMap<String, i32>> = 
        serde_saphyr::from_str_with_options(yaml, options).unwrap();
        
    let base = result.get("base").unwrap();
    assert_eq!(base.get("a").unwrap(), &1);
    assert_eq!(base.get("b").unwrap(), &2);
    assert_eq!(base.get("override").unwrap(), &2);
}

#[cfg(feature = "include")]
#[test]
fn test_anchor_on_include_site() {
    let yaml = "
base:
  inc: &A !include child.yaml
  ref: *A
";

    let options = serde_saphyr::Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        if req.spec == "child.yaml" {
            Ok(serde_saphyr::ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: serde_saphyr::InputSource::from_string("a: 1\nb: 2\n".to_string())
            })
        } else {
            Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
        }
    });

    let result: std::collections::BTreeMap<String, std::collections::BTreeMap<String, std::collections::BTreeMap<String, i32>>> = 
        serde_saphyr::from_str_with_options(yaml, options).unwrap();
        
    let base = result.get("base").unwrap();
    let inc = base.get("inc").unwrap();
    let ref_ = base.get("ref").unwrap();
    
    assert_eq!(inc.get("a").unwrap(), &1);
    assert_eq!(ref_.get("a").unwrap(), &1);
}

#[cfg(feature = "include")]
#[test]
fn test_anchor_on_empty_include() {
    let yaml = "
base:
  inc: !include empty.yaml
  next: value
";

    let options = serde_saphyr::Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        if req.spec == "empty.yaml" {
            Ok(serde_saphyr::ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: serde_saphyr::InputSource::from_string("".to_string())
            })
        } else {
            Err(serde_saphyr::IncludeResolveError::Message("Not found".to_string()))
        }
    });

    let result: std::collections::BTreeMap<String, std::collections::BTreeMap<String, Option<String>>> = 
        serde_saphyr::from_str_with_options(yaml, options).unwrap();
        
    println!("result: {:#?}", result);
    let base = result.get("base").unwrap();
    let inc = base.get("inc").unwrap();
    assert_eq!(inc, &None); // The empty include resolves to a null value.
}

#[cfg(feature = "include")]
#[test]
fn test_include_request_reports_remaining_reader_quota() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_in_resolver = seen.clone();
    let yaml = "pad: 12345\ninc: !include child.yaml\n";
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_reader_input_bytes: Some(64),
        },
    }
    .with_include_resolver(move |req: serde_saphyr::IncludeRequest| {
        seen_in_resolver.borrow_mut().push(req.size_remaining);
        Ok(serde_saphyr::ResolvedInclude {
            id: req.spec.to_string(),
            name: req.spec.to_string(),
            source: serde_saphyr::InputSource::from_string("value: ok\n".to_string()),
        })
    });

    let parsed: QuotaConfig =
        serde_saphyr::from_reader_with_options(std::io::Cursor::new(yaml.as_bytes()), options)
            .unwrap();
    assert_eq!(parsed.pad, 12345);
    assert_eq!(parsed.inc.get("value").map(String::as_str), Some("ok"));

    let remaining = seen.borrow();
    assert_eq!(remaining.len(), 1);
    let remaining = remaining[0].expect("remaining quota should be reported");
    assert!(remaining < 64, "remaining quota should shrink after root bytes are read");
    assert!(remaining >= "value: ok\n".len());
}
