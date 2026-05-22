//! Internal support for deserializing `Commented<T>`.

use serde::de::{self, IntoDeserializer, Visitor};

use super::Error;
use crate::Deserializer;

pub(super) fn deserialize_yaml_commented<'de, V>(
    mut de: Deserializer<'de, '_>,
    visitor: V,
) -> Result<V::Value, Error>
where
    V: Visitor<'de>,
{
    let mut comments = std::mem::take(&mut de.pending_comments);
    comments.extend(std::mem::take(&mut de.pending_value_comments));
    comments.extend(de.ev.take_leading_comments_for_next_node()?);

    visitor.visit_seq(CommentedSeqAccess {
        de,
        comments,
        state: 0,
    })
}

fn joined_comments(comments: Vec<String>) -> String {
    comments
        .into_iter()
        .filter(|comment| !comment.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

struct CommentedSeqAccess<'de, 'e> {
    de: Deserializer<'de, 'e>,
    comments: Vec<String>,
    state: u8,
}

impl<'de, 'e> de::SeqAccess<'de> for CommentedSeqAccess<'de, 'e> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.state {
            0 => {
                self.state = 1;
                let value = seed.deserialize(Deserializer::new(&mut *self.de.ev, self.de.cfg))?;
                self.comments
                    .extend(self.de.ev.take_trailing_comments_after_node()?);
                Ok(Some(value))
            }
            1 => {
                self.state = 2;
                let comment = joined_comments(std::mem::take(&mut self.comments));
                seed.deserialize(comment.into_deserializer()).map(Some)
            }
            _ => Ok(None),
        }
    }
}
