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
    use crate::{Commented, Tagged, from_str};

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
