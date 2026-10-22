"""Prepare and export the unit-tag serialization change without moving any refs."""
from pathlib import Path
import json
import os
import sys
from urllib.request import Request, urlopen

REPO = "bourumir-wyngs/serde-saphyr"
BASE_TREE = "b0b905c94cf270362459cacde310f1675e25f88d"
FILES = ["src/ser/serializer.rs", "tests/local_enum_tag_serialization.rs",
         "tests/tag_only_unit_roundtrip.rs", "README.md", "CHANGELOG.md"]


def replace(path, old, new):
    """Replace exactly one old fragment with new in path; fail on unexpected source."""
    p = Path(path)
    source = p.read_text()
    if source.count(old) != 1:
        raise RuntimeError(f"{path}: expected one match: {old[:100]!r}")
    p.write_text(source.replace(old, new))


def prepare():
    """Apply the serializer change and update expectations and documentation."""
    p = "src/ser/serializer.rs"
    old = '''    /// Emit pending YAML node properties before a scalar.
    #[inline]
    fn write_scalar_prefix_if_anchor(&mut self) -> Result<()> {
        let tag = self.state.pending_tag.take();
        let anchor_id = self.anchors.pending_id.take();
        if tag.is_some() || anchor_id.is_some() {
            if self.state.at_line_start {
                self.write_indent(self.state.depth)?;
            }
            if let Some(tag) = tag.as_ref() {
                self.out.write_str(&tag.emitted)?;
                self.out.write_char(' ')?;
            }
            if let Some(id) = anchor_id {
                self.anchors
                    .record_resolved_tag(id, tag.as_ref().map(|tag| tag.resolved.as_str()));
                self.out.write_char('&')?;
                self.write_anchor_name(id)?;
                self.out.write_char(' ')?;
            }
        }
        Ok(())
    }
'''
    new = '''    /// Emit pending YAML node properties before scalar content.
    #[inline]
    fn write_scalar_prefix_if_anchor(&mut self) -> Result<()> {
        self.write_node_properties(true)
    }

    /// Emit pending tags and anchors, separated by a space.
    /// `trailing_space` separates the properties from following scalar content;
    /// omit it for a tag-only unit variant or a block-node header.
    fn write_node_properties(&mut self, trailing_space: bool) -> Result<()> {
        let tag = self.state.pending_tag.take();
        let anchor_id = self.anchors.pending_id.take();
        if tag.is_some() || anchor_id.is_some() {
            if self.state.at_line_start {
                self.write_indent(self.state.depth)?;
            }
            if let Some(tag) = tag.as_ref() {
                self.out.write_str(&tag.emitted)?;
            }
            if let Some(id) = anchor_id {
                self.anchors
                    .record_resolved_tag(id, tag.as_ref().map(|tag| tag.resolved.as_str()));
                if tag.is_some() {
                    self.out.write_char(' ')?;
                }
                self.out.write_char('&')?;
                self.write_anchor_name(id)?;
            }
            if trailing_space {
                self.out.write_char(' ')?;
            }
        }
        Ok(())
    }
'''
    replace(p, old, new)
    source = Path(p).read_text()
    start = source.index("    /// Emit pending YAML node properties on their own line before a block node.")
    end = source.index("    /// Emit an alias", start)
    replace(p, source[start:end], '''    /// Emit pending YAML node properties on their own line before a block node.
    #[inline]
    fn write_anchor_for_complex_node(&mut self) -> Result<()> {
        if self.has_pending_node_properties() {
            self.write_space_if_pending()?;
            self.write_node_properties(false)?;
            self.newline()?;
        }
        Ok(())
    }

''')
    replace(p, '''            && self.pending_tag_selects_variant(variant)?
        {
            return self.serialize_unit();
        }''', '''            && self.pending_tag_selects_variant(variant)?
        {
            // The tag identifies a fieldless variant; no scalar payload is needed.
            self.write_space_if_pending()?;
            self.write_node_properties(false)?;
            return self.write_end_of_scalar();
        }''')
    p = "tests/local_enum_tag_serialization.rs"
    replace(p, "fn local_tags_serialize_unit_variants_with_null_payloads(",
               "fn local_tags_serialize_unit_variants_without_payloads(")
    replace(p, 'format!("{tag} null\\n")', 'format!("{tag}\\n")')
    replace(p, 'format!("generator: {tag} null\\n")', 'format!("generator: {tag}\\n")')
    p = "README.md"
    marker = "The standalone `!` is a separate non-specific tag"
    note = ("A matching local tag on a unit enum variant is emitted without a payload:\n"
            "`Tagged(E::Timestamp, Some(\"!timestamp\".into()))` writes `!timestamp`, not\n"
            "`!timestamp null`. The same form round-trips through `Option<E>`, nested\n"
            "options, and tagged wrappers. Reading still accepts an omitted payload or\n"
            "an explicit plain `null` or `~` for unit variants; non-null payloads are\n"
            "rejected. Variants with fields keep their payloads: for example, a tagged\n"
            "`E::Maybe(None)` writes `!maybe null`, while empty tuple and struct payloads\n"
            "retain `[]` and `{}`. Ordinary untagged enum serialization is unchanged.\n\n")
    replace(p, marker, note + marker)
    p = "CHANGELOG.md"
    source = Path(p).read_text()
    marker = "### Fixed\n\n"
    pos = source.index(marker) + len(marker)
    note = ("- Emit matching local tags on unit enum variants without a redundant `null`\n"
            "  payload, including through optional and tagged wrappers. Existing explicit\n"
            "  null payloads remain readable; actual variant fields retain their payload\n"
            "  representation. Share node-property emission to preserve anchors and spacing.\n")
    Path(p).write_text(source[:pos] + note + source[pos:])
    Path("/tmp/pr246-unit-files.json").write_text(json.dumps(FILES))


def api(path, payload):
    """Create a repository Git object and return its decoded JSON response."""
    req = Request(f"https://api.github.com/repos/{REPO}/{path}",
                  data=json.dumps(payload).encode(), method="POST",
                  headers={"Authorization": f"Bearer {os.environ['GH_TOKEN']}",
                           "Accept": "application/vnd.github+json",
                           "Content-Type": "application/json"})
    with urlopen(req, timeout=60) as response:
        return json.load(response)


def export():
    """Export the changed source tree, excluding temporary validation files."""
    entries = []
    for path in FILES:
        blob = api("git/blobs", {"content": Path(path).read_text(), "encoding": "utf-8"})
        entries.append({"path": path, "mode": "100644", "type": "blob", "sha": blob["sha"]})
    tree = api("git/trees", {"base_tree": BASE_TREE, "tree": entries})
    print("PREPARED_TREE", tree["sha"], flush=True)
    Path("/tmp/pr246-unit-tree.json").write_text(json.dumps({"tree": tree["sha"], "files": FILES}))


if __name__ == "__main__":
    if sys.argv[1:] == ["export"]:
        export()
    elif not sys.argv[1:]:
        prepare()
    else:
        raise SystemExit("expected no arguments or 'export'")
