//! Internal support for deserializing `Commented<T>`.

use std::borrow::Cow;

use serde_core::de::{self, IntoDeserializer, Visitor};

use super::Error;
use super::events::Ev;
use crate::Deserializer;

pub(super) fn deserialize_yaml_commented<'de, V>(
    mut de: Deserializer<'de, '_>,
    visitor: V,
) -> Result<V::Value, Error>
where
    V: Visitor<'de>,
{
    let next_is_container = matches!(
        de.ev.peek()?,
        Some(Ev::MapStart { .. } | Ev::SeqStart { .. })
    );

    let mut comments = std::mem::take(&mut de.pending_comments);
    comments.extend(std::mem::take(&mut de.pending_value_separator_comments));
    if next_is_container {
        // Parent-classified value comments belong inside the container. This
        // also covers nested aliases to containers: alias replay exposes the
        // anchored container start here, and comments above the alias stay
        // available to the first replayed child.
        //
        // If no parent has classified comments for us, live leading comments on
        // the container start belong to this wrapper.
        if de.pending_value_comments.is_empty() {
            comments.extend(de.ev.take_leading_comments_for_next_node()?);
        }
    } else {
        comments.extend(std::mem::take(&mut de.pending_value_comments));
        comments.extend(de.ev.take_leading_comments_for_next_node()?);
    }

    visitor.visit_seq(CommentedSeqAccess {
        de,
        comments,
        // For containers, comments already inside the container must stay with
        // the wrapped value so the first child key/item can claim them. This is
        // intentionally the same for a nested alias whose target is a container.
        defer_value_comments: next_is_container,
        state: 0,
    })
}

fn joined_comments(comments: Vec<Cow<'_, str>>) -> String {
    comments
        .into_iter()
        .filter(|comment| !comment.is_empty())
        .map(Cow::into_owned)
        .collect::<Vec<_>>()
        .join("\n")
}

struct CommentedSeqAccess<'de, 'e> {
    de: Deserializer<'de, 'e>,
    comments: Vec<Cow<'de, str>>,
    defer_value_comments: bool,
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
                let value = {
                    let mut de = Deserializer::new(&mut *self.de.ev, self.de.cfg);
                    if self.defer_value_comments {
                        de.pending_value_comments =
                            std::mem::take(&mut self.de.pending_value_comments);
                    }
                    seed.deserialize(de)?
                };
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
