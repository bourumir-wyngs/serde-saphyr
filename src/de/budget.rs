//! Streaming YAML budget checker using granit-parser.
//!
//! This inspects the parser's event stream and enforces simple budgets to
//! avoid pathological inputs

use crate::options::MergeKeyPolicy;
use granit_parser::{Event, Parser, ScalarStyle, ScanError, Tag};
use smallvec::SmallVec;
use std::collections::HashSet;

const DEFAULT_MAX_SCALAR_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_MAX_TOTAL_COMMENT_BYTES: usize = 64 * 1024 * 1024;

#[cfg(feature = "serde_derived_types")]
fn default_max_total_comment_bytes() -> usize {
    DEFAULT_MAX_TOTAL_COMMENT_BYTES
}

/// Budgets for a streaming YAML scan.
///
/// The defaults are intentionally permissive for typical configuration files
/// while stopping obvious resource-amplifying inputs. Tune these per your
/// application if you regularly process very large YAML streams.
///
/// Example: using a `Budget` with `from_str_with_options` to parse into a small
/// `Config` struct.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
///   name: My Application
///   enabled: true
///   retries: 5
/// "#;
///
/// let options = serde_saphyr::options! {
///     budget: serde_saphyr::budget! {
///         // Example override
///         max_documents: 2,
///     },
/// };
///
/// let cfg: Config = serde_saphyr::from_str_with_options(yaml, options).unwrap();
/// assert_eq!(cfg.retries, 5);
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(
    feature = "serde_derived_types",
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct Budget {
    /// Hard cap on the size of the input in bytes.
    /// This limit only applies to reader-based input, to avoid resource exhaustion
    /// from large malicious inputs (even when they are valid YAML). String inputs can be
    /// checked by the caller when needed because they are already in memory.
    /// If the limit is exceeded, `serde_saphyr::Error::IOError` is returned with a
    /// `std::io::Error` cause using `ErrorKind::FileTooLarge`.
    ///
    /// If set to None, this check is not active. This may be needed when reading from
    /// stream into iterator, where potentially infinite input may need to be supported.
    ///
    /// Default: 256 Mb
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub max_reader_input_bytes: Option<usize>,
    /// Maximum total parser events (counting every event).
    ///
    /// Default: 1,000,000
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub max_events: usize,
    /// Maximum number of alias (`*ref`) events allowed.
    ///
    /// Default: 50,000
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub max_aliases: usize,
    /// Maximal total number of anchors (distinct `&anchor` definitions).
    ///
    /// Default: 50,000
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub max_anchors: usize,
    /// Maximum structural nesting depth (sequences + mappings).
    ///
    /// Default: 64
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub max_depth: usize,
    /// Maximum allowed `!include` nesting depth.
    ///
    /// This limits how many nested included parsers may be active below the root
    /// document at once. A value of `0` disables includes entirely.
    ///
    /// Default: 24
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub max_inclusion_depth: u32,
    /// Maximum number of YAML documents in the stream.
    ///
    /// Default: 1,024. If enforcing policy is "per document", this is ignored.
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub max_documents: usize,
    /// Maximum number of *nodes* (SequenceStart/MappingStart/Scalar).
    ///
    /// Default: 250,000
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub max_nodes: usize,
    /// Maximum total bytes of scalar contents plus explicit tag spellings.
    ///
    /// Default: 67,108,864 (64 MiB)
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub max_total_scalar_bytes: usize,
    /// Maximum total bytes of comment contents.
    ///
    /// Comment text may be copied and retained for [`crate::Commented`] support, so this
    /// limit is enforced before deserialization stores comment text.
    ///
    /// Default: 67,108,864 (64 MiB)
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    #[cfg_attr(
        feature = "serde_derived_types",
        serde(default = "default_max_total_comment_bytes")
    )]
    pub max_total_comment_bytes: usize,
    /// Maximum number of merge keys (`<<`) allowed across the stream when merge-key
    /// expansion is enabled.
    ///
    /// Default: 10,000
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub max_merge_keys: usize,
    /// If `true`, enforce the alias/anchor heuristic.
    ///
    /// The heuristic flags inputs that use an excessive number of aliases
    /// relative to the number of defined anchors.
    ///
    /// Default: true
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub enforce_alias_anchor_ratio: bool,
    /// Minimum number of aliases required before the alias/anchor ratio
    /// heuristic is evaluated. This avoids tiny-input false positives.
    ///
    /// Default: 100
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub alias_anchor_min_aliases: usize,
    /// Multiplier used for the alias/anchor ratio heuristic. A breach occurs
    /// when `aliases > alias_anchor_ratio_multiplier * anchors` (after
    /// scanning), once [`Budget::alias_anchor_min_aliases`] is met.
    ///
    /// Default: 10
    #[deprecated(
        note = "Direct construction of `Budget` will be disabled from 1.0.0, use macro `budget!`"
    )]
    pub alias_anchor_ratio_multiplier: usize,
}

