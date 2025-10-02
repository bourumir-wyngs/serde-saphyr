//! Streaming YAML budget checker using saphyr-parser (YAML 1.2).
//!
//! This inspects the parser's event stream and enforces simple budgets to
//! avoid pathological inputs

use std::borrow::Cow;
use std::collections::HashSet;

use saphyr_parser::{Event, Parser, ScalarStyle, ScanError};

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
/// name: My Application
/// enabled: true
/// retries: 5
/// "#;
///
/// let mut options = serde_saphyr::Options::default();
/// options.budget = Some(serde_saphyr::Budget::default());
///
/// let cfg: Config = serde_saphyr::from_str_with_options(yaml, options).unwrap();
/// assert_eq!(cfg.retries, 5);
/// ```
#[derive(Clone, Debug)]
pub struct Budget {
    /// Maximum total parser events (counting every event).
    ///
    /// Default: 1,000,000
    pub max_events: usize,
    /// Maximum number of alias (`*ref`) events allowed.
    ///
    /// Default: 50,000
    pub max_aliases: usize,
    /// Maximal total number of anchors (distinct `&anchor` definitions).
    ///
    /// Default: 50,000
    pub max_anchors: usize,
    /// Maximum structural nesting depth (sequences + mappings).
    ///
    /// Default: 2,000
    pub max_depth: usize,
    /// Maximum number of YAML documents in the stream.
    ///
    /// Default: 1,024
    pub max_documents: usize,
    /// Maximum number of *nodes* (SequenceStart/MappingStart/Scalar).
    ///
    /// Default: 250,000
    pub max_nodes: usize,
    /// Maximum total bytes of scalar contents (sum of `Scalar.value.len()`).
    ///
    /// Default: 67,108,864 (64 MiB)
    pub max_total_scalar_bytes: usize,
    /// Maximum number of merge keys (`<<`) allowed across the stream.
    ///
    /// Default: 10,000
    pub max_merge_keys: usize,
    /// If `true`, enforce the alias/anchor heuristic.
    ///
    /// The heuristic flags inputs that use an excessive number of aliases
    /// relative to the number of defined anchors.
    ///
    /// Default: true
    pub enforce_alias_anchor_ratio: bool,
    /// Minimum number of aliases required before the alias/anchor ratio
    /// heuristic is evaluated. This avoids tiny-input false positives.
    ///
    /// Default: 100
    pub alias_anchor_min_aliases: usize,
    /// Multiplier used for the alias/anchor ratio heuristic. A breach occurs
    /// when `aliases > alias_anchor_ratio_multiplier * anchors` (after
    /// scanning), once [`Budget::alias_anchor_min_aliases`] is met.
    ///
    /// Default: 10
    pub alias_anchor_ratio_multiplier: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_events: 1_000_000, // plenty for normal configs
            max_aliases: 50_000,   // liberal absolute cap
            max_anchors: 50_000,
            max_depth: 2_000,                         // protects stack/CPU
            max_documents: 1_024,                     // doc separator storms
            max_nodes: 250_000,                       // sequences + maps + scalars
            max_total_scalar_bytes: 64 * 1024 * 1024, // 64 MiB of scalar text
            max_merge_keys: 10_000,                   // generous cap for merge keys
            enforce_alias_anchor_ratio: true,
            alias_anchor_min_aliases: 100,
            alias_anchor_ratio_multiplier: 10,
        }
    }
}

/// What tripped the budget (if anything).
#[derive(Clone, Debug)]
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
}

/// Summary of the scan (even if no breach).
#[derive(Clone, Debug, Default)]
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

    /// Total number of merge keys (`<<`) encountered.
    pub merge_keys: usize,
}

/// Stateful helper that enforces a [`Budget`] while consuming a stream of [`Event`]s.
#[derive(Debug)]
pub struct BudgetEnforcer {
    budget: Budget,
    report: BudgetReport,
    depth: usize,
    defined_anchors: HashSet<usize>,
    containers: Vec<ContainerState>,
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

impl BudgetEnforcer {
    /// Create a new enforcer for the provided `budget`.
    pub fn new(budget: Budget) -> Self {
        Self {
            budget,
            report: BudgetReport::default(),
            depth: 0,
            defined_anchors: HashSet::with_capacity(256),
            containers: Vec::with_capacity(64),
        }
    }

