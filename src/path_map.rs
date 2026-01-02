//! This module records YAML key paths (as seen during deserialization) and later tries to map
//! validation paths back to those YAML locations.
//!
//! The key problem is that validation paths are derived from Rust field identifiers (typically
//! `snake_case`), while YAML keys can use different spellings (`camelCase`, `kebab-case`, etc.).
//! The parser has no direct access to Rust field names as reported by validation crates.
//!
//! We apply a small, ordered set of comparison strategies and only accept a match when it is
//! **unique**.
//!
//! Matching rules:
//! - Paths must have the same length and the same per-segment kind (key vs index).
//! - Segment names are first normalized by stripping Rust raw-identifier prefixes (`r#type` →
//!   `type`) to work around reserved-keyword field names.
//! - `PathMap::search` runs multiple passes from most exact to most fuzzy:
//!   1. Direct lookup (exact `Path` equality).
//!   2. Whole-path ASCII case-insensitive match.
//!   3. Token-sequence match: split on separators and common casing/digit boundaries
//!      (`user_id`, `userId`, `user-id` → tokens `user`, `id`).
//!   4. Collapsed match: drop all non-alphanumeric characters and compare ASCII-lowercased.
//!
//! Any non-direct pass succeeds only if it yields exactly one candidate; otherwise the result is
//! considered ambiguous.

