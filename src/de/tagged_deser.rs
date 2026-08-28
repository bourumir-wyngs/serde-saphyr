//! Internal support for deserializing `Tagged<T>`.

use serde_core::de::{self, IntoDeserializer, Visitor};

use super::Error;
use super::events::Ev;
use crate::Deserializer;

/// Capture the parser-resolved tag on the next YAML node and expose the
/// wrapper as the virtual sequence `[value, tag?]`.
pub(super) fn deserialize_yaml_tagged<'de, V>(
    de: Deserializer<'de, '_>,
    visitor: V,
) -> Result<V::Value, Error>
where
    V: Visitor<'de>,
{
    let tag = match de.ev.peek()? {
        Some(
            Ev::Scalar { raw_tag, .. }
            | Ev::SeqStart { raw_tag, .. }
            | Ev::MapStart { raw_tag, .. },
        ) => raw_tag.as_deref().map(str::to_owned),
        Some(Ev::SeqEnd { .. } | Ev::MapEnd { .. } | Ev::Taken { .. }) | None => None,
    };

    visitor.visit_seq(TaggedSeqAccess { de, tag, state: 0 })
}

struct TaggedSeqAccess<'de, 'e> {
    de: Deserializer<'de, 'e>,
    tag: Option<String>,
    state: u8,
}

impl<'de> de::SeqAccess<'de> for TaggedSeqAccess<'de, '_> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.state {
            0 => {
                self.state = 1;

                #[cfg(any(feature = "garde", feature = "validator"))]
                let value = if let Some(garde_ref) = self.de.garde.as_mut() {
                    let recorder: &mut super::path_map::PathRecorder = garde_ref;
                    let mut de = Deserializer::new_with_path_recorder(
                        &mut *self.de.ev,
                        self.de.cfg,
                        recorder,
                    );
                    de.pending_comments = std::mem::take(&mut self.de.pending_comments);
                    de.pending_value_separator_comments =
                        std::mem::take(&mut self.de.pending_value_separator_comments);
                    de.pending_value_comments = std::mem::take(&mut self.de.pending_value_comments);
                    seed.deserialize(de)?
                } else {
                    let mut de = Deserializer::new(&mut *self.de.ev, self.de.cfg);
                    de.pending_comments = std::mem::take(&mut self.de.pending_comments);
                    de.pending_value_separator_comments =
                        std::mem::take(&mut self.de.pending_value_separator_comments);
                    de.pending_value_comments = std::mem::take(&mut self.de.pending_value_comments);
                    seed.deserialize(de)?
                };

                #[cfg(not(any(feature = "garde", feature = "validator")))]
                let value = {
                    let mut de = Deserializer::new(&mut *self.de.ev, self.de.cfg);
                    de.pending_comments = std::mem::take(&mut self.de.pending_comments);
                    de.pending_value_separator_comments =
                        std::mem::take(&mut self.de.pending_value_separator_comments);
                    de.pending_value_comments = std::mem::take(&mut self.de.pending_value_comments);
                    seed.deserialize(de)?
                };

                Ok(Some(value))
            }
            1 => {
                self.state = 2;
                match self.tag.take() {
                    Some(tag) => seed.deserialize(tag.into_deserializer()).map(Some),
                    None => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        let len = 1 + usize::from(self.tag.is_some());
        Some(len.saturating_sub(self.state.into()))
    }
}

#[cfg(test)]
mod tests {
    use serde_core::Deserialize;
    use serde_core::de::{self, SeqAccess, Visitor};
    use std::fmt;

    use crate::{Commented, Tagged, from_str};

    #[derive(Debug, PartialEq)]
    struct InspectedTagged {
        value: u32,
        tag: Option<String>,
        size_hints: [Option<usize>; 4],
    }

    impl<'de> Deserialize<'de> for InspectedTagged {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde_core::Deserializer<'de>,
        {
            struct InspectVisitor;