    /// Observe a parser [`Event`], updating the internal counters.
    ///
    /// Returns `Err(BudgetBreach)` as soon as a limit is exceeded.
    pub fn observe(&mut self, ev: &Event) -> Result<(), BudgetBreach> {
        self.report.events += 1;
        if self.report.events > self.budget.max_events {
            return Err(BudgetBreach::Events {
                events: self.report.events,
            });
        }

        match ev {
            Event::StreamStart | Event::StreamEnd => {}
            Event::DocumentStart(_explicit) => {
                self.report.documents += 1;
                if self.report.documents > self.budget.max_documents {
                    return Err(BudgetBreach::Documents {
                        documents: self.report.documents,
                    });
                }
            }
            Event::DocumentEnd => {}
            Event::Alias(_anchor_id) => {
                self.report.aliases += 1;
                if self.report.aliases > self.budget.max_aliases {
                    return Err(BudgetBreach::Aliases {
                        aliases: self.report.aliases,
                    });
                }
                self.handle_alias();
            }
            Event::Scalar(value, style, anchor_id, tag_opt) => {
                self.bump_nodes()?;
                let len = match value {
                    Cow::Borrowed(s) => s.len(),
                    Cow::Owned(s) => s.len(),
                };
                self.report.total_scalar_bytes = self.report.total_scalar_bytes.saturating_add(len);
                if self.report.total_scalar_bytes > self.budget.max_total_scalar_bytes {
                    return Err(BudgetBreach::ScalarBytes {
                        total_scalar_bytes: self.report.total_scalar_bytes,
                    });
                }
                self.record_anchor(*anchor_id)?;
                self.handle_scalar(value, style, tag_opt.is_some())?;
            }
            Event::SequenceStart(anchor_id, _tag_opt) => {
                self.bump_nodes()?;
                self.depth = self.depth.saturating_add(1);
                if self.depth > self.report.max_depth {
                    self.report.max_depth = self.depth;
                }
                if self.report.max_depth > self.budget.max_depth {
                    return Err(BudgetBreach::Depth {
                        depth: self.report.max_depth,
                    });
                }
                let from_mapping_value = self.entering_container();
                self.containers
                    .push(ContainerState::Sequence { from_mapping_value });
                self.record_anchor(*anchor_id)?;
            }
            Event::SequenceEnd => {
                if let Some(new_depth) = self.depth.checked_sub(1) {
                    self.depth = new_depth;
                } else {
                    return Err(BudgetBreach::SequenceUnbalanced);
                }
                self.leave_sequence()?;
            }
            Event::MappingStart(anchor_id, _tag_opt) => {
                self.bump_nodes()?;
                self.depth = self.depth.saturating_add(1);
                if self.depth > self.report.max_depth {
                    self.report.max_depth = self.depth;
                }
                if self.report.max_depth > self.budget.max_depth {
                    return Err(BudgetBreach::Depth {
                        depth: self.report.max_depth,
                    });
                }
                let from_mapping_value = self.entering_container();
                self.containers.push(ContainerState::Mapping {
                    expecting_key: true,
                    from_mapping_value,
                });
                self.record_anchor(*anchor_id)?;
            }
            Event::MappingEnd => {
                if let Some(new_depth) = self.depth.checked_sub(1) {
                    self.depth = new_depth;
                } else {
                    return Err(BudgetBreach::SequenceUnbalanced);
                }
                self.leave_mapping()?;
            }
            Event::Nothing => {}
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

    fn record_anchor(&mut self, anchor_id: usize) -> Result<(), BudgetBreach> {
        if anchor_id != 0 {
            if self.defined_anchors.insert(anchor_id) {
                let count = self.defined_anchors.len();
                if count > self.budget.max_anchors {
                    self.report.anchors = count;
                    return Err(BudgetBreach::Anchors { anchors: count });
                }
            }
        }
        self.report.anchors = self.defined_anchors.len();
        Ok(())
    }

    fn handle_scalar(
        &mut self,
        value: &Cow<'_, str>,
        style: &ScalarStyle,
        has_tag: bool,
    ) -> Result<(), BudgetBreach> {
        if let Some(ContainerState::Mapping { expecting_key, .. }) = self.containers.last_mut() {
            if *expecting_key {
                if !has_tag && matches!(style, ScalarStyle::Plain) && value.as_ref() == "<<" {
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

    fn leave_sequence(&mut self) -> Result<(), BudgetBreach> {
        match self.containers.pop() {
            Some(ContainerState::Sequence { from_mapping_value }) => {
                if from_mapping_value {
                    self.finish_value();
                }
                Ok(())
            }
            _ => Err(BudgetBreach::SequenceUnbalanced),
        }
    }

    fn leave_mapping(&mut self) -> Result<(), BudgetBreach> {
        match self.containers.pop() {
            Some(ContainerState::Mapping {
                from_mapping_value, ..
            }) => {
                if from_mapping_value {
                    self.finish_value();
                }
                Ok(())
            }
            _ => Err(BudgetBreach::SequenceUnbalanced),
        }
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

    /// Finalize the enforcement, performing post-scan heuristics (like alias/anchor ratio).
    pub fn finalize(mut self) -> BudgetReport {
        self.report.anchors = self.defined_anchors.len();

        if self.budget.enforce_alias_anchor_ratio
            && self.report.aliases >= self.budget.alias_anchor_min_aliases
        {
            if self.report.anchors == 0
                || self.report.aliases
                    > self.budget.alias_anchor_ratio_multiplier * self.report.anchors
            {
                self.report.breached = Some(BudgetBreach::AliasAnchorRatio {
                    aliases: self.report.aliases,
                    anchors: self.report.anchors,
                });
            }
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
pub fn check_yaml_budget(input: &str, budget: &Budget) -> Result<BudgetReport, ScanError> {
    let mut parser = Parser::new_from_str(input);
    let mut enforcer = BudgetEnforcer::new(budget.clone());

    while let Some(item) = parser.next() {
        let (ev, _span) = item?;
        if let Err(breach) = enforcer.observe(&ev) {
            let mut report = enforcer.into_report();
            report.breached = Some(breach);
            return Ok(report);
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
pub fn parse_yaml(input: &str, budget: &Budget) -> Result<bool, ScanError> {
    let report = check_yaml_budget(input, budget)?;
    Ok(report.breached.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiny_yaml_ok() {
        let b = Budget::default();
        let y = "a: [1, 2, 3]\n";
        let r = check_yaml_budget(y, &b).unwrap();
        assert!(r.breached.is_none());
        assert_eq!(r.documents, 1);
        assert_eq!(r.nodes > 0, true);
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

        let mut b = Budget::default();
        b.max_aliases = 3; // set a tiny limit for the test

        let rep = check_yaml_budget(y, &b).unwrap();
        assert!(matches!(rep.breached, Some(BudgetBreach::Aliases { .. })));
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

        let mut b = Budget::default();
        b.max_depth = 150;

        let rep = check_yaml_budget(&y, &b).unwrap();
        assert!(matches!(rep.breached, Some(BudgetBreach::Depth { .. })));
    }

    #[test]
    fn anchors_limit_trips() {
        // Three distinct anchors defined on scalar nodes
        let y = "a: &A 1\nb: &B 2\nc: &C 3\n";
        let mut b = Budget::default();
        b.max_anchors = 2;
        let rep = check_yaml_budget(y, &b).unwrap();
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

        let mut b = Budget::default();
        b.max_merge_keys = 2;

        let rep = check_yaml_budget(&y, &b).unwrap();
        assert!(matches!(
            rep.breached,
            Some(BudgetBreach::MergeKeys { merge_keys }) if merge_keys == 3
        ));
        assert_eq!(rep.merge_keys, 3);
    }

    #[test]
    fn alias_anchor_ratio_trips_when_excessive() {
        let yaml = "root: &A [1]\na: *A\nb: *A\nc: *A\n";

        let mut budget = Budget::default();
        budget.alias_anchor_min_aliases = 1;
        budget.alias_anchor_ratio_multiplier = 2;

        let report = check_yaml_budget(yaml, &budget).unwrap();
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
    fn alias_anchor_ratio_respects_minimum_alias_threshold() {
        let yaml = "root: &A [1]\na: *A\nb: *A\nc: *A\n";

        let mut budget = Budget::default();
        budget.alias_anchor_min_aliases = 5;
        budget.alias_anchor_ratio_multiplier = 1;

        let report = check_yaml_budget(yaml, &budget).unwrap();
        assert!(report.breached.is_none());
        assert_eq!(report.aliases, 3);
        assert_eq!(report.anchors, 1);
    }
}
