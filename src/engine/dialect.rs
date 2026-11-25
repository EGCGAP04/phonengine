//! Chronological dialect trees and multi-branch derivation traces.

use std::collections::{BTreeMap, BTreeSet};

use crate::phonology::{Inventory, Word};

use super::applier::derive;
use super::error::{DialectError, EngineError};
use super::rule::Rule;
use super::trace::DerivationTrace;

/// A single chronological or dialectal stage in a [`DialectTree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialectNode {
    id: String,
    name: String,
    parent: Option<String>,
    rules: Vec<Rule>,
}

impl DialectNode {
    /// Creates a new [`DialectNode`] with an optional parent stage identifier and ordered rules.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        parent: Option<String>,
        rules: Vec<Rule>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            parent,
            rules,
        }
    }

    /// Returns the stage's unique identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the human-readable stage name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the parent stage identifier, or `None` for the root proto-language.
    #[must_use]
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    /// Returns the ordered rules applied at this stage.
    #[must_use]
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

/// Derivation trace recorded for one stage inside a [`DialectTree`] evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTrace {
    stage_id: String,
    stage_name: String,
    trace: DerivationTrace,
}

impl StageTrace {
    /// Returns the stage's unique identifier.
    #[must_use]
    pub fn stage_id(&self) -> &str {
        &self.stage_id
    }

    /// Returns the human-readable stage name.
    #[must_use]
    pub fn stage_name(&self) -> &str {
        &self.stage_name
    }

    /// Returns the [`DerivationTrace`] produced by this stage's rules.
    #[must_use]
    pub fn trace(&self) -> &DerivationTrace {
        &self.trace
    }

    /// Returns the output [`Word`] at the end of this stage.
    #[must_use]
    pub fn final_word(&self) -> &Word {
        self.trace.final_word()
    }
}

/// Chronological derivation from root to a single target stage in a [`DialectTree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialectTrace {
    initial: Word,
    target_stage: String,
    stages: Vec<StageTrace>,
    final_word: Word,
}

impl DialectTrace {
    /// Returns the initial input word fed into the root stage.
    #[must_use]
    pub fn initial(&self) -> &Word {
        &self.initial
    }

    /// Returns the requested target stage identifier.
    #[must_use]
    pub fn target_stage(&self) -> &str {
        &self.target_stage
    }

    /// Returns the ordered stage traces from root down to [`target_stage`](Self::target_stage).
    #[must_use]
    pub fn stages(&self) -> &[StageTrace] {
        &self.stages
    }

    /// Returns the final word produced at [`target_stage`](Self::target_stage).
    #[must_use]
    pub fn final_word(&self) -> &Word {
        &self.final_word
    }
}

/// Full multi-branch derivation across every stage of a [`DialectTree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeDerivation {
    root_id: String,
    initial: Word,
    stages: BTreeMap<String, StageTrace>,
    ancestry_paths: BTreeMap<String, Vec<String>>,
    leaves: Vec<String>,
}

impl TreeDerivation {
    /// Returns the root stage identifier.
    #[must_use]
    pub fn root_id(&self) -> &str {
        &self.root_id
    }

    /// Returns the initial input word fed into the root stage.
    #[must_use]
    pub fn initial(&self) -> &Word {
        &self.initial
    }

    /// Returns all stage traces keyed deterministically by stage identifier.
    #[must_use]
    pub fn stages(&self) -> &BTreeMap<String, StageTrace> {
        &self.stages
    }

    /// Returns the final word at `stage_id`, or `None` if `stage_id` does not exist.
    #[must_use]
    pub fn word_at(&self, stage_id: &str) -> Option<&Word> {
        self.stages.get(stage_id).map(StageTrace::final_word)
    }

    /// Returns the ordered slice of [`StageTrace`] references from root to `stage_id`.
    #[must_use]
    pub fn path_to(&self, stage_id: &str) -> Option<Vec<&StageTrace>> {
        let path_ids = self.ancestry_paths.get(stage_id)?;
        let mut path = Vec::with_capacity(path_ids.len());
        for id in path_ids {
            path.push(self.stages.get(id)?);
        }
        Some(path)
    }

