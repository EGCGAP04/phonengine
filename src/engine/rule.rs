//! Phonological rule AST, natural classes, contexts, and validation invariants.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::phonology::{
    FeatureDomain, FeatureField, FeatureMutation, FeaturePattern, FeatureVar, FeatureVarValue,
    LiteralStress, Segment, StressMatch, StressUpdate, VarBindings, VarPolarity,
};

use super::error::RuleError;

/// Named set of segment symbols forming a phonological natural class.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NaturalClass {
    /// Class identifier (e.g., `"V"` for vowels).
    pub name: String,
    /// Deterministic ordered set of member segment symbols.
    pub members: BTreeSet<String>,
}

impl NaturalClass {
    /// Creates a [`NaturalClass`] from a name and an iterable of member symbols.
    #[must_use]
    pub fn new<I, S>(name: impl Into<String>, members: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            name: name.into(),
            members: members.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns `true` if `segment` belongs to this natural class.
    #[must_use]
    pub fn matches(&self, segment: &Segment) -> bool {
        self.members.contains(&segment.symbol)
    }
}

/// Segment base matcher wrapped inside a [`ConstrainedPattern`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternBase {
    /// Exact match on `segment.symbol`.
    Literal(String),
    /// Feature-based match on `segment.bundle`.
    Features(FeaturePattern),
    /// Membership check in a [`NaturalClass`].
    Class(NaturalClass),
}

impl PatternBase {
    /// Returns `true` if `segment` matches this base pattern.
    #[must_use]
    pub fn matches(&self, segment: &Segment) -> bool {
        match self {
            Self::Literal(symbol) => segment.symbol == *symbol,
            Self::Features(pattern) => pattern.matches(&segment.bundle),
            Self::Class(class) => class.matches(segment),
        }
    }
}

/// Segment pattern combining a [`PatternBase`] with prosodic [`StressMatch`] and alpha variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstrainedPattern {
    /// Underlying literal, feature bundle pattern, or natural class matcher.
    pub base: PatternBase,
    /// Prosodic stress predicate evaluated against `segment.stress`.
    pub stress: StressMatch,
    /// Alpha variable constraints bound or verified against `segment`.
    pub vars: Vec<FeatureVar>,
}

impl ConstrainedPattern {
    /// Creates a [`ConstrainedPattern`] with unconstrained stress and no alpha variables.
    #[must_use]
    pub const fn new(base: PatternBase) -> Self {
        Self {
            base,
            stress: StressMatch::Any,
            vars: Vec::new(),
        }
    }

    /// Constrains the prosodic stress level matched by this pattern.
    #[must_use]
    pub const fn with_stress(mut self, stress: StressMatch) -> Self {
        self.stress = stress;
        self
    }

    /// Appends an alpha variable constraint to this pattern.
    #[must_use]
    pub fn with_var(mut self, var: FeatureVar) -> Self {
        self.vars.push(var);
        self
    }

    /// Appends multiple alpha variable constraints to this pattern.
    #[must_use]
    pub fn with_vars<I>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = FeatureVar>,
    {
        self.vars.extend(vars);
        self
    }
}

/// Segment pattern usable inside rule targets and contexts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// Exact match on `segment.symbol`.
    Literal(String),
    /// Feature-based match on `segment.bundle`.
    Features(FeaturePattern),
    /// Membership check in a [`NaturalClass`].
    Class(NaturalClass),
    /// Base pattern constrained by prosodic stress and/or alpha variables.
    Constrained(ConstrainedPattern),
}

impl Pattern {
    /// Returns `true` if `segment` matches this pattern under fresh, temporary variable bindings.
    #[must_use]
    pub fn matches(&self, segment: &Segment) -> bool {
        let mut scratch = VarBindings::new();
        self.matches_with_bindings(segment, &mut scratch)
    }