use crate::Location;

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PathKind {
    Key,
    Index,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PathSegment {
    pub(crate) kind: PathKind,
    pub(crate) name: String,
}

impl From<&str> for PathSegment {
    fn from(value: &str) -> Self {
        Self {
            kind: PathKind::Key,
            name: value.to_owned(),
        }
    }
}

impl From<String> for PathSegment {
    fn from(value: String) -> Self {
        Self {
            kind: PathKind::Key,
            name: value,
        }
    }
}

impl From<usize> for PathSegment {
    fn from(value: usize) -> Self {
        Self {
            kind: PathKind::Index,
            name: value.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PathKey {
    segments: Vec<PathSegment>,
}

impl PathKey {
    pub(crate) fn empty() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub(crate) fn join<T: Into<PathSegment>>(mut self, seg: T) -> Self {
        self.segments.push(seg.into());
        self
    }

    pub(crate) fn len(&self) -> usize {
        self.segments.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub(crate) fn leaf_string(&self) -> Option<String> {
        self.segments.last().map(|seg| seg.name.clone())
    }

    fn iter_segments(&self) -> impl Iterator<Item = (&PathKind, &str)> {
        self.segments
            .iter()
            .map(|seg| (&seg.kind, seg.name.as_str()))
    }
}

pub(crate) fn format_path_with_resolved_leaf(path: &PathKey, resolved_leaf: &str) -> String {
    let mut out = String::new();
    let last_index = path.segments.len().saturating_sub(1);

    for (idx, seg) in path.segments.iter().enumerate() {
        match seg.kind {
            PathKind::Index => {
                out.push('[');
                out.push_str(&seg.name);
                out.push(']');
            }
            PathKind::Key => {
                if idx > 0 {
                    out.push('.');
                }
                if idx == last_index {
                    out.push_str(resolved_leaf);
                } else {
                    out.push_str(&seg.name);
                }
            }
        }
    }

    if out.is_empty() {
        "<root>".to_owned()
    } else {
        out
    }
}

#[cfg(feature = "garde")]
pub(crate) fn path_key_from_garde(path: &garde::error::Path) -> PathKey {
    use garde::error::Kind;

    let mut segs: Vec<PathSegment> = path
        .__iter()
        .map(|(k, s)| match k {
            Kind::Index => PathSegment {
                kind: PathKind::Index,
                name: s.as_str().to_owned(),
            },
            _ => PathSegment {
                kind: PathKind::Key,
                name: s.as_str().to_owned(),
            },
        })
        .collect();
    segs.reverse();

    PathKey { segments: segs }
}

#[derive(Debug)]
pub struct PathMap {
    pub(crate) map: HashMap<PathKey, Location>,
}

impl PathMap {
    pub(crate) fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub(crate) fn insert(&mut self, path: PathKey, location: Location) {
        self.map.insert(path, location);
    }

    pub(crate) fn search(&self, path: &PathKey) -> Option<(Location, String)> {
        // 1) Direct lookup.
        if let Some(loc) = self.map.get(path) {
            let leaf = path.leaf_string()?;
            return Some((*loc, leaf));
        }

        // Multi-pass matching (more exact -> more fuzzy). Each pass succeeds only if it yields
        // exactly one candidate.
        //
        // This is used to bridge Rust field paths (snake_case, etc.) to YAML key spellings
        // recorded during deserialization, without attempting arbitrary rename mapping.

        // 2) Whole-path case-insensitive match (only if unique).
        self.find_unique_by(path, segments_equal_case_insensitive)
            // 3) Token-sequence match (only if unique).
            //
            // Tokenization is stronger than “collapsed” matching: it treats separators and common
            // casing boundaries as token boundaries, reducing false collisions like:
            //   ab_c  vs a_bc  (both collapse to "abc", but tokenize to ["ab","c"] vs ["a","bc"]).
            .or_else(|| self.find_unique_by(path, segments_equal_tokenized_case_insensitive))
            // 4) Loose collapsed match (only if unique): remove all non-alphanumeric characters
            // within each segment and compare case-insensitively.
            .or_else(|| self.find_unique_by(path, segments_equal_collapsed_case_insensitive))
    }

    fn find_unique_by(
        &self,
        target: &PathKey,
        mut matches: impl FnMut(&PathKey, &PathKey) -> bool,
    ) -> Option<(Location, String)> {
        if target.is_empty() {
            return None;
        }

        let mut found: Option<(Location, String)> = None;
        for (candidate, loc) in self.map.iter() {
            if matches(target, candidate) {
                if found.is_some() {
                    return None; // ambiguous
                }
                found = Some((*loc, candidate.leaf_string()?));
            }
        }
        found
    }
}

fn strip_raw_identifier_prefix(s: &str) -> &str {
    // Rust raw identifiers are formatted like `r#type`.
    s.strip_prefix("r#").unwrap_or(s)
}

fn segments_equal_case_insensitive(target: &PathKey, candidate: &PathKey) -> bool {
    if target.len() != candidate.len() {
        return false;
    }

    target
        .iter_segments()
        .zip(candidate.iter_segments())
        .all(|((tk, ts), (ck, cs))| {
            tk == ck
                && match tk {
                    PathKind::Index => ts == cs,
                    PathKind::Key => strip_raw_identifier_prefix(ts)
                        .eq_ignore_ascii_case(strip_raw_identifier_prefix(cs)),
                }
        })
}

fn collapse_non_alnum_ascii_lower(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    out.extend(
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase()),
    );
    out
}

fn segments_equal_collapsed_case_insensitive(target: &PathKey, candidate: &PathKey) -> bool {
    if target.len() != candidate.len() {
        return false;
    }

    target
        .iter_segments()
        .zip(candidate.iter_segments())
        .all(|((tk, ts), (ck, cs))| {
            tk == ck
                && match tk {
                    PathKind::Index => ts == cs,
                    PathKind::Key => {
                        collapse_non_alnum_ascii_lower(strip_raw_identifier_prefix(ts))
                            == collapse_non_alnum_ascii_lower(strip_raw_identifier_prefix(cs))
                    }
                }
        })
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CharClass {
    Lower,
    Upper,
    Digit,
    Other,
}

fn classify_ascii(c: char) -> CharClass {
    if c.is_ascii_lowercase() {
        CharClass::Lower
    } else if c.is_ascii_uppercase() {
        CharClass::Upper
    } else if c.is_ascii_digit() {
        CharClass::Digit
    } else {
        CharClass::Other
    }
}

fn tokenize_segment(s: &str) -> Vec<String> {
    // 1) Split on any non-alphanumeric separator.
    // 2) Further split each piece on:
    //    - camel/pascal case boundaries (userId -> user + id)
    //    - digit boundaries (sha256Sum -> sha + 256 + sum)
    //    - acronym boundary heuristic (HTTPServer -> http + server)
    let mut tokens = Vec::new();

    for piece in s
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
    {
        let chars: Vec<char> = piece.chars().collect();
        if chars.is_empty() {
            continue;
        }

        let mut start = 0usize;
        for i in 1..chars.len() {
            let prev = classify_ascii(chars[i - 1]);
            let curr = classify_ascii(chars[i]);
            let next = chars.get(i + 1).copied().map(classify_ascii);

            let boundary = match (prev, curr) {
                // userId / userID / userID2
                (CharClass::Lower, CharClass::Upper) => true,
                // sha256 / foo2Bar
                (CharClass::Digit, CharClass::Lower | CharClass::Upper) => true,
                (CharClass::Lower | CharClass::Upper, CharClass::Digit) => true,
                // HTTPServer: split before the S in Server (Acronym + Word)
                (CharClass::Upper, CharClass::Upper) if matches!(next, Some(CharClass::Lower)) => {
                    true
                }
                _ => false,
            };

            if boundary {
                if start < i {
                    let tok: String = chars[start..i]
                        .iter()
                        .map(|c| c.to_ascii_lowercase())
                        .collect();
                    if !tok.is_empty() {
                        tokens.push(tok);
                    }
                }
                start = i;
            }
        }

        if start < chars.len() {
            let tok: String = chars[start..]
                .iter()
                .map(|c| c.to_ascii_lowercase())
                .collect();
            if !tok.is_empty() {
                tokens.push(tok);
            }
        }
    }

    tokens
}

fn segments_equal_tokenized_case_insensitive(target: &PathKey, candidate: &PathKey) -> bool {
    if target.len() != candidate.len() {
        return false;
    }

    target
        .iter_segments()
        .zip(candidate.iter_segments())
        .all(|((tk, ts), (ck, cs))| {
            tk == ck
                && match tk {
                    PathKind::Index => ts == cs,
                    PathKind::Key => {
                        tokenize_segment(strip_raw_identifier_prefix(ts))
                            == tokenize_segment(strip_raw_identifier_prefix(cs))
                    }
                }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p2(a: &str, b: &str) -> PathKey {
        PathKey::empty().join(a).join(b)
    }

    #[test]
    fn search_direct_hit() {
        let mut m = PathMap::new();
        let k = p2("gp", "a1");
        m.insert(k.clone(), Location::new(3, 7));

        assert_eq!(m.search(&k), Some((Location::new(3, 7), "a1".to_string())));
    }

    #[test]
    fn search_case_insensitive_unique() {
        let mut m = PathMap::new();
        m.insert(p2("opwKinematics", "a1"), Location::new(10, 2));

        assert_eq!(
            m.search(&p2("OPWKINEMATICS", "A1")),
            Some((Location::new(10, 2), "a1".to_string()))
        );
    }

    #[test]
    fn search_case_insensitive_ambiguous() {
        let mut m = PathMap::new();
        m.insert(p2("FOO", "bar"), Location::new(1, 1));
        m.insert(p2("foo", "bar"), Location::new(2, 2));

        assert_eq!(m.search(&p2("Foo", "BAR")), None);
    }

    #[test]
    fn search_tokenized_unique_snake_vs_camel() {
        let mut m = PathMap::new();
        m.insert(p2("userId", "a1"), Location::new(5, 9));

        assert_eq!(
            m.search(&p2("user_id", "a1")),
            Some((Location::new(5, 9), "a1".to_string()))
        );
    }

    #[test]
    fn search_tokenized_unique_separators_equivalent() {
        let mut m = PathMap::new();
        // All of these represent the same token sequence ["user","id"].
        m.insert(p2("user-id", "a1"), Location::new(7, 3));

        assert_eq!(
            m.search(&p2("user.id", "a1")),
            Some((Location::new(7, 3), "a1".to_string()))
        );
        assert_eq!(
            m.search(&p2("user id", "a1")),
            Some((Location::new(7, 3), "a1".to_string()))
        );
        assert_eq!(
            m.search(&p2("UserID", "a1")),
            Some((Location::new(7, 3), "a1".to_string()))
        );
    }

    #[test]
    fn search_tokenized_unique_digit_boundaries() {
        let mut m = PathMap::new();
        m.insert(p2("sha_256_sum", "a1"), Location::new(9, 4));

        assert_eq!(
            m.search(&p2("sha256Sum", "a1")),
            Some((Location::new(9, 4), "a1".to_string()))
        );
    }

    #[test]
    fn search_tokenized_unique_acronym_boundary() {
        let mut m = PathMap::new();
        m.insert(p2("http_server", "a1"), Location::new(11, 2));

        assert_eq!(
            m.search(&p2("HTTPServer", "a1")),
            Some((Location::new(11, 2), "a1".to_string()))
        );
    }

    #[test]
    fn search_collapsed_fallback_avoids_token_collision() {
        let mut m = PathMap::new();
        // These collide under fully-collapsed matching ("abc"), but are distinct by tokens.
        m.insert(p2("ab_c", "x"), Location::new(1, 1));
        m.insert(p2("a_bc", "x"), Location::new(2, 2));

        // Target tokenizes to ["ab","c"], so we should pick only the first.
        assert_eq!(
            m.search(&p2("abC", "x")),
            Some((Location::new(1, 1), "x".to_string()))
        );
    }

    #[test]
    fn search_collapsed_match_unique_after_token_pass_fails() {
        let mut m = PathMap::new();
        m.insert(p2("userid", "a1"), Location::new(12, 6));

        // Tokenization for "userId" yields ["user","id"], while "userid" yields ["userid"].
        // So token pass does not match; collapsed pass should still bridge it.
        assert_eq!(
            m.search(&p2("userId", "a1")),
            Some((Location::new(12, 6), "a1".to_string()))
        );
    }

    #[test]
    fn search_collapsed_match_ambiguous() {
        let mut m = PathMap::new();
        m.insert(p2("ab_c", "x"), Location::new(1, 1));
        m.insert(p2("a_bc", "x"), Location::new(2, 2));

        // Target tokenizes to ["abc"], so the token pass does not match either candidate.
        // Collapsed("abc") == "abc" matches both candidates, so the result must be ambiguous.
        assert_eq!(m.search(&p2("abc", "x")), None);
    }

    #[test]
    fn search_returns_resolved_leaf_segment_when_leaf_is_renamed() {
        let mut m = PathMap::new();
        // YAML key spelling is camelCase, path might be snake_case.
        m.insert(PathKey::empty().join("myField"), Location::new(1, 10));

        assert_eq!(
            m.search(&PathKey::empty().join("my_field")),
            Some((Location::new(1, 10), "myField".to_string()))
        );
    }

    #[test]
    fn search_strips_raw_identifier_prefix() {
        let mut m = PathMap::new();
        // Rust reserved keywords use raw identifiers in paths (`r#type`), but YAML keys are plain.
        m.insert(PathKey::empty().join("type"), Location::new(9, 3));

        assert_eq!(
            m.search(&PathKey::empty().join("r#type")),
            Some((Location::new(9, 3), "type".to_string()))
        );
    }

    #[test]
    fn search_handles_index_segments() {
        let mut m = PathMap::new();
        let path = PathKey::empty().join("items").join(2usize).join("name");
        m.insert(path.clone(), Location::new(5, 8));

        assert_eq!(
            m.search(&PathKey::empty().join("items").join(2usize).join("name")),
            Some((Location::new(5, 8), "name".to_string()))
        );
    }
}

pub(crate) struct PathRecorder {
    pub(crate) current: PathKey,
    /// Use-site (reference) locations, consistent with `Events::reference_location()`.
    pub(crate) map: PathMap,
    /// Definition-site locations (typically `Ev::location()` from `peek()`).
    pub(crate) defined: PathMap,
}

impl PathRecorder {
    pub(crate) fn new() -> Self {
        Self {
            current: PathKey::empty(),
            map: PathMap::new(),
            defined: PathMap::new(),
        }
    }
}
