#![cfg(feature = "deserialize")]
/// See https://github.com/bourumir-wyngs/serde-saphyr/issues/170
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HorizontalIndex {
    Relative(i32),
    Absolute(AbsoluteIndex),
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(untagged, rename_all = "camelCase", try_from = "Value", into = "Value")]
pub enum AbsoluteIndex {
    Number(u32),
    Last,
}

impl TryFrom<Value> for AbsoluteIndex {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Number(number) => number
                .as_u64()
                .and_then(|number| u32::try_from(number).ok())
                .map(Self::Number)
                .ok_or_else(|| "absolute index must be an unsigned 32-bit integer".to_owned()),
            Value::String(value) if value == "last" => Ok(Self::Last),
            _ => Err("absolute index must be an unsigned 32-bit integer or `last`".to_owned()),
        }
    }
}

impl From<AbsoluteIndex> for Value {
    fn from(value: AbsoluteIndex) -> Self {
        match value {
            AbsoluteIndex::Number(number) => Self::from(number),
            AbsoluteIndex::Last => Self::String("last".to_owned()),
        }
    }
}

#[test]
fn serde_hor_index() -> Result<()> {
    let res: HorizontalIndex = serde_saphyr::from_str("!relative 5")?;
    assert_eq!(res, HorizontalIndex::Relative(5));
    let res: HorizontalIndex = serde_saphyr::from_str("!relative -3")?;
    assert_eq!(res, HorizontalIndex::Relative(-3));
    let res: HorizontalIndex = serde_saphyr::from_str("!relative 0")?;
    assert_eq!(res, HorizontalIndex::Relative(0));
    let res: HorizontalIndex = serde_saphyr::from_str("!absolute 0")?;
    assert_eq!(res, HorizontalIndex::Absolute(AbsoluteIndex::Number(0)));
    let res: HorizontalIndex = serde_saphyr::from_str("!absolute 7")?;
    assert_eq!(res, HorizontalIndex::Absolute(AbsoluteIndex::Number(7)));
    let res: HorizontalIndex = serde_saphyr::from_str("!absolute last")?;
    assert_eq!(res, HorizontalIndex::Absolute(AbsoluteIndex::Last));
    Ok(())
}
