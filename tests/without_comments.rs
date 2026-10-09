#![cfg(all(feature = "deserialize", not(feature = "comments")))]

use serde_json::{Value, json};
use serde_saphyr::budget::{BudgetReport, EnforcingPolicy, check_yaml_budget};
use serde_saphyr::{
    Budget, Error, Options, from_reader_with_options, from_slice_with_options,
    from_str_with_options,
};
use std::cell::RefCell;
use std::rc::Rc;

type Parse = fn(&str, Options) -> Result<Value, Error>;

const ENTRYPOINTS: [(&str, Parse); 3] = [
    ("string", |yaml, options| {
        from_str_with_options(yaml, options)
    }),
    ("slice", |yaml, options| {
        from_slice_with_options(yaml.as_bytes(), options)
    }),
    ("reader", |yaml, options| {
        from_reader_with_options(yaml.as_bytes(), options)
    }),
];

fn options(with_budget: bool) -> Options {
    let mut options = Options::default();
    if !with_budget {
        options.budget = None;
    }
    options
}

#[test]
fn comments_are_ignored_for_string_slice_and_reader_inputs() {
    const YAML: &str = "# document\nfirst: &items # anchor\n  - 1 # one\n  # separator\n  - 2\nsecond: *items # alias\nflow: [3, # flow item\n  4]\n";
    let expected = json!({
        "first": [1, 2],
        "second": [1, 2],
        "flow": [3, 4],
    });

    for (entrypoint, parse) in ENTRYPOINTS {
        for with_budget in [true, false] {
            let value = parse(YAML, options(with_budget)).expect(entrypoint);
            assert_eq!(value, expected, "{entrypoint}, with_budget={with_budget}");
        }
    }
}

fn parse_with_report(parse: Parse, yaml: &str, options: Options) -> (Value, BudgetReport) {
    let report = Rc::new(RefCell::new(None));
    let captured_report = Rc::clone(&report);
    let options =
        options.with_budget_report(move |value| *captured_report.borrow_mut() = Some(value));
    let value = parse(yaml, options).expect("YAML should parse within budget");
    let report = report
        .borrow_mut()
        .take()
        .expect("budget report should be delivered");
    (value, report)
}

#[test]
fn ignored_comments_do_not_consume_the_event_budget() {
    for (entrypoint, parse) in ENTRYPOINTS {
        let (_, baseline) = parse_with_report(parse, "value\n", Options::default());
        let options = serde_saphyr::options! {
            budget: serde_saphyr::budget! { max_events: baseline.events },
        };
        let (value, report) =
            parse_with_report(parse, "# first\n# second\nvalue # trailing\n", options);

        assert_eq!(value, "value", "{entrypoint}");
        assert_eq!(report.events, baseline.events, "{entrypoint}");
        assert!(report.breached.is_none(), "{entrypoint}");
    }
}

#[test]
fn standalone_budget_checks_ignore_comments() {
    let baseline =
        check_yaml_budget("value\n", Budget::default(), EnforcingPolicy::AllContent).unwrap();
    let report = check_yaml_budget(
        "# first\n# second\nvalue # trailing\n",
        serde_saphyr::budget! { max_events: baseline.events }.unwrap(),
        EnforcingPolicy::AllContent,
    )
    .unwrap();

    assert!(report.breached.is_none());
    assert_eq!(report.events, baseline.events);
}

#[cfg(feature = "include")]
#[test]
fn comments_are_ignored_in_included_sources() {
    use serde_saphyr::{InputSource, ResolvedInclude};

    const YAML: &str = "# document\nkey: &selected # anchor\n  - 1 # one\n  # separator\n  - 2\n";

    for (entrypoint, parse) in ENTRYPOINTS {
        for source_kind in ["text", "reader", "anchored"] {
            for with_budget in [true, false] {
                let options = options(with_budget).with_include_resolver(move |req| {
                    let source = match source_kind {
                        "text" => InputSource::from_string(YAML.to_string()),
                        "reader" => InputSource::Reader(Box::new(YAML.as_bytes())),
                        "anchored" => InputSource::AnchoredText {
                            text: YAML.to_string(),
                            anchor: "selected".to_string(),
                        },
                        _ => unreachable!(),
                    };
                    Ok(ResolvedInclude::new(req.spec, req.spec, source))
                });
                let value = parse("included: !include child.yaml\n", options).unwrap();
                let expected = if source_kind == "anchored" {
                    json!({"included": [1, 2]})
                } else {
                    json!({"included": {"key": [1, 2]}})
                };
                assert_eq!(
                    value, expected,
                    "{entrypoint}, {source_kind}, with_budget={with_budget}"
                );
            }
        }
    }
}