    /// Returns the final word of every leaf dialect keyed deterministically by stage identifier.
    #[must_use]
    pub fn leaf_outcomes(&self) -> BTreeMap<&str, &Word> {
        let mut outcomes = BTreeMap::new();
        for leaf_id in &self.leaves {
            if let Some(stage_trace) = self.stages.get(leaf_id) {
                outcomes.insert(leaf_id.as_str(), stage_trace.final_word());
            }
        }
        outcomes
    }
}

/// Validated rooted tree of chronological and dialectal phonological stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialectTree {
    root_id: String,
    nodes: BTreeMap<String, DialectNode>,
    children: BTreeMap<String, Vec<String>>,
}

impl DialectTree {
    /// Validates and constructs a rooted [`DialectTree`] from `nodes`.
    ///
    /// # Errors
    ///
    /// - [`DialectError::EmptyTree`] if `nodes` is empty.
    /// - [`DialectError::EmptyStageId`] if any node has an empty or whitespace-only `id`.
    /// - [`DialectError::DuplicateStageId`] if two nodes share an `id`.
    /// - [`DialectError::UnknownParent`] if any node references a non-existent parent.
    /// - [`DialectError::MissingRoot`] if no node has `parent: None`.
    /// - [`DialectError::MultipleRoots`] if more than one node has `parent: None`.
    /// - [`DialectError::CycleDetected`] if parent links form a cycle unreachable from the root.
    pub fn new(nodes: Vec<DialectNode>) -> Result<Self, DialectError> {
        if nodes.is_empty() {
            return Err(DialectError::EmptyTree);
        }

        let mut by_id = BTreeMap::new();
        for node in nodes {
            if node.id.trim().is_empty() {
                return Err(DialectError::EmptyStageId);
            }
            if by_id.contains_key(&node.id) {
                return Err(DialectError::DuplicateStageId(node.id));
            }
            by_id.insert(node.id.clone(), node);
        }

        let mut roots = Vec::new();
        let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (id, node) in &by_id {
            children.entry(id.clone()).or_default();
            match &node.parent {
                None => roots.push(id.clone()),
                Some(parent_id) => {
                    if !by_id.contains_key(parent_id) {
                        return Err(DialectError::UnknownParent {
                            stage_id: id.clone(),
                            parent_id: parent_id.clone(),
                        });
                    }
                    children
                        .entry(parent_id.clone())
                        .or_default()
                        .push(id.clone());
                }
            }
        }

        if roots.is_empty() {
            return Err(DialectError::MissingRoot);
        }
        if roots.len() > 1 {
            return Err(DialectError::MultipleRoots(roots));
        }

        let root_id = roots.swap_remove(0);
        let mut visited = BTreeSet::new();
        let mut stack = vec![root_id.clone()];
        while let Some(current_id) = stack.pop() {
            if !visited.insert(current_id.clone()) {
                continue;
            }
            if let Some(child_ids) = children.get(&current_id) {
                for child_id in child_ids.iter().rev() {
                    stack.push(child_id.clone());
                }
            }
        }

        if visited.len() < by_id.len() {
            let unvisited: BTreeSet<&str> = by_id
                .keys()
                .map(String::as_str)
                .filter(|id| !visited.contains(*id))
                .collect();
            return Err(DialectError::CycleDetected(extract_cycle(
                &by_id, &unvisited,
            )));
        }

        Ok(Self {
            root_id,
            nodes: by_id,
            children,
        })
    }

    /// Returns the root stage identifier.
    #[must_use]
    pub fn root_id(&self) -> &str {
        &self.root_id
    }

    /// Looks up a stage node by identifier.
    #[must_use]
    pub fn node(&self, stage_id: &str) -> Option<&DialectNode> {
        self.nodes.get(stage_id)
    }

    /// Returns all stage nodes keyed deterministically by identifier.
    #[must_use]
    pub fn nodes(&self) -> &BTreeMap<String, DialectNode> {
        &self.nodes
    }