impl Default for Budget {
    #[allow(deprecated)]
    fn default() -> Self {
        Self {
            max_reader_input_bytes: Some(256 * 1024 * 1024), // 256 Mb
            max_events: 1_000_000,                           // plenty for normal configs
            max_aliases: 50_000,                             // liberal absolute cap
            max_anchors: 50_000,
            max_depth: 64, // protects stack/CPU
            max_inclusion_depth: 24,
            max_documents: 1_024, // doc separator storms
            max_nodes: 250_000,   // sequences + maps + scalars
            max_total_scalar_bytes: DEFAULT_MAX_SCALAR_BYTES, // 64 MiB of scalar text
            max_total_comment_bytes: DEFAULT_MAX_TOTAL_COMMENT_BYTES, // 64 MiB of comment text
            max_merge_keys: 10_000, // generous cap for merge keys
            enforce_alias_anchor_ratio: true,
            alias_anchor_min_aliases: 100,
            alias_anchor_ratio_multiplier: 10,
        }
    }
}

/// What tripped the budget (if anything).
#[non_exhaustive]
#[derive(Clone, Debug)]
#[cfg_attr(
    feature = "serde_derived_types",
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum BudgetBreach {
    /// The total number of parser events exceeded [`Budget::max_events`].
    Events {
        /// Total events observed at the moment of the breach.
        events: usize,
    },

    /// The number of alias events (`*ref`) exceeded [`Budget::max_aliases`].
    Aliases {
        /// Total alias events observed at the moment of the breach.
        aliases: usize,
    },

    /// The number of distinct anchors defined exceeded [`Budget::max_anchors`].
    Anchors {
        /// Total distinct anchors observed at the moment of the breach.
        anchors: usize,
    },

    /// The structural nesting depth exceeded [`Budget::max_depth`].
    ///
    /// Depth counts nested `SequenceStart` and `MappingStart` events.
    Depth {
        /// Maximum depth reached when the breach occurred.
        depth: usize,
    },

    /// The include nesting depth exceeded [`Budget::max_inclusion_depth`].
    InclusionDepth {
        /// Include nesting depth reached when the breach occurred.
        depth: u32,
    },

    /// The number of YAML documents exceeded [`Budget::max_documents`].
    Documents {
        /// Total documents observed at the moment of the breach.
        documents: usize,
    },

    /// The number of nodes exceeded [`Budget::max_nodes`].
    ///
    /// Nodes are `SequenceStart`, `MappingStart`, and `Scalar` events.
    Nodes {
        /// Total nodes observed at the moment of the breach.
        nodes: usize,
    },

    /// The cumulative size of scalar contents exceeded [`Budget::max_total_scalar_bytes`].
    ScalarBytes {
        /// Sum of `Scalar.value.len()` over all scalars seen so far.
        total_scalar_bytes: usize,
    },

    /// The cumulative size of comment contents exceeded [`Budget::max_total_comment_bytes`].
    CommentBytes {
        /// Sum of comment text lengths over all comments seen so far.
        total_comment_bytes: usize,
    },

    /// The number of merge keys (`<<`) exceeded [`Budget::max_merge_keys`].
    MergeKeys {
        /// Total merge keys observed at the moment of the breach.
        merge_keys: usize,
    },

    /// The ratio of aliases to defined anchors is excessive.
    ///
    /// Triggered when [`Budget::enforce_alias_anchor_ratio`] is true and
    /// `aliases > alias_anchor_ratio_multiplier × anchors` (after scanning),
    /// once `aliases >= alias_anchor_min_aliases` to avoid tiny-input
    /// false positives.
    AliasAnchorRatio {
        /// Total alias events seen.
        aliases: usize,
        /// Total distinct anchors defined (by id) in the input.
        anchors: usize,
    },

    /// Unbalanced structure: a closing event was encountered without a matching
    /// opening event (depth underflow). Indicates malformed or truncated input.
    SequenceUnbalanced,

    /// The total number of input bytes exceeded [`Budget::max_reader_input_bytes`].
    InputBytes {
        /// Total number of bytes consumed from the input when the breach occurred.
        input_bytes: usize,
    },
}