    /// Matches `segment` while verifying and extending `bindings` atomically.
    ///
    /// If any constraint fails, `bindings` is left unmodified.
    pub fn matches_with_bindings(&self, segment: &Segment, bindings: &mut VarBindings) -> bool {
        match self {
            Self::Literal(symbol) => segment.symbol == *symbol,
            Self::Features(pattern) => pattern.matches(&segment.bundle),
            Self::Class(class) => class.matches(segment),
            Self::Constrained(constrained) => {
                if !constrained.base.matches(segment) || !constrained.stress.matches(segment.stress)
                {
                    return false;
                }
                if constrained.vars.is_empty() {
                    return true;
                }
                let mut trial = bindings.clone();
                if constrained
                    .vars
                    .iter()
                    .all(|var| trial.match_or_bind(var, segment))
                {
                    *bindings = trial;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Matches `segment` assuming all alpha variables in this pattern are already present in
    /// `bindings` (used by the fast DP path when no unbound context variables remain).
    #[must_use]
    pub fn matches_with_fixed_bindings(&self, segment: &Segment, bindings: &VarBindings) -> bool {
        match self {
            Self::Literal(symbol) => segment.symbol == *symbol,
            Self::Features(pattern) => pattern.matches(&segment.bundle),
            Self::Class(class) => class.matches(segment),
            Self::Constrained(constrained) => {
                constrained.base.matches(segment)
                    && constrained.stress.matches(segment.stress)
                    && constrained.vars.iter().all(|var| {
                        let Some(expected) = bindings.resolve(var) else {
                            return false;
                        };
                        FeatureVarValue::extract(segment, var.field) == Some(expected)
                    })
            }
        }
    }

    /// Returns the alpha variable constraints attached to this pattern.
    #[must_use]
    pub fn vars(&self) -> &[FeatureVar] {
        match self {
            Self::Constrained(constrained) => &constrained.vars,
            Self::Literal(_) | Self::Features(_) | Self::Class(_) => &[],
        }
    }
}

/// Segment-level mutation combining intrinsic feature changes, prosodic stress updates,
/// and alpha variable assignments.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SegmentMutation {
    /// Intrinsic bundle mutations applied to the source segment.
    pub features: FeatureMutation,
    /// Prosodic stress update applied after copying the source segment's stress.
    pub stress: StressUpdate,
    /// Alpha variable values applied on top of `features` and `stress`.
    pub vars: Vec<FeatureVar>,
}

impl SegmentMutation {
    /// Creates a no-op mutation preserving all features and prosodic stress of the source segment.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            features: FeatureMutation::new(),
            stress: StressUpdate::Keep,
            vars: Vec::new(),
        }
    }

    /// Wraps an intrinsic [`FeatureMutation`] with [`StressUpdate::Keep`] and no variables.
    #[must_use]
    pub const fn from_features(features: FeatureMutation) -> Self {
        Self {
            features,
            stress: StressUpdate::Keep,
            vars: Vec::new(),
        }
    }

    /// Sets the prosodic [`StressUpdate`] for this mutation.
    #[must_use]
    pub const fn with_stress(mut self, stress: StressUpdate) -> Self {
        self.stress = stress;
        self
    }

    /// Appends an alpha variable assignment to this mutation.
    #[must_use]
    pub fn with_var(mut self, var: FeatureVar) -> Self {
        self.vars.push(var);
        self
    }

    /// Appends multiple alpha variable assignments to this mutation.
    #[must_use]
    pub fn with_vars<I>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = FeatureVar>,
    {
        self.vars.extend(vars);
        self
    }
}

impl From<FeatureMutation> for SegmentMutation {
    fn from(features: FeatureMutation) -> Self {
        Self::from_features(features)
    }
}

/// Single output element produced when a rule match is rewritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementItem {
    /// Emits the segment registered under this symbol in the inventory, inheriting stress by
    /// position unless a `Mutate` already copies that source index.
    Literal(String),
    /// Emits the segment registered under `symbol` with explicit or inherited [`LiteralStress`].
    LiteralWithStress {
        /// Registered inventory symbol to emit.
        symbol: String,
        /// Stress directive (`Inherit`, explicit `Set`, or `Var`).
        stress: LiteralStress,
    },
    /// Mutates the matched input segment at `source_index` via [`SegmentMutation`].
    Mutate {
        /// Zero-based index into the matched `target` slice.
        source_index: usize,
        /// Segment mutation applied to the source segment.
        mutation: SegmentMutation,
    },
}