            impl<'de> Visitor<'de> for InspectVisitor {
                type Value = InspectedTagged;

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a tagged value exposed as a sequence")
                }

                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: SeqAccess<'de>,
                {
                    let before_value = seq.size_hint();
                    let value = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                    let after_value = seq.size_hint();
                    let tag = seq.next_element()?;
                    let after_tag = seq.size_hint();
                    if seq.next_element::<de::IgnoredAny>()?.is_some() {
                        return Err(de::Error::invalid_length(3, &self));
                    }
                    let after_exhaustion = seq.size_hint();

                    Ok(InspectedTagged {
                        value,
                        tag,
                        size_hints: [before_value, after_value, after_tag, after_exhaustion],
                    })
                }
            }

            deserializer.deserialize_newtype_struct("__yaml_tagged", InspectVisitor)
        }
    }

    #[test]
    fn captures_resolved_tags_and_none_for_untagged_nodes() {
        let core: Tagged<String> = from_str("!!str value").unwrap();
        assert_eq!(
            core,
            Tagged("value".into(), Some("tag:yaml.org,2002:str".into()))
        );

        let local: Tagged<String> = from_str("!widget value").unwrap();
        assert_eq!(local, Tagged("value".into(), Some("!widget".into())));

        let verbatim: Tagged<String> = from_str("!<tag:example.com,2026:widget> value").unwrap();
        assert_eq!(
            verbatim,
            Tagged("value".into(), Some("tag:example.com,2026:widget".into()))
        );

        let untagged: Tagged<String> = from_str("value").unwrap();
        assert_eq!(untagged, Tagged("value".into(), None));
    }

    #[test]
    fn virtual_sequence_reports_remaining_elements_and_stays_exhausted() {
        let tagged: InspectedTagged = from_str("!number 7").unwrap();
        assert_eq!(
            tagged,
            InspectedTagged {
                value: 7,
                tag: Some("!number".into()),
                size_hints: [Some(2), Some(1), Some(0), Some(0)],
            }
        );

        let untagged: InspectedTagged = from_str("7").unwrap();
        assert_eq!(
            untagged,
            InspectedTagged {
                value: 7,
                tag: None,
                size_hints: [Some(1), Some(0), Some(0), Some(0)],
            }
        );
    }

    #[test]
    fn empty_input_reaches_the_tagged_eof_path() {
        let error = from_str::<Tagged<String>>("").unwrap_err();
        assert!(
            error.to_string().contains("unexpected end of input"),
            "{error}"
        );
    }

    #[test]
    fn captures_tag_directive_resolution() {
        let tagged: Tagged<String> =
            from_str("%TAG !e! tag:example.com,2026:\n--- !e!widget value\n").unwrap();

        assert_eq!(
            tagged,
            Tagged("value".into(), Some("tag:example.com,2026:widget".into()))
        );
    }

    #[test]
    fn captures_container_tags() {
        let sequence: Tagged<Vec<u32>> = from_str("!!seq [1, 2]").unwrap();
        assert_eq!(
            sequence,
            Tagged(vec![1, 2], Some("tag:yaml.org,2002:seq".into()))
        );

        let mapping: Tagged<std::collections::BTreeMap<String, u32>> =
            from_str("!!map {one: 1}").unwrap();
        assert_eq!(
            mapping,
            Tagged(
                std::collections::BTreeMap::from([("one".into(), 1)]),
                Some("tag:yaml.org,2002:map".into()),
            )
        );
    }

    #[test]
    fn composes_with_commented_in_both_orders() {
        let tagged_comment: Tagged<Commented<String>> = from_str("!widget value # note").unwrap();
        assert_eq!(
            tagged_comment,
            Tagged(
                Commented("value".into(), "note".into()),
                Some("!widget".into())
            )
        );

        let commented_tag: Commented<Tagged<String>> = from_str("!widget value # note").unwrap();
        assert_eq!(
            commented_tag,
            Commented(
                Tagged("value".into(), Some("!widget".into())),
                "note".into()
            )
        );
    }
}
