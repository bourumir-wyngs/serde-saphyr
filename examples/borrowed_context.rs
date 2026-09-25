//! Pass formatting context through borrowed `Serialize` adapters.
//!
//! Run with: cargo run --example borrowed_context
//!
//! An application can keep its original value tree (represented here by
//! `serde_json::Value`) and borrow it together with per-call formatting options.
//! Each map or sequence creates temporary adapters for its children as Serde
//! visits them. This avoids allocating a second tree of serialization containers
//! or storing the context in thread-local state; ordinary Serde is sufficient.
//!
//! Adapters compose with `Tagged<T>` and select style wrappers such as `LitStr`
//! and `DoubleQuoted` at the leaves. This is not allocation-free serialization:
//! `Tagged` owns its tag string, and the output and serializer may allocate.

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use serde_json::{Value, json};
use serde_saphyr::{DoubleQuoted, LitStr, Tagged};

struct FormatContext {
    // Quoting takes precedence over literal block style when both are enabled.
    quote_string_values: bool,
    literal_multiline: bool,
}

struct YamlView<'value, 'context> {
    value: &'value Value,
    context: &'context FormatContext,
}

impl Serialize for YamlView<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.value {
            Value::String(text) if self.context.quote_string_values => {
                DoubleQuoted(text.as_str()).serialize(serializer)
            }
            Value::String(text) if self.context.literal_multiline && text.contains('\n') => {
                LitStr(text).serialize(serializer)
            }
            Value::Array(values) => {
                let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    sequence.serialize_element(&YamlView {
                        value,
                        context: self.context,
                    })?;
                }
                sequence.end()
            }
            Value::Object(entries) => {
                let mut mapping = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    // Keys use the serializer's normal string formatting.
                    mapping.serialize_entry(
                        key,
                        &YamlView {
                            value,
                            context: self.context,
                        },
                    )?;
                }
                mapping.end()
            }
            // Only scalar leaves can bypass the adapter. Serializing a whole
            // container directly would skip the context for its descendants.
            scalar => scalar.serialize(serializer),
        }
    }
}

fn main() -> anyhow::Result<()> {
    // Build the application's original tree once. Serialization only borrows it.
    let value = json!({
        "name": "build",
        "steps": [
            {"command": "cargo check\ncargo test\n", "retries": 2},
            "done"
        ]
    });
    let readable = FormatContext {
        quote_string_values: false,
        literal_multiline: true,
    };
    let quoted = FormatContext {
        quote_string_values: true,
        literal_multiline: false,
    };

    // Both views can coexist: each borrows its own context and the same tree.
    let readable_view = YamlView {
        value: &value,
        context: &readable,
    };
    let quoted_view = YamlView {
        value: &value,
        context: &quoted,
    };
    let readable_yaml = serde_saphyr::to_string(&Tagged(&readable_view, Some("!job".into())))?;
    let quoted_yaml = serde_saphyr::to_string(&Tagged(&quoted_view, Some("!job".into())))?;

    println!("Readable string values:\n{readable_yaml}");
    println!("Quoted string values:\n{quoted_yaml}");
    Ok(())
}