/// Summary of the scan (even if no breach).
#[derive(Clone, Debug, Default)]
#[cfg_attr(
    feature = "serde_derived_types",
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct BudgetReport {
    /// `Some(..)` if a limit was exceeded; `None` if all budgets were respected.
    pub breached: Option<BudgetBreach>,

    /// Total number of parser events observed.
    pub events: usize,

    /// Total number of alias events (`*ref`).
    pub aliases: usize,

    /// Total number of distinct anchors that were defined (by id).
    pub anchors: usize,

    /// Total number of YAML documents in the stream.
    pub documents: usize,

    /// Total number of nodes encountered (scalars + sequence starts + mapping starts).
    pub nodes: usize,

    /// Maximum structural nesting depth reached at any point in the stream.
    pub max_depth: usize,

    /// Sum of bytes across all scalar values (`Scalar.value.len()`), saturating on overflow.
    pub total_scalar_bytes: usize,

    /// Sum of bytes across all comment values, saturating on overflow.
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub total_comment_bytes: usize,

    /// Total number of merge keys (`<<`) encountered while merge-key expansion was enabled.
    pub merge_keys: usize,
}

impl BudgetReport {
    fn reset(&mut self) {
        // Resets all fields except "breached" and document count.
        self.events = 0;
        self.aliases = 0;
        self.anchors = 0;
        self.nodes = 0;
        self.max_depth = 0;
        self.total_scalar_bytes = 0;
        self.total_comment_bytes = 0;
        self.merge_keys = 0;
    }
}

/// Defines how budget limit policies are enforces (per document or for all content).
/// Default is for all content, except when streaming from reader to iterator where
/// it is per document as infinite may be required.
#[non_exhaustive]
#[derive(Debug, PartialEq)]
pub enum EnforcingPolicy {
    AllContent,
    PerDocument,
}

/// Stateful helper that enforces a [`Budget`] while consuming a stream of [`Event`]s.
#[derive(Debug)]
pub(crate) struct BudgetEnforcer {
    budget: Budget,
    report: BudgetReport,
    depth: usize,
    defined_anchors: HashSet<usize>,
    containers: SmallVec<[ContainerState; 64]>,
    policy: EnforcingPolicy,
    merge_keys: MergeKeyPolicy,
}

#[derive(Clone, Copy, Debug)]
enum ContainerState {
    Sequence {
        from_mapping_value: bool,
    },
    Mapping {
        expecting_key: bool,
        from_mapping_value: bool,
    },
}

#[derive(Clone, Copy)]
enum ContainerKind {
    Sequence,
    Mapping,
}

fn tag_display_len(tag: Option<&Tag>) -> usize {
    tag.map_or(0, |tag| {
        let (handle, suffix) = tag.parts();
        handle.len().saturating_add(suffix.len())
    })
}

impl BudgetEnforcer {
    /// Create a new enforcer for the provided `budget`.
    pub(crate) fn new(budget: Budget, policy: EnforcingPolicy, merge_keys: MergeKeyPolicy) -> Self {
        Self {
            budget,
            report: BudgetReport::default(),
            depth: 0,
            defined_anchors: HashSet::with_capacity(256),
            containers: SmallVec::new(),
            policy,
            merge_keys,
        }
    }

    /// Observe a parser [`Event`], updating the internal counters.
    ///
    /// Returns `Err(BudgetBreach)` as soon as a limit is exceeded.
    pub fn observe(&mut self, ev: &Event) -> Result<(), BudgetBreach> {
        if let Some(breach) = self.report.breached.as_ref() {
            return Err(breach.clone());
        }
        let result = self.observe_inner(ev);
        self.remember_breach(&result);
        result
    }