/// Element of a rule's left or right conditioning environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextElement {
    /// Word boundary (`#`); only valid as the outermost element of a context.
    WordBoundary,
    /// Single segment matching `Pattern`.
    Segment(Pattern),
    /// Optional single segment (`Pattern?` / `(Pattern)`): matches zero or one segment.
    Optional(Pattern),
    /// Non-local Kleene closure (`Pattern*`): zero or more consecutive segments matching `Pattern`.
    ZeroOrMore(Pattern),
}

impl ContextElement {
    const fn pattern(&self) -> Option<&Pattern> {
        match self {
            Self::WordBoundary => None,
            Self::Segment(pat) | Self::Optional(pat) | Self::ZeroOrMore(pat) => Some(pat),
        }
    }
}

/// Conditioning environment branch paired with left and right contexts around `_`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Environment {
    /// Left context elements ordered left-to-right (outermost edge first).
    pub left: Vec<ContextElement>,
    /// Right context elements ordered left-to-right (outermost edge last).
    pub right: Vec<ContextElement>,
}

impl Environment {
    /// Creates an [`Environment`] from explicit `left` and `right` context element lists.
    #[must_use]
    pub const fn new(left: Vec<ContextElement>, right: Vec<ContextElement>) -> Self {
        Self { left, right }
    }

    /// Returns `true` if both left and right contexts are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }
}

/// Evaluation order strategy for rule application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ApplicationMode {
    /// Evaluates all non-overlapping matches against an immutable snapshot of the input word.
    #[default]
    Simultaneous,
    /// Single left-to-right pass that rewrites the word progressively, so earlier rewrites can
    /// feed or bleed later matches.
    LeftToRight,
    /// Single right-to-left pass, the mirror image of [`ApplicationMode::LeftToRight`].
    RightToLeft,
}

impl fmt::Display for ApplicationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Simultaneous => "simultaneous",
            Self::LeftToRight => "left-to-right",
            Self::RightToLeft => "right-to-left",
        };
        f.write_str(label)
    }
}

/// A validated phonological sound-change rule supporting general `N -> M` rewrites,
/// prosodic stress constraints, alpha feature variables, and multiple conditioning environments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    id: String,
    name: String,
    target: Vec<Pattern>,
    replacement: Vec<ReplacementItem>,
    environments: Vec<Environment>,
    mode: ApplicationMode,
}