    /// Returns the ordered child stage identifiers of `stage_id` (empty slice if leaf or unknown).
    #[must_use]
    pub fn children_of(&self, stage_id: &str) -> &[String] {
        self.children.get(stage_id).map_or(&[], Vec::as_slice)
    }

    /// Returns all leaf stage identifiers in deterministic lexicographic order.
    #[must_use]
    pub fn leaves(&self) -> Vec<&str> {
        self.children
            .iter()
            .filter_map(|(id, kids)| kids.is_empty().then_some(id.as_str()))
            .collect()
    }

    /// Returns the chronological stage ID path from root to `stage_id`, or `None` if unknown.
    #[must_use]
    pub fn ancestry(&self, stage_id: &str) -> Option<Vec<&str>> {
        let mut cursor = self
            .nodes
            .get_key_value(stage_id)
            .map(|(key, _)| key.as_str());
        cursor?;
        let mut reversed = Vec::new();
        while let Some(id) = cursor {
            reversed.push(id);
            cursor = self.nodes.get(id).and_then(DialectNode::parent);
        }
        reversed.reverse();
        Some(reversed)
    }

    /// Derives `initial` along the single chronological lineage from root to `target_stage`.
    ///
    /// # Errors
    ///
    /// - [`EngineError::UnknownStage`] if `target_stage` does not exist in this tree.
    /// - Propagates any [`EngineError`] emitted while applying a stage's rules.
    pub fn derive_stage(
        &self,
        initial: &Word,
        target_stage: &str,
        inventory: &Inventory,
    ) -> Result<DialectTrace, EngineError> {
        let Some(path) = self.ancestry(target_stage) else {
            return Err(EngineError::UnknownStage(target_stage.to_owned()));
        };

        let mut current = initial.clone();
        let mut stages = Vec::with_capacity(path.len());
        for stage_id in path {
            let Some(node) = self.nodes.get(stage_id) else {
                return Err(EngineError::UnknownStage(stage_id.to_owned()));
            };
            let trace = derive(&current, &node.rules, inventory)?;
            current = trace.final_word().clone();
            stages.push(StageTrace {
                stage_id: node.id.clone(),
                stage_name: node.name.clone(),
                trace,
            });
        }

        Ok(DialectTrace {
            initial: initial.clone(),
            target_stage: target_stage.to_owned(),
            stages,
            final_word: current,
        })
    }

    /// Derives `initial` across the entire dialect tree, reusing each shared ancestor's output.
    ///
    /// # Errors
    ///
    /// Propagates any [`EngineError`] emitted while applying a stage's rules.
    pub fn derive_tree(
        &self,
        initial: &Word,
        inventory: &Inventory,
    ) -> Result<TreeDerivation, EngineError> {
        let mut stages = BTreeMap::new();
        let mut ancestry_paths = BTreeMap::new();

        // Iterative deterministic DFS starting from root, passing parent output & lineage path.
        let mut stack = vec![(self.root_id.clone(), initial.clone(), Vec::new())];
        while let Some((stage_id, stage_input, mut path)) = stack.pop() {
            let Some(node) = self.nodes.get(&stage_id) else {
                return Err(EngineError::UnknownStage(stage_id));
            };
            let trace = derive(&stage_input, &node.rules, inventory)?;
            let stage_output = trace.final_word().clone();
            path.push(stage_id.clone());

            stages.insert(
                stage_id.clone(),
                StageTrace {
                    stage_id: node.id.clone(),
                    stage_name: node.name.clone(),
                    trace,
                },
            );
            ancestry_paths.insert(stage_id.clone(), path.clone());

            if let Some(child_ids) = self.children.get(&stage_id) {
                for child_id in child_ids.iter().rev() {
                    stack.push((child_id.clone(), stage_output.clone(), path.clone()));
                }
            }
        }

        let leaves = self.leaves().into_iter().map(str::to_owned).collect();
        Ok(TreeDerivation {
            root_id: self.root_id.clone(),
            initial: initial.clone(),
            stages,
            ancestry_paths,
            leaves,
        })
    }
}