    fn observe_inner(&mut self, ev: &Event) -> Result<(), BudgetBreach> {
        self.report.events += 1;
        if self.report.events > self.budget.max_events {
            return Err(BudgetBreach::Events {
                events: self.report.events,
            });
        }

        match ev {
            Event::Scalar(value, style, anchor_id, tag_opt) => {
                self.bump_nodes()?;
                self.bump_total_scalar_bytes(
                    value
                        .len()
                        .saturating_add(tag_display_len(tag_opt.as_deref())),
                )?;
                self.record_anchor(*anchor_id)?;
                self.handle_scalar(value, style, tag_opt.is_some())?;
            }
            Event::MappingStart(_style, anchor_id, tag_opt) => {
                self.enter_container(*anchor_id, tag_opt.as_deref(), |from_mapping_value| {
                    ContainerState::Mapping {
                        expecting_key: true,
                        from_mapping_value,
                    }
                })?;
            }
            Event::MappingEnd => {
                self.leave_container(ContainerKind::Mapping)?;
            }
            Event::SequenceStart(_style, anchor_id, tag_opt) => {
                self.enter_container(*anchor_id, tag_opt.as_deref(), |from_mapping_value| {
                    ContainerState::Sequence { from_mapping_value }
                })?;
            }
            Event::SequenceEnd => {
                self.leave_container(ContainerKind::Sequence)?;
            }
            Event::Alias(_anchor_id) => {
                self.observe_alias_event(true)?;
            }
            Event::DocumentStart(..) => {
                if self.policy == EnforcingPolicy::PerDocument {
                    self.finalize_document()?;
                    self.report.reset();
                    self.defined_anchors.clear();
                } else {
                    self.report.documents += 1;
                    if self.report.documents > self.budget.max_documents {
                        return Err(BudgetBreach::Documents {
                            documents: self.report.documents,
                        });
                    }
                }
            }
            Event::DocumentEnd => {}
            Event::Comment(text, _) => {
                self.report.total_comment_bytes =
                    self.report.total_comment_bytes.saturating_add(text.len());
                if self.report.total_comment_bytes > self.budget.max_total_comment_bytes {
                    return Err(BudgetBreach::CommentBytes {
                        total_comment_bytes: self.report.total_comment_bytes,
                    });
                }
            }
            Event::Nothing => {}
            Event::StreamStart | Event::StreamEnd => {}
        }

        Ok(())
    }

    /// Observe an alias token from the parser stream without advancing mapping
    /// key/value state.
    ///
    /// This is used by `LiveEvents`, where alias references are immediately
    /// expanded into replayed events that already advance mapping state.
    pub(crate) fn observe_alias_reference(&mut self) -> Result<(), BudgetBreach> {
        if let Some(breach) = self.report.breached.as_ref() {
            return Err(breach.clone());
        }
        let result = self.observe_alias_reference_inner();
        self.remember_breach(&result);
        result
    }

    fn observe_alias_reference_inner(&mut self) -> Result<(), BudgetBreach> {
        self.report.events += 1;
        if self.report.events > self.budget.max_events {
            return Err(BudgetBreach::Events {
                events: self.report.events,
            });
        }
        self.observe_alias_event(false)
    }

    fn remember_breach<T>(&mut self, result: &Result<T, BudgetBreach>) {
        if self.report.breached.is_none()
            && let Err(breach) = result
        {
            self.report.breached = Some(breach.clone());
        }
    }

    fn observe_alias_event(&mut self, advance_mapping_state: bool) -> Result<(), BudgetBreach> {
        self.report.aliases += 1;
        if self.report.aliases > self.budget.max_aliases {
            return Err(BudgetBreach::Aliases {
                aliases: self.report.aliases,
            });
        }
        if advance_mapping_state {
            self.handle_alias();
        }
        Ok(())
    }

    fn bump_nodes(&mut self) -> Result<(), BudgetBreach> {
        self.report.nodes += 1;
        if self.report.nodes > self.budget.max_nodes {
            return Err(BudgetBreach::Nodes {
                nodes: self.report.nodes,
            });
        }
        Ok(())
    }