impl Rule {
    /// Constructs and validates a phonological [`Rule`] with a single optional context pair.
    ///
    /// When both `left_context` and `right_context` are empty, the rule is stored as unconditional
    /// (`environments: []`).
    ///
    /// # Errors
    ///
    /// Returns [`RuleError`] on invalid rule identifiers, unanchored insertions, misplaced `#`,
    /// out-of-range `source_index`, or invalid/conflicting/unbound alpha variables.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        target: Vec<Pattern>,
        replacement: Vec<ReplacementItem>,
        left_context: Vec<ContextElement>,
        right_context: Vec<ContextElement>,
        mode: ApplicationMode,
    ) -> Result<Self, RuleError> {
        let environments = if left_context.is_empty() && right_context.is_empty() {
            Vec::new()
        } else {
            vec![Environment::new(left_context, right_context)]
        };
        Self::with_environments(id, name, target, replacement, environments, mode)
    }

    /// Constructs and validates a phonological [`Rule`] with zero or more disjunctive
    /// [`Environment`] branches evaluated in declared order.
    ///
    /// An empty `environments` vector represents an unconditional rule.
    ///
    /// # Errors
    ///
    /// - [`RuleError::EmptyId`] if `id` is empty or whitespace-only.
    /// - [`RuleError::UnconstrainedInsertion`] if `target` is empty and `environments` is empty or
    ///   any environment branch lacks a [`ContextElement::WordBoundary`] or [`ContextElement::Segment`].
    /// - [`RuleError::MisplacedWordBoundary`] if any environment places `#` away from its outer edges.
    /// - [`RuleError::InvalidSourceIndex`] if any [`ReplacementItem::Mutate`] has `source_index >= target.len()`.
    /// - [`RuleError::ConflictingFeatureConstraint`] if a segment spec constrains/mutates the same field twice.
    /// - [`RuleError::InvalidVariableName`], [`RuleError::InvalidVariablePolarity`], or
    ///   [`RuleError::IncompatibleVariableDomain`] on malformed alpha variable declarations.
    /// - [`RuleError::UnboundVariable`] if `replacement` reads a variable not guaranteed to be bound.
    pub fn with_environments(
        id: impl Into<String>,
        name: impl Into<String>,
        target: Vec<Pattern>,
        replacement: Vec<ReplacementItem>,
        environments: Vec<Environment>,
        mode: ApplicationMode,
    ) -> Result<Self, RuleError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(RuleError::EmptyId);
        }
        if target.is_empty()
            && (environments.is_empty() || !environments.iter().all(env_is_anchored))
        {
            return Err(RuleError::UnconstrainedInsertion { rule_id: id });
        }
        for env in &environments {
            if !boundary_only_at(&env.left, 0)
                || !boundary_only_at(&env.right, env.right.len().saturating_sub(1))
            {
                return Err(RuleError::MisplacedWordBoundary { rule_id: id });
            }
        }
        for item in &replacement {
            if let ReplacementItem::Mutate { source_index, .. } = item
                && *source_index >= target.len()
            {
                return Err(RuleError::InvalidSourceIndex {
                    rule_id: id,
                    source_index: *source_index,
                    target_len: target.len(),
                });
            }
        }

        validate_conflicts(&id, &target, &environments, &replacement)?;
        validate_variables(&id, &target, &environments, &replacement)?;
        validate_bound_variables(&id, &target, &environments, &replacement)?;

        Ok(Self {
            id,
            name: name.into(),
            target,
            replacement,
            environments,
            mode,
        })
    }

    /// Returns the rule's unique identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the human-readable rule name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the target pattern sequence.
    #[must_use]
    pub fn target(&self) -> &[Pattern] {
        &self.target
    }

    /// Returns the replacement item sequence.
    #[must_use]
    pub fn replacement(&self) -> &[ReplacementItem] {
        &self.replacement
    }

    /// Returns the disjunctive list of conditioning [`Environment`]s (`[]` if unconditional).
    #[must_use]
    pub fn environments(&self) -> &[Environment] {
        &self.environments
    }

    /// Returns the first environment's left conditioning context (or `&[]` if unconditional).
    #[must_use]
    pub fn left_context(&self) -> &[ContextElement] {
        self.environments
            .first()
            .map_or(&[], |env| env.left.as_slice())
    }

    /// Returns the first environment's right conditioning context (or `&[]` if unconditional).
    #[must_use]
    pub fn right_context(&self) -> &[ContextElement] {
        self.environments
            .first()
            .map_or(&[], |env| env.right.as_slice())
    }

    /// Returns the rule's [`ApplicationMode`].
    #[must_use]
    pub const fn mode(&self) -> ApplicationMode {
        self.mode
    }
}

fn env_is_anchored(env: &Environment) -> bool {
    env.left.iter().chain(&env.right).any(|elem| {
        matches!(
            elem,
            ContextElement::WordBoundary | ContextElement::Segment(_)
        )
    })
}

/// Returns `true` if every [`ContextElement::WordBoundary`] in `context` sits at `outer_index`.
fn boundary_only_at(context: &[ContextElement], outer_index: usize) -> bool {
    context
        .iter()
        .enumerate()
        .all(|(idx, elem)| idx == outer_index || !matches!(elem, ContextElement::WordBoundary))
}

fn all_patterns<'a>(
    target: &'a [Pattern],
    environments: &'a [Environment],
) -> impl Iterator<Item = &'a Pattern> {
    target.iter().chain(environments.iter().flat_map(|env| {
        env.left
            .iter()
            .chain(&env.right)
            .filter_map(ContextElement::pattern)
    }))
}