fn extract_cycle(by_id: &BTreeMap<String, DialectNode>, unvisited: &BTreeSet<&str>) -> Vec<String> {
    let Some(&start) = unvisited.first() else {
        return Vec::new();
    };

    let mut seen_index: BTreeMap<&str, usize> = BTreeMap::new();
    let mut chain: Vec<String> = Vec::new();
    let mut cursor = Some(start);

    while let Some(id) = cursor {
        if let Some(&idx) = seen_index.get(id) {
            let mut cycle = chain[idx..].to_vec();
            cycle.push(id.to_owned());
            return cycle;
        }
        seen_index.insert(id, chain.len());
        chain.push(id.to_owned());
        cursor = by_id.get(id).and_then(DialectNode::parent);
    }

    chain
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{ApplicationMode, ContextElement, Pattern, ReplacementItem};
    use crate::phonology::{Backness, FeatureBundle, Height, InventoryError, Manner, Place};

    fn greek_inventory() -> Result<Inventory, InventoryError> {
        let mut inv = Inventory::new();
        inv.insert(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
        )?;
        inv.insert(
            "aː",
            FeatureBundle::vowel(Height::Low, Backness::Central, false).with_long(true),
        )?;
        inv.insert(
            "eː",
            FeatureBundle::vowel(Height::MidHigh, Backness::Front, false).with_long(true),
        )?;
        inv.insert(
            "s",
            FeatureBundle::consonant(Manner::Fricative, Place::Alveolar, false),
        )?;
        inv.insert(
            "h",
            FeatureBundle::consonant(Manner::Fricative, Place::Glottal, false),
        )?;
        inv.insert(
            "r",
            FeatureBundle::consonant(Manner::Rhotic, Place::Alveolar, true),
        )?;
        inv.insert(
            "t",
            FeatureBundle::consonant(Manner::Stop, Place::Alveolar, false),
        )?;
        Ok(inv)
    }

    fn greek_tree() -> Result<DialectTree, Box<dyn std::error::Error>> {
        let pie = DialectNode::new("pie", "Proto-Indo-European", None, vec![]);
        let s_to_h = Rule::new(
            "s_debucc",
            "Initial s > h",
            vec![Pattern::Literal("s".to_owned())],
            vec![ReplacementItem::Literal("h".to_owned())],
            vec![ContextElement::WordBoundary],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        let proto_greek = DialectNode::new(
            "proto_greek",
            "Proto-Greek",
            Some("pie".to_owned()),
            vec![s_to_h],
        );

        let attic_raise = Rule::new(
            "a_raising",
            "Long a > e",
            vec![Pattern::Literal("aː".to_owned())],
            vec![ReplacementItem::Literal("eː".to_owned())],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        let attic_revert = Rule::new(
            "alpha_purum",
            "eː > aː after r",
            vec![Pattern::Literal("eː".to_owned())],
            vec![ReplacementItem::Literal("aː".to_owned())],
            vec![ContextElement::Segment(Pattern::Literal("r".to_owned()))],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        let attic = DialectNode::new(
            "attic",
            "Attic Greek",
            Some("proto_greek".to_owned()),
            vec![attic_raise.clone(), attic_revert],
        );

        let ionic = DialectNode::new(
            "ionic",
            "Ionic Greek",
            Some("proto_greek".to_owned()),
            vec![attic_raise],
        );

        Ok(DialectTree::new(vec![pie, proto_greek, attic, ionic])?)
    }

    #[test]
    fn dialect_tree_derives_single_stage_and_full_branching_tree()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = greek_inventory()?;
        let tree = greek_tree()?;

        assert_eq!(tree.root_id(), "pie");
        assert_eq!(tree.children_of("proto_greek"), &["attic", "ionic"]);
        assert_eq!(tree.leaves(), vec!["attic", "ionic"]);
        assert_eq!(
            tree.ancestry("attic"),
            Some(vec!["pie", "proto_greek", "attic"])
        );

        let word = Word::from_spaced("s r aː t", &inv)?;

        let attic_trace = tree.derive_stage(&word, "attic", &inv)?;
        assert_eq!(attic_trace.target_stage(), "attic");
        assert_eq!(attic_trace.stages().len(), 3);
        assert_eq!(attic_trace.final_word().to_spaced(), "h r aː t");
        // Both raising and reversion happened inside `attic` stage.
        assert_eq!(
            attic_trace.stages()[2].trace().fired_rule_ids(),
            vec!["a_raising", "alpha_purum"]
        );

        let full = tree.derive_tree(&word, &inv)?;
        assert_eq!(full.root_id(), "pie");
        assert_eq!(full.initial(), &word);
        assert_eq!(
            full.word_at("proto_greek").map(Word::to_spaced).as_deref(),
            Some("h r aː t")
        );
        assert_eq!(
            full.word_at("attic").map(Word::to_spaced).as_deref(),
            Some("h r aː t")
        );
        assert_eq!(
            full.word_at("ionic").map(Word::to_spaced).as_deref(),
            Some("h r eː t")
        );

        let leaves: BTreeMap<&str, String> = full
            .leaf_outcomes()
            .into_iter()
            .map(|(k, w)| (k, w.to_spaced()))
            .collect();
        assert_eq!(leaves.get("attic").map(String::as_str), Some("h r aː t"));
        assert_eq!(leaves.get("ionic").map(String::as_str), Some("h r eː t"));

        let ionic_path = full
            .path_to("ionic")
            .ok_or_else(|| EngineError::UnknownStage("ionic".to_owned()))?;
        assert_eq!(
            ionic_path
                .iter()
                .map(|s| s.stage_id())
                .collect::<Vec<&str>>(),
            vec!["pie", "proto_greek", "ionic"]
        );

        assert_eq!(
            tree.derive_stage(&word, "dorian", &inv),
            Err(EngineError::UnknownStage("dorian".to_owned()))
        );
        Ok(())
    }

    #[test]
    fn dialect_tree_validates_structural_invariants() {
        assert_eq!(DialectTree::new(vec![]), Err(DialectError::EmptyTree));

        assert_eq!(
            DialectTree::new(vec![DialectNode::new("  ", "Blank", None, vec![])]),
            Err(DialectError::EmptyStageId)
        );

        assert_eq!(
            DialectTree::new(vec![
                DialectNode::new("pie", "PIE", None, vec![]),
                DialectNode::new("pie", "Duplicate", None, vec![]),
            ]),
            Err(DialectError::DuplicateStageId("pie".to_owned()))
        );

        assert_eq!(
            DialectTree::new(vec![
                DialectNode::new("pie", "PIE", None, vec![]),
                DialectNode::new("attic", "Attic", Some("missing".to_owned()), vec![]),
            ]),
            Err(DialectError::UnknownParent {
                stage_id: "attic".to_owned(),
                parent_id: "missing".to_owned(),
            })
        );

        assert_eq!(
            DialectTree::new(vec![
                DialectNode::new("a", "A", Some("b".to_owned()), vec![]),
                DialectNode::new("b", "B", Some("a".to_owned()), vec![]),
            ]),
            Err(DialectError::MissingRoot)
        );

        assert_eq!(
            DialectTree::new(vec![
                DialectNode::new("root1", "Root 1", None, vec![]),
                DialectNode::new("root2", "Root 2", None, vec![]),
            ]),
            Err(DialectError::MultipleRoots(vec![
                "root1".to_owned(),
                "root2".to_owned(),
            ]))
        );

        assert_eq!(
            DialectTree::new(vec![
                DialectNode::new("pie", "PIE", None, vec![]),
                DialectNode::new("a", "A", Some("b".to_owned()), vec![]),
                DialectNode::new("b", "B", Some("a".to_owned()), vec![]),
            ]),
            Err(DialectError::CycleDetected(vec![
                "a".to_owned(),
                "b".to_owned(),
                "a".to_owned(),
            ]))
        );
    }
}