    fn bump_total_scalar_bytes(&mut self, bytes: usize) -> Result<(), BudgetBreach> {
        self.report.total_scalar_bytes = self.report.total_scalar_bytes.saturating_add(bytes);
        if self.report.total_scalar_bytes > self.budget.max_total_scalar_bytes {
            return Err(BudgetBreach::ScalarBytes {
                total_scalar_bytes: self.report.total_scalar_bytes,
            });
        }
        Ok(())
    }

    // Track structural nesting before a mapping or sequence is pushed.
    fn enter_depth(&mut self) -> Result<(), BudgetBreach> {
        self.depth = self.depth.saturating_add(1);
        if self.depth > self.report.max_depth {
            self.report.max_depth = self.depth;
        }
        if self.report.max_depth > self.budget.max_depth {
            return Err(BudgetBreach::Depth {
                depth: self.report.max_depth,
            });
        }
        Ok(())
    }

    // Apply all shared accounting for a mapping or sequence start event.
    fn enter_container(
        &mut self,
        anchor_id: usize,
        tag: Option<&Tag>,
        container: impl FnOnce(bool) -> ContainerState,
    ) -> Result<(), BudgetBreach> {
        self.bump_nodes()?;
        self.enter_depth()?;
        self.bump_total_scalar_bytes(tag_display_len(tag))?;
        let from_mapping_value = self.entering_container();
        self.containers.push(container(from_mapping_value));
        self.record_anchor(anchor_id)
    }

    fn record_anchor(&mut self, anchor_id: usize) -> Result<(), BudgetBreach> {
        if anchor_id != 0 && self.defined_anchors.insert(anchor_id) {
            let count = self.defined_anchors.len();
            if count > self.budget.max_anchors {
                self.report.anchors = count;
                return Err(BudgetBreach::Anchors { anchors: count });
            }
        }
        self.report.anchors = self.defined_anchors.len();
        Ok(())
    }

    fn handle_scalar(
        &mut self,
        value: &str,
        style: &ScalarStyle,
        has_tag: bool,
    ) -> Result<(), BudgetBreach> {
        if let Some(ContainerState::Mapping { expecting_key, .. }) = self.containers.last_mut() {
            if *expecting_key {
                if matches!(self.merge_keys, MergeKeyPolicy::Merge)
                    && !has_tag
                    && matches!(style, ScalarStyle::Plain)
                    && value == "<<"
                {
                    self.report.merge_keys += 1;
                    if self.report.merge_keys > self.budget.max_merge_keys {
                        return Err(BudgetBreach::MergeKeys {
                            merge_keys: self.report.merge_keys,
                        });
                    }
                }
                *expecting_key = false;
            } else {
                self.finish_value();
            }
        }
        Ok(())
    }

    fn handle_alias(&mut self) {
        if let Some(ContainerState::Mapping { expecting_key, .. }) = self.containers.last_mut() {
            if *expecting_key {
                *expecting_key = false;
            } else {
                self.finish_value();
            }
        }
    }

    fn entering_container(&mut self) -> bool {
        if let Some(ContainerState::Mapping { expecting_key, .. }) = self.containers.last_mut() {
            if *expecting_key {
                *expecting_key = false;
                false
            } else {
                true
            }
        } else {
            false
        }
    }

    // Pop the expected container and complete its parent mapping value if needed.
    fn leave_container(&mut self, expected: ContainerKind) -> Result<(), BudgetBreach> {
        self.depth = self
            .depth
            .checked_sub(1)
            .ok_or(BudgetBreach::SequenceUnbalanced)?;

        let from_mapping_value = match (expected, self.containers.pop()) {
            (ContainerKind::Sequence, Some(ContainerState::Sequence { from_mapping_value }))
            | (
                ContainerKind::Mapping,
                Some(ContainerState::Mapping {
                    from_mapping_value, ..
                }),
            ) => from_mapping_value,
            _ => return Err(BudgetBreach::SequenceUnbalanced),
        };

        if from_mapping_value {
            self.finish_value();
        }
        Ok(())
    }

    fn finish_value(&mut self) {
        if let Some(ContainerState::Mapping { expecting_key, .. }) = self.containers.last_mut() {
            *expecting_key = true;
        }
    }