fn validate_conflicts(
    rule_id: &str,
    target: &[Pattern],
    environments: &[Environment],
    replacement: &[ReplacementItem],
) -> Result<(), RuleError> {
    for pattern in all_patterns(target, environments) {
        if let Pattern::Constrained(cp) = pattern {
            let mut seen = BTreeSet::new();
            if let PatternBase::Features(fp) = &cp.base {
                seen.extend(fp.constrained_fields());
            }
            if cp.stress != StressMatch::Any && !seen.insert(FeatureField::Stress) {
                return Err(RuleError::ConflictingFeatureConstraint {
                    rule_id: rule_id.to_owned(),
                    field: FeatureField::Stress,
                });
            }
            for var in &cp.vars {
                if !seen.insert(var.field) {
                    return Err(RuleError::ConflictingFeatureConstraint {
                        rule_id: rule_id.to_owned(),
                        field: var.field,
                    });
                }
            }
        }
    }

    for item in replacement {
        if let ReplacementItem::Mutate { mutation, .. } = item {
            let mut seen: BTreeSet<FeatureField> =
                mutation.features.updated_fields().into_iter().collect();
            if mutation.stress != StressUpdate::Keep && !seen.insert(FeatureField::Stress) {
                return Err(RuleError::ConflictingFeatureConstraint {
                    rule_id: rule_id.to_owned(),
                    field: FeatureField::Stress,
                });
            }
            for var in &mutation.vars {
                if !seen.insert(var.field) {
                    return Err(RuleError::ConflictingFeatureConstraint {
                        rule_id: rule_id.to_owned(),
                        field: var.field,
                    });
                }
            }
        }
    }

    Ok(())
}

fn register_variable<'a>(
    domains: &mut BTreeMap<&'a str, FeatureDomain>,
    rule_id: &str,
    name: &'a str,
    field: FeatureField,
    polarity: VarPolarity,
) -> Result<(), RuleError> {
    if name.trim().is_empty() {
        return Err(RuleError::InvalidVariableName {
            rule_id: rule_id.to_owned(),
            var: name.to_owned(),
        });
    }
    if polarity == VarPolarity::Opposite && !field.is_bool() {
        return Err(RuleError::InvalidVariablePolarity {
            rule_id: rule_id.to_owned(),
            var: name.to_owned(),
            field,
        });
    }
    let domain = field.domain();
    match domains.entry(name) {
        Entry::Vacant(slot) => {
            slot.insert(domain);
        }
        Entry::Occupied(slot) => {
            let first = *slot.get();
            if first != domain {
                return Err(RuleError::IncompatibleVariableDomain {
                    rule_id: rule_id.to_owned(),
                    var: name.to_owned(),
                    first,
                    second: domain,
                });
            }
        }
    }
    Ok(())
}

fn validate_variables(
    rule_id: &str,
    target: &[Pattern],
    environments: &[Environment],
    replacement: &[ReplacementItem],
) -> Result<(), RuleError> {
    let mut domains = BTreeMap::new();

    for pattern in all_patterns(target, environments) {
        for var in pattern.vars() {
            register_variable(&mut domains, rule_id, &var.name, var.field, var.polarity)?;
        }
    }

    for item in replacement {
        match item {
            ReplacementItem::Literal(_) => {}
            ReplacementItem::LiteralWithStress { stress, .. } => {
                if let LiteralStress::Var(name) = stress {
                    register_variable(
                        &mut domains,
                        rule_id,
                        name,
                        FeatureField::Stress,
                        VarPolarity::Same,
                    )?;
                }
            }
            ReplacementItem::Mutate { mutation, .. } => {
                for var in &mutation.vars {
                    register_variable(&mut domains, rule_id, &var.name, var.field, var.polarity)?;
                }
            }
        }
    }

    Ok(())
}

fn validate_bound_variables(
    rule_id: &str,
    target: &[Pattern],
    environments: &[Environment],
    replacement: &[ReplacementItem],
) -> Result<(), RuleError> {
    let target_bound: BTreeSet<&str> = target
        .iter()
        .flat_map(|pat| pat.vars().iter().map(|var| var.name.as_str()))
        .collect();

    let env_guaranteed: Vec<BTreeSet<&str>> = environments
        .iter()
        .map(|env| {
            env.left
                .iter()
                .chain(&env.right)
                .filter_map(|elem| match elem {
                    ContextElement::Segment(pat) => Some(pat.vars()),
                    ContextElement::WordBoundary
                    | ContextElement::Optional(_)
                    | ContextElement::ZeroOrMore(_) => None,
                })
                .flat_map(|vars| vars.iter().map(|var| var.name.as_str()))
                .collect()
        })
        .collect();

    for item in replacement {
        match item {
            ReplacementItem::Literal(_) => {}
            ReplacementItem::LiteralWithStress { stress, .. } => {
                if let LiteralStress::Var(name) = stress {
                    ensure_variable_bound(rule_id, name, &target_bound, &env_guaranteed)?;
                }
            }
            ReplacementItem::Mutate { mutation, .. } => {
                for var in &mutation.vars {
                    ensure_variable_bound(rule_id, &var.name, &target_bound, &env_guaranteed)?;
                }
            }
        }
    }

    Ok(())
}

fn ensure_variable_bound(
    rule_id: &str,
    name: &str,
    target_bound: &BTreeSet<&str>,
    env_guaranteed: &[BTreeSet<&str>],
) -> Result<(), RuleError> {
    if target_bound.contains(name) {
        return Ok(());
    }
    if env_guaranteed.is_empty() || !env_guaranteed.iter().all(|set| set.contains(name)) {
        return Err(RuleError::UnboundVariable {
            rule_id: rule_id.to_owned(),
            var: name.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonology::{Backness, FeatureBundle, Height, Manner, Place};

    #[test]
    fn rule_rejects_empty_id() {
        let err = Rule::new(
            "   ",
            "Empty ID",
            vec![Pattern::Literal("m".to_owned())],
            vec![],
            vec![],
            vec![ContextElement::WordBoundary],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(err, Err(RuleError::EmptyId));
    }

    #[test]
    fn rule_rejects_unconstrained_insertion() {
        let err = Rule::new(
            "R_INS",
            "Unconstrained epenthesis",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            err,
            Err(RuleError::UnconstrainedInsertion {
                rule_id: "R_INS".to_owned()
            })
        );
    }

    #[test]
    fn rule_allows_constrained_insertion_with_either_context() -> Result<(), RuleError> {
        let left_only = Rule::new(
            "R_INS_L",
            "Post-rhotic epenthesis",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![ContextElement::Segment(Pattern::Literal("r".to_owned()))],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(left_only.id(), "R_INS_L");

        let right_only = Rule::new(
            "R_INS_R",
            "Prothesis",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![],
            vec![ContextElement::Segment(Pattern::Literal("s".to_owned()))],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(right_only.id(), "R_INS_R");

        // `# V*` is anchored by the boundary even though `V*` alone is not.
        let boundary_and_closure = Rule::new(
            "R_INS_B",
            "Initial epenthesis across vowels",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![
                ContextElement::WordBoundary,
                ContextElement::ZeroOrMore(Pattern::Literal("a".to_owned())),
            ],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(boundary_and_closure.id(), "R_INS_B");
        Ok(())
    }

    #[test]
    fn rule_rejects_insertion_anchored_only_by_zero_or_more() {
        let closure = ContextElement::ZeroOrMore(Pattern::Literal("a".to_owned()));
        let err = Rule::new(
            "R_INS_STAR",
            "Epenthesis after V*",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![closure.clone()],
            vec![closure],
            ApplicationMode::LeftToRight,
        );
        assert_eq!(
            err,
            Err(RuleError::UnconstrainedInsertion {
                rule_id: "R_INS_STAR".to_owned()
            })
        );
    }

    #[test]
    fn rule_accepts_boundaries_only_at_the_outer_edges() -> Result<(), RuleError> {
        let t = || ContextElement::Segment(Pattern::Literal("t".to_owned()));
        let build = |left: Vec<ContextElement>, right: Vec<ContextElement>| {
            Rule::new(
                "R_HASH",
                "Boundary placement",
                vec![Pattern::Literal("a".to_owned())],
                vec![ReplacementItem::Literal("e".to_owned())],
                left,
                right,
                ApplicationMode::Simultaneous,
            )
        };
        let misplaced = Err(RuleError::MisplacedWordBoundary {
            rule_id: "R_HASH".to_owned(),
        });

        build(
            vec![ContextElement::WordBoundary, t()],
            vec![t(), ContextElement::WordBoundary],
        )?;
        build(
            vec![ContextElement::WordBoundary],
            vec![ContextElement::WordBoundary],
        )?;

        // `t # _`: the boundary is not the first element of the left context.
        assert_eq!(
            build(vec![t(), ContextElement::WordBoundary], vec![]),
            misplaced
        );
        // `_ # t`: the boundary is not the last element of the right context.
        assert_eq!(
            build(vec![], vec![ContextElement::WordBoundary, t()]),
            misplaced
        );
        // `# # _`: at most one boundary per side.
        assert_eq!(
            build(
                vec![ContextElement::WordBoundary, ContextElement::WordBoundary],
                vec![]
            ),
            misplaced
        );
        Ok(())
    }

    #[test]
    fn rule_rejects_invalid_source_index() {
        let err = Rule::new(
            "R_BAD_IDX",
            "Out-of-bounds mutation",
            vec![Pattern::Literal("p".to_owned())],
            vec![ReplacementItem::Mutate {
                source_index: 1,
                mutation: FeatureMutation::new().set_voiced(true).into(),
            }],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            err,
            Err(RuleError::InvalidSourceIndex {
                rule_id: "R_BAD_IDX".to_owned(),
                source_index: 1,
                target_len: 1,
            })
        );

        let err_empty_target = Rule::new(
            "R_BAD_INS",
            "Mutation on empty target",
            vec![],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().into(),
            }],
            vec![ContextElement::WordBoundary],
            vec![],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            err_empty_target,
            Err(RuleError::InvalidSourceIndex {
                rule_id: "R_BAD_INS".to_owned(),
                source_index: 0,
                target_len: 0,
            })
        );
    }

    #[test]
    fn pattern_matches_literal_features_and_natural_class() {
        let a = Segment::unstressed(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
        );
        let p = Segment::unstressed(
            "p",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false),
        );
        let vowels = NaturalClass::new("V", ["a", "e", "i", "o", "u"]);

        assert!(Pattern::Literal("a".to_owned()).matches(&a));
        assert!(!Pattern::Literal("a".to_owned()).matches(&p));

        let stop = Pattern::Features(FeaturePattern::new().with_manner(Manner::Stop));
        assert!(stop.matches(&p));
        assert!(!stop.matches(&a));

        let vowel_class = Pattern::Class(vowels);
        assert!(vowel_class.matches(&a));
        assert!(!vowel_class.matches(&p));
    }

    #[test]
    fn multi_environment_optional_and_variable_validation_work_as_specified()
    -> Result<(), RuleError> {
        let vowel = Pattern::Features(FeaturePattern::new().with_manner(Manner::Vowel));
        let nasal = Pattern::Features(FeaturePattern::new().with_manner(Manner::Nasal));
        let stop = Pattern::Constrained(
            ConstrainedPattern::new(PatternBase::Features(
                FeaturePattern::new().with_manner(Manner::Stop),
            ))
            .with_var(FeatureVar::same(FeatureField::Place, "α")),
        );

        // Multi-branch rule: optional element and boundary in different branches.
        let rule = Rule::with_environments(
            "R_MULTI",
            "Assimilate nasal before stop or rhotic-stop cluster",
            vec![nasal],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: SegmentMutation::new()
                    .with_var(FeatureVar::same(FeatureField::Place, "α")),
            }],
            vec![
                Environment::new(
                    vec![],
                    vec![
                        ContextElement::Optional(vowel.clone()),
                        ContextElement::Segment(stop.clone()),
                    ],
                ),
                Environment::new(vec![], vec![ContextElement::Segment(stop.clone())]),
            ],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(rule.environments().len(), 2);
        assert_eq!(rule.left_context(), &[]);
        assert_eq!(rule.right_context().len(), 2);

        // Empty-target insertion rejected if any branch only has Optional/ZeroOrMore.
        let bad_ins = Rule::with_environments(
            "R_BAD_INS",
            "Optional is not an anchor",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![
                Environment::new(vec![ContextElement::WordBoundary], vec![]),
                Environment::new(vec![ContextElement::Optional(vowel.clone())], vec![]),
            ],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            bad_ins,
            Err(RuleError::UnconstrainedInsertion {
                rule_id: "R_BAD_INS".to_owned(),
            })
        );

        // Empty variable name rejected.
        let bad_name = Rule::new(
            "R_EMPTY_VAR",
            "Blank var name",
            vec![Pattern::Constrained(
                ConstrainedPattern::new(PatternBase::Literal("a".to_owned()))
                    .with_var(FeatureVar::same(FeatureField::Long, "  ")),
            )],
            vec![],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            bad_name,
            Err(RuleError::InvalidVariableName {
                rule_id: "R_EMPTY_VAR".to_owned(),
                var: "  ".to_owned(),
            })
        );

        // Opposite polarity on non-bool field rejected.
        let bad_pol = Rule::new(
            "R_BAD_POL",
            "Negated Place",
            vec![Pattern::Constrained(
                ConstrainedPattern::new(PatternBase::Features(
                    FeaturePattern::new().with_manner(Manner::Stop),
                ))
                .with_var(FeatureVar::opposite(FeatureField::Place, "α")),
            )],
            vec![],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            bad_pol,
            Err(RuleError::InvalidVariablePolarity {
                rule_id: "R_BAD_POL".to_owned(),
                var: "α".to_owned(),
                field: FeatureField::Place,
            })
        );

        // Incompatible domain (`Place` vs `Bool`), whereas two `Bool` fields are compatible.
        let bad_dom = Rule::new(
            "R_BAD_DOM",
            "Incompatible domain",
            vec![stop.clone()],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: SegmentMutation::new()
                    .with_var(FeatureVar::same(FeatureField::Voiced, "α")),
            }],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            bad_dom,
            Err(RuleError::IncompatibleVariableDomain {
                rule_id: "R_BAD_DOM".to_owned(),
                var: "α".to_owned(),
                first: FeatureDomain::Place,
                second: FeatureDomain::Bool,
            })
        );

        let shared_bool = Rule::new(
            "R_BOOL_SHARE",
            "Share bool variable across Voiced and Aspirated",
            vec![Pattern::Constrained(
                ConstrainedPattern::new(PatternBase::Features(
                    FeaturePattern::new().with_manner(Manner::Stop),
                ))
                .with_var(FeatureVar::same(FeatureField::Voiced, "α")),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: SegmentMutation::new()
                    .with_var(FeatureVar::same(FeatureField::Aspirated, "α")),
            }],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(shared_bool.id(), "R_BOOL_SHARE");

        // Conflicting concrete + var constraint on the same segment.
        let conflict = Rule::new(
            "R_CONFLICT",
            "Concrete and var on voiced",
            vec![Pattern::Constrained(
                ConstrainedPattern::new(PatternBase::Features(
                    FeaturePattern::new().with_voiced(false),
                ))
                .with_var(FeatureVar::same(FeatureField::Voiced, "α")),
            )],
            vec![],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            conflict,
            Err(RuleError::ConflictingFeatureConstraint {
                rule_id: "R_CONFLICT".to_owned(),
                field: FeatureField::Voiced,
            })
        );

        // Variable used in replacement must be guaranteed bound in ALL branches (`Optional` or
        // missing branch does not guarantee binding).
        let unbound_in_optional = Rule::with_environments(
            "R_UNBOUND_OPT",
            "Var only in optional context",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: SegmentMutation::new()
                    .with_var(FeatureVar::same(FeatureField::Place, "α")),
            }],
            vec![Environment::new(
                vec![],
                vec![ContextElement::Optional(stop.clone())],
            )],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            unbound_in_optional,
            Err(RuleError::UnboundVariable {
                rule_id: "R_UNBOUND_OPT".to_owned(),
                var: "α".to_owned(),
            })
        );

        let missing_in_branch = Rule::with_environments(
            "R_UNBOUND_BRANCH",
            "Var bound only in branch 1",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::LiteralWithStress {
                symbol: "e".to_owned(),
                stress: LiteralStress::Var("s".to_owned()),
            }],
            vec![
                Environment::new(
                    vec![],
                    vec![ContextElement::Segment(Pattern::Constrained(
                        ConstrainedPattern::new(PatternBase::Literal("t".to_owned()))
                            .with_var(FeatureVar::same(FeatureField::Stress, "s")),
                    ))],
                ),
                Environment::new(vec![], vec![ContextElement::WordBoundary]),
            ],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            missing_in_branch,
            Err(RuleError::UnboundVariable {
                rule_id: "R_UNBOUND_BRANCH".to_owned(),
                var: "s".to_owned(),
            })
        );
        Ok(())
    }
}