    /// Consume the enforcer and return the accumulated [`BudgetReport`].
    ///
    /// This should be used after a breach has already been detected.
    pub fn into_report(mut self) -> BudgetReport {
        self.report.anchors = self.defined_anchors.len();
        self.report
    }

    /// Finalize the current document when limits are enforced per document.
    ///
    /// Unlike [`BudgetEnforcer::finalize`], this keeps the enforcer alive so it
    /// can be reset and reused for the next document in a stream.
    pub(crate) fn finalize_document(&mut self) -> Result<(), BudgetBreach> {
        if let Some(breach) = self.report.breached.as_ref() {
            return Err(breach.clone());
        }
        if self.policy == EnforcingPolicy::PerDocument {
            self.check_post_scan_heuristics()?;
        }
        Ok(())
    }

    fn check_post_scan_heuristics(&mut self) -> Result<(), BudgetBreach> {
        self.report.anchors = self.defined_anchors.len();

        if self.budget.enforce_alias_anchor_ratio
            && self.report.aliases >= self.budget.alias_anchor_min_aliases
            && (self.report.anchors == 0
                || self.report.aliases
                    > self
                        .budget
                        .alias_anchor_ratio_multiplier
                        .saturating_mul(self.report.anchors))
        {
            let breach = BudgetBreach::AliasAnchorRatio {
                aliases: self.report.aliases,
                anchors: self.report.anchors,
            };
            self.report.breached = Some(breach.clone());
            return Err(breach);
        }

        Ok(())
    }

    /// Finalize the enforcement, performing post-scan heuristics (like alias/anchor ratio).
    pub fn finalize(mut self) -> BudgetReport {
        if self.report.breached.is_none() {
            let _ = self.check_post_scan_heuristics();
        }
        self.report
    }
}

/// Check an input `&str` against the given `Budget`.
///
/// Parameters:
/// - `input`: YAML text (UTF-8). If you accept non-UTF-8, transcode before calling.
/// - `budget`: limits to enforce (see [`Budget`]).
///
/// Returns:
/// - `Ok(report)` — `report.breached.is_none()` means **within budget**.
///   If `report.breached.is_some()`, you should **reject** the input.
/// - `Err(ScanError)` — scanning (lexing/parsing) failed.
///
/// Note:
/// - This is **streaming** and does not allocate a DOM.
/// - Depth counts nested `SequenceStart` and `MappingStart`.
/// - Standalone budget checks do not receive [`Options`](crate::Options);
///   merge keys are counted as if merge expansion is enabled, preserving
///   historical behavior.
pub fn check_yaml_budget(
    input: &str,
    budget: Budget,
    policy: EnforcingPolicy,
) -> Result<BudgetReport, ScanError> {
    let parser = Parser::new_from_str(input);
    let mut enforcer = BudgetEnforcer::new(budget, policy, MergeKeyPolicy::Merge);

    for item in parser {
        let (ev, _span) = item?;
        if enforcer.observe(&ev).is_err() {
            return Ok(enforcer.into_report());
        }
    }

    Ok(enforcer.finalize())
}

/// Convenience wrapper that returns `true` if the YAML **exceeds** any budget.
///
/// Parameters:
/// - `input`: YAML text (UTF-8).
/// - `budget`: limits to enforce.
///
/// Returns:
/// - `Ok(true)` if a budget was exceeded (reject).
/// - `Ok(false)` if within budget.
/// - `Err(ScanError)` on parser error.
///
/// Despite the (historical) name, this function only scans the event stream and reports
/// whether a budget was exceeded; it does not deserialize a YAML value.
pub fn parse_yaml(input: &str, budget: Budget) -> Result<bool, ScanError> {
    let report = check_yaml_budget(input, budget, EnforcingPolicy::AllContent)?;
    Ok(report.breached.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiny_yaml_ok() {
        let b = Budget::default();
        let y = "a: [1, 2, 3]\n";
        let r = check_yaml_budget(y, b, EnforcingPolicy::AllContent).unwrap();
        assert!(r.breached.is_none());
        assert_eq!(r.documents, 1);
        assert!(r.nodes > 0);
    }

    #[test]
    fn alias_bomb_trips_alias_limit() {
        // A toy alias-bomb-ish input (not huge, just to exercise the check).
        let y = r#"root: &A [1, 2]
a: *A
b: *A
c: *A
d: *A
e: *A
"#;

        let b = Budget {
            max_aliases: 3, // set a tiny limit for the test
            ..Default::default()
        };

        let rep = check_yaml_budget(y, b, EnforcingPolicy::AllContent).unwrap();
        assert!(matches!(rep.breached, Some(BudgetBreach::Aliases { .. })));
    }

    #[test]
    fn finalization_preserves_an_immediate_breach() {
        let budget = Budget {
            max_aliases: 0,
            alias_anchor_min_aliases: 1,
            ..Default::default()
        };
        let mut enforcer =
            BudgetEnforcer::new(budget, EnforcingPolicy::AllContent, MergeKeyPolicy::Merge);

        let breach = enforcer.observe_alias_reference().unwrap_err();
        assert!(matches!(breach, BudgetBreach::Aliases { aliases: 1 }));

        let report = enforcer.finalize();
        assert!(matches!(
            report.breached,
            Some(BudgetBreach::Aliases { aliases: 1 })
        ));
    }

    #[test]
    fn deep_nesting_trips_depth() {
        let mut y = String::new();
        // Keep nesting below saphyr's internal recursion ceiling to ensure
        // the budget check, not the parser, trips first.
        for _ in 0..200 {
            y.push('[');
        }
        for _ in 0..200 {
            y.push(']');
        }

        let b = Budget {
            max_depth: 150,
            ..Default::default()
        };

        let rep = check_yaml_budget(&y, b, EnforcingPolicy::AllContent).unwrap();
        assert!(matches!(rep.breached, Some(BudgetBreach::Depth { .. })));
    }

    #[test]
    fn anchors_limit_trips() {
        // Three distinct anchors defined on scalar nodes
        let y = "a: &A 1\nb: &B 2\nc: &C 3\n";
        let b = Budget {
            max_anchors: 2,
            ..Default::default()
        };
        let rep = check_yaml_budget(y, b, EnforcingPolicy::AllContent).unwrap();
        assert!(matches!(
            rep.breached,
            Some(BudgetBreach::Anchors { anchors: 3 })
        ));
    }

    #[test]
    fn merge_key_limit_trips() {
        let mut y = String::from("base: &B\n  k: 1\nitems:\n");
        for idx in 0..3 {
            y.push_str(&format!("  item{idx}:\n    <<: *B\n    extra: {idx}\n"));
        }

        let b = Budget {
            max_merge_keys: 2,
            ..Default::default()
        };

        let rep = check_yaml_budget(&y, b, EnforcingPolicy::AllContent).unwrap();
        assert!(matches!(
            rep.breached,
            Some(BudgetBreach::MergeKeys { merge_keys }) if merge_keys == 3
        ));
        assert_eq!(rep.merge_keys, 3);
    }

    #[test]
    fn merge_key_limit_is_ignored_when_policy_is_as_ordinary() {
        let y = "base: &B\n  k: 1\nroot:\n  <<: *B\n";
        let budget = Budget {
            max_merge_keys: 0,
            ..Default::default()
        };
        let mut enforcer = BudgetEnforcer::new(
            budget,
            EnforcingPolicy::AllContent,
            MergeKeyPolicy::AsOrdinary,
        );

        for item in Parser::new_from_str(y) {
            let (event, _span) = item.unwrap();
            enforcer.observe(&event).unwrap();
        }

        let report = enforcer.finalize();
        assert!(report.breached.is_none());
        assert_eq!(report.merge_keys, 0);
    }

    #[test]
    fn alias_anchor_ratio_trips_when_excessive() {
        let yaml = "root: &A [1]\na: *A\nb: *A\nc: *A\n";

        let budget = Budget {
            alias_anchor_min_aliases: 1,
            alias_anchor_ratio_multiplier: 2,
            ..Default::default()
        };

        let report = check_yaml_budget(yaml, budget, EnforcingPolicy::AllContent).unwrap();
        assert!(matches!(
            report.breached,
            Some(BudgetBreach::AliasAnchorRatio {
                aliases: 3,
                anchors: 1
            })
        ));
        assert_eq!(report.aliases, 3);
        assert_eq!(report.anchors, 1);
    }

    #[test]
    fn per_document_alias_anchor_ratio_is_checked_before_reset() {
        let yaml = concat!(
            "---\n",
            "root: &A [1]\n",
            "a: *A\n",
            "b: *A\n",
            "c: *A\n",
            "---\n",
            "root: &B [2]\n",
        );
        let budget = Budget {
            alias_anchor_min_aliases: 1,
            alias_anchor_ratio_multiplier: 2,
            ..Default::default()
        };

        let report = check_yaml_budget(yaml, budget, EnforcingPolicy::PerDocument).unwrap();

        assert!(matches!(
            report.breached,
            Some(BudgetBreach::AliasAnchorRatio {
                aliases: 3,
                anchors: 1
            })
        ));
    }

    #[test]
    fn alias_anchor_ratio_respects_minimum_alias_threshold() {
        let yaml = "root: &A [1]\na: *A\nb: *A\nc: *A\n";

        let budget = Budget {
            alias_anchor_min_aliases: 5,
            alias_anchor_ratio_multiplier: 1,
            ..Default::default()
        };

        let report = check_yaml_budget(yaml, budget, EnforcingPolicy::AllContent).unwrap();
        assert!(report.breached.is_none());
        assert_eq!(report.aliases, 3);
        assert_eq!(report.anchors, 1);
    }

    #[test]
    fn alias_anchor_ratio_multiplier_overflow_does_not_panic() {
        let budget = Budget {
            alias_anchor_min_aliases: 1,
            alias_anchor_ratio_multiplier: usize::MAX,
            ..Default::default()
        };
        let mut enforcer =
            BudgetEnforcer::new(budget, EnforcingPolicy::AllContent, MergeKeyPolicy::Merge);
        enforcer.report.aliases = usize::MAX;
        enforcer.defined_anchors.insert(1);
        enforcer.defined_anchors.insert(2);

        let report = enforcer.finalize();

        assert!(report.breached.is_none());
        assert_eq!(report.aliases, usize::MAX);
        assert_eq!(report.anchors, 2);
    }

    #[test]
    fn budget_default_sets_max_inclusion_depth() {
        let budget = Budget::default();

        assert_eq!(budget.max_inclusion_depth, 24);
    }

    #[test]
    fn scalar_budget_counts_tag_bytes() {
        let yaml = "root: !!str tagged\n";
        let budget = Budget {
            max_total_scalar_bytes: 14,
            ..Default::default()
        };

        let report = check_yaml_budget(yaml, budget, EnforcingPolicy::AllContent).unwrap();
        assert!(matches!(
            report.breached,
            Some(BudgetBreach::ScalarBytes {
                total_scalar_bytes
            }) if total_scalar_bytes > 14
        ));
    }

    #[test]
    fn scalar_budget_counts_container_tag_bytes() {
        for yaml in ["root: !!seq [a]\n", "root: !!map {a: b}\n"] {
            let budget = Budget {
                max_total_scalar_bytes: 24,
                ..Default::default()
            };

            let report = check_yaml_budget(yaml, budget, EnforcingPolicy::AllContent).unwrap();
            assert!(
                matches!(
                    report.breached,
                    Some(BudgetBreach::ScalarBytes {
                        total_scalar_bytes
                    }) if total_scalar_bytes > 24
                ),
                "yaml: {yaml:?}, report: {report:?}"
            );
        }
    }

    #[test]
    fn tag_display_len_matches_display_without_allocating() {
        for tag in [
            Tag::with_original_handle("tag:yaml.org,2002:", "str", "!!"),
            Tag::with_original_handle("!", "local", "!"),
            Tag::with_original_handle("", "tag:example.com,2000:thing", ""),
        ] {
            assert_eq!(tag_display_len(Some(&tag)), tag.to_string().len());
        }
    }

    #[test]
    fn comment_budget_counts_comment_bytes() {
        let yaml = "#abcdef\nroot: ok\n";
        let budget = Budget {
            max_total_comment_bytes: 5,
            ..Default::default()
        };

        let report = check_yaml_budget(yaml, budget, EnforcingPolicy::AllContent).unwrap();
        assert!(matches!(
            report.breached,
            Some(BudgetBreach::CommentBytes {
                total_comment_bytes
            }) if total_comment_bytes > 5
        ));
        assert_eq!(report.total_scalar_bytes, 0);
    }
}
