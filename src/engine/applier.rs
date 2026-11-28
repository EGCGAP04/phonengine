//! Pure rule-application engine supporting Simultaneous, LeftToRight, RightToLeft,
//! prosodic stress matching/mutations, alpha feature variables, multi-context environments,
//! optional elements, and non-local `ZeroOrMore` contexts.

use std::collections::BTreeMap;

use crate::phonology::{
    FeatureVarValue, Inventory, LiteralStress, Segment, Stress, VarBindings, Word,
};

use super::error::{EngineError, RuleError};
use super::rule::{ApplicationMode, ContextElement, Environment, Pattern, ReplacementItem, Rule};
use super::trace::{DerivationStep, DerivationTrace};

/// Applies a single phonological `rule` to `word` using `inventory` without mutating inputs.
///
/// Supports [`ApplicationMode::Simultaneous`], [`ApplicationMode::LeftToRight`], and
/// [`ApplicationMode::RightToLeft`], multiple disjunctive [`Environment`] branches, local and
/// [`ContextElement::Optional`] / [`ContextElement::ZeroOrMore`] contexts, prosodic
/// [`StressUpdate`](crate::phonology::StressUpdate)s, and alpha-variable assimilation. A [`ReplacementItem::Mutate`] applies its
/// [`StressUpdate`](crate::phonology::StressUpdate) (defaulting to `Keep`) on top of its source segment's stress, a
/// [`ReplacementItem::LiteralWithStress`] applies its explicit [`LiteralStress`], and a
/// [`ReplacementItem::Literal`] (or `LiteralStress::Inherit`) inherits the matched segment's stress
/// at the same output position unless a `Mutate` copies that `source_index` or a sibling item in
/// the same replacement explicitly assigns stress via [`LiteralStress::Set`]
/// (`Primary` / `Secondary`) or [`LiteralStress::Var`].
///
/// # Errors
///
/// - [`EngineError::UnknownSymbol`] if a [`ReplacementItem::Literal`] or
///   [`ReplacementItem::LiteralWithStress`] symbol is missing from `inventory`.
/// - [`EngineError::UnmappedFeatureBundle`] if a [`ReplacementItem::Mutate`] yields a bundle
///   absent from `inventory`.
/// - [`EngineError::InvalidRule`] defensively if rule invariants are violated during evaluation.
///
/// Rule-level errors carry the identifier of `rule`.
pub fn apply_rule(word: &Word, rule: &Rule, inventory: &Inventory) -> Result<Word, EngineError> {
    match rule.mode() {
        ApplicationMode::Simultaneous => apply_simultaneous(word, rule, inventory),
        ApplicationMode::LeftToRight => apply_left_to_right(word, rule, inventory),
        ApplicationMode::RightToLeft => apply_right_to_left(word, rule, inventory),
    }
}

/// Applies an ordered sequence of `rules` to `initial`, returning an immutable [`DerivationTrace`].
///
/// Vacuous matches where `word_after == word_before` are omitted from [`DerivationTrace::steps`].
///
/// # Errors
///
/// Propagates any [`EngineError`] produced while evaluating a rule.
pub fn derive(
    initial: &Word,
    rules: &[Rule],
    inventory: &Inventory,
) -> Result<DerivationTrace, EngineError> {
    let mut current = initial.clone();
    let mut steps = Vec::new();

    for rule in rules {
        let next = apply_rule(&current, rule, inventory)?;
        if next != current {
            steps.push(DerivationStep::new(
                rule.id(),
                rule.name(),
                current,
                next.clone(),
            ));
            current = next;
        }
    }

    Ok(DerivationTrace::new(initial.clone(), steps, current))
}

fn apply_simultaneous(
    word: &Word,
    rule: &Rule,
    inventory: &Inventory,
) -> Result<Word, EngineError> {
    let snapshot = word.segments();
    let target = rule.target();
    let mut output = Vec::with_capacity(snapshot.len());

    if target.is_empty() {
        let initial = VarBindings::new();
        for pos in 0..=snapshot.len() {
            if let Some(bindings) =
                match_environments(snapshot, pos, pos, rule.environments(), &initial)
            {
                append_replacement(&mut output, rule, &[], &bindings, inventory)?;
            }
            if let Some(seg) = snapshot.get(pos) {
                output.push(seg.clone());
            }
        }
        return Ok(Word::new(output));
    }

    let mut pos = 0;
    while pos < snapshot.len() {
        let end = pos + target.len();
        if end <= snapshot.len()
            && let Some(target_bindings) = match_target(&snapshot[pos..end], target)
            && let Some(bindings) =
                match_environments(snapshot, pos, end, rule.environments(), &target_bindings)
        {
            let matched = &snapshot[pos..end];
            append_replacement(&mut output, rule, matched, &bindings, inventory)?;
            pos += target.len();
        } else {
            output.push(snapshot[pos].clone());
            pos += 1;
        }
    }

    Ok(Word::new(output))
}

fn apply_left_to_right(
    word: &Word,
    rule: &Rule,
    inventory: &Inventory,
) -> Result<Word, EngineError> {
    let mut current = word.segments().to_vec();
    let target = rule.target();

    if target.is_empty() {
        let initial = VarBindings::new();
        let mut pos = 0;
        loop {
            let inserted_len = if let Some(bindings) =
                match_environments(&current, pos, pos, rule.environments(), &initial)
            {
                let repl = build_replacement(rule, &[], &bindings, inventory)?;
                let len = repl.len();
                current.splice(pos..pos, repl);
                len
            } else {
                0
            };
            if pos + inserted_len == current.len() {
                break;
            }
            pos += inserted_len + 1;
        }
        return Ok(Word::new(current));
    }

    let mut pos = 0;
    while pos + target.len() <= current.len() {
        let end = pos + target.len();
        if let Some(target_bindings) = match_target(&current[pos..end], target)
            && let Some(bindings) =
                match_environments(&current, pos, end, rule.environments(), &target_bindings)
        {
            let repl = build_replacement(rule, &current[pos..end], &bindings, inventory)?;
            let repl_len = repl.len();
            current.splice(pos..end, repl);
            pos += repl_len;
        } else {
            pos += 1;
        }
    }

    Ok(Word::new(current))
}

fn apply_right_to_left(
    word: &Word,
    rule: &Rule,
    inventory: &Inventory,
) -> Result<Word, EngineError> {
    let mut current = word.segments().to_vec();
    let target = rule.target();

    if target.is_empty() {
        let initial = VarBindings::new();
        let mut pos = current.len();
        loop {
            if let Some(bindings) =
                match_environments(&current, pos, pos, rule.environments(), &initial)
            {
                let repl = build_replacement(rule, &[], &bindings, inventory)?;
                current.splice(pos..pos, repl);
            }
            if pos == 0 {
                break;
            }
            pos -= 1;
        }
        return Ok(Word::new(current));
    }

    let mut end = current.len();
    while end >= target.len() {
        let start = end - target.len();
        if let Some(target_bindings) = match_target(&current[start..end], target)
            && let Some(bindings) =
                match_environments(&current, start, end, rule.environments(), &target_bindings)
        {
            let repl = build_replacement(rule, &current[start..end], &bindings, inventory)?;
            current.splice(start..end, repl);
            end = start;
        } else {
            end -= 1;
        }
    }

    Ok(Word::new(current))
}

fn match_target(slice: &[Segment], target: &[Pattern]) -> Option<VarBindings> {
    if slice.len() != target.len() {
        return None;
    }
    let mut bindings = VarBindings::new();
    for (seg, pat) in slice.iter().zip(target.iter()) {
        if !pat.matches_with_bindings(seg, &mut bindings) {
            return None;
        }
    }
    Some(bindings)
}

fn match_environments(
    snapshot: &[Segment],
    start: usize,
    end: usize,
    environments: &[Environment],
    initial_bindings: &VarBindings,
) -> Option<VarBindings> {
    if environments.is_empty() {
        return Some(initial_bindings.clone());
    }
    for env in environments {
        let left_candidates = match_left_context(snapshot, start, &env.left, initial_bindings);
        for left_bindings in left_candidates {
            if let Some(final_bindings) =
                match_right_context(snapshot, end, &env.right, &left_bindings)
            {
                return Some(final_bindings);
            }
        }
    }
    None
}

fn context_has_unbound_vars(context: &[ContextElement], bindings: &VarBindings) -> bool {
    context.iter().any(|elem| {
        let vars = match elem {
            ContextElement::WordBoundary => &[],
            ContextElement::Segment(pat)
            | ContextElement::Optional(pat)
            | ContextElement::ZeroOrMore(pat) => pat.vars(),
        };
        vars.iter().any(|var| !bindings.contains(&var.name))
    })
}

fn match_left_fast(
    snapshot: &[Segment],
    start: usize,
    left: &[ContextElement],
    bindings: &VarBindings,
) -> bool {
    let n = snapshot.len();
    let mut reachable = vec![false; n + 1];
    reachable[start] = true;
    let mut boundary_reached = false;

    for elem in left.iter().rev() {
        if boundary_reached {
            return false;
        }
        match elem {
            ContextElement::WordBoundary => {
                if !reachable[0] {
                    return false;
                }
                reachable.fill(false);
                boundary_reached = true;
            }
            ContextElement::Segment(pat) => {
                let mut next = vec![false; n + 1];
                let mut any = false;
                for idx in 1..=n {
                    if reachable[idx]
                        && pat.matches_with_fixed_bindings(&snapshot[idx - 1], bindings)
                    {
                        next[idx - 1] = true;
                        any = true;
                    }
                }
                if !any {
                    return false;
                }
                reachable = next;
            }
            ContextElement::Optional(pat) => {
                let mut next = reachable.clone();
                for idx in 1..=n {
                    if reachable[idx]
                        && pat.matches_with_fixed_bindings(&snapshot[idx - 1], bindings)
                    {
                        next[idx - 1] = true;
                    }
                }
                reachable = next;
            }
            ContextElement::ZeroOrMore(pat) => {
                for idx in (1..=n).rev() {
                    if reachable[idx]
                        && pat.matches_with_fixed_bindings(&snapshot[idx - 1], bindings)
                    {
                        reachable[idx - 1] = true;
                    }
                }
            }
        }
    }

    boundary_reached || reachable.iter().any(|&flag| flag)
}

fn match_right_fast(
    snapshot: &[Segment],
    end: usize,
    right: &[ContextElement],
    bindings: &VarBindings,
) -> bool {
    let n = snapshot.len();
    let mut reachable = vec![false; n + 1];
    reachable[end] = true;
    let mut boundary_reached = false;

    for elem in right {
        if boundary_reached {
            return false;
        }
        match elem {
            ContextElement::WordBoundary => {
                if !reachable[n] {
                    return false;
                }
                reachable.fill(false);
                boundary_reached = true;
            }
            ContextElement::Segment(pat) => {
                let mut next = vec![false; n + 1];
                let mut any = false;
                for idx in 0..n {
                    if reachable[idx] && pat.matches_with_fixed_bindings(&snapshot[idx], bindings) {
                        next[idx + 1] = true;
                        any = true;
                    }
                }
                if !any {
                    return false;
                }
                reachable = next;
            }
            ContextElement::Optional(pat) => {
                let mut next = reachable.clone();
                for idx in 0..n {
                    if reachable[idx] && pat.matches_with_fixed_bindings(&snapshot[idx], bindings) {
                        next[idx + 1] = true;
                    }
                }
                reachable = next;
            }
            ContextElement::ZeroOrMore(pat) => {
                for idx in 0..n {
                    if reachable[idx] && pat.matches_with_fixed_bindings(&snapshot[idx], bindings) {
                        reachable[idx + 1] = true;
                    }
                }
            }
        }
    }

    boundary_reached || reachable.iter().any(|&flag| flag)
}

fn insert_dp_state(
    slot: &mut BTreeMap<VarBindings, Vec<usize>>,
    bindings: VarBindings,
    key: Vec<usize>,
) {
    match slot.get_mut(&bindings) {
        Some(existing_key) => {
            if key < *existing_key {
                *existing_key = key;
            }
        }
        None => {
            slot.insert(bindings, key);
        }
    }
}

fn step_pattern_states(
    source: &BTreeMap<VarBindings, Vec<usize>>,
    dest: &mut BTreeMap<VarBindings, Vec<usize>>,
    pat: &Pattern,
    seg: &Segment,
    focus_dist: usize,
) {
    for (bindings, key) in source {
        let mut trial = bindings.clone();
        if pat.matches_with_bindings(seg, &mut trial) {
            let mut next_key = key.clone();
            if trial.len() > bindings.len() {
                next_key.push(focus_dist);
            }
            insert_dp_state(dest, trial, next_key);
        }
    }
}

fn match_left_context(
    snapshot: &[Segment],
    start: usize,
    left: &[ContextElement],
    initial_bindings: &VarBindings,
) -> Vec<VarBindings> {
    if left.is_empty() {
        return vec![initial_bindings.clone()];
    }
    if !context_has_unbound_vars(left, initial_bindings) {
        return if match_left_fast(snapshot, start, left, initial_bindings) {
            vec![initial_bindings.clone()]
        } else {
            Vec::new()
        };
    }

    let n = snapshot.len();
    let mut reachable: Vec<BTreeMap<VarBindings, Vec<usize>>> = vec![BTreeMap::new(); n + 1];
    reachable[start].insert(initial_bindings.clone(), Vec::new());
    let mut boundary_bindings = BTreeMap::new();
    let mut boundary_reached = false;

    for elem in left.iter().rev() {
        if boundary_reached {
            return Vec::new();
        }
        match elem {
            ContextElement::WordBoundary => {
                if reachable[0].is_empty() {
                    return Vec::new();
                }
                boundary_bindings = std::mem::take(&mut reachable[0]);
                for slot in &mut reachable {
                    slot.clear();
                }
                boundary_reached = true;
            }
            ContextElement::Segment(pat) => {
                let mut next: Vec<BTreeMap<VarBindings, Vec<usize>>> = vec![BTreeMap::new(); n + 1];
                for idx in 1..=n {
                    if reachable[idx].is_empty() {
                        continue;
                    }
                    let dist = start.saturating_sub(idx);
                    step_pattern_states(
                        &reachable[idx],
                        &mut next[idx - 1],
                        pat,
                        &snapshot[idx - 1],
                        dist,
                    );
                }
                if next.iter().all(BTreeMap::is_empty) {
                    return Vec::new();
                }
                reachable = next;
            }
            ContextElement::Optional(pat) => {
                let mut next = reachable.clone();
                for idx in 1..=n {
                    if reachable[idx].is_empty() {
                        continue;
                    }
                    let dist = start.saturating_sub(idx);
                    step_pattern_states(
                        &reachable[idx],
                        &mut next[idx - 1],
                        pat,
                        &snapshot[idx - 1],
                        dist,
                    );
                }
                reachable = next;
            }
            ContextElement::ZeroOrMore(pat) => {
                for idx in (1..=n).rev() {
                    if reachable[idx].is_empty() {
                        continue;
                    }
                    let dist = start.saturating_sub(idx);
                    let current_slot = reachable[idx].clone();
                    step_pattern_states(
                        &current_slot,
                        &mut reachable[idx - 1],
                        pat,
                        &snapshot[idx - 1],
                        dist,
                    );
                }
            }
        }
    }

    let final_map = if boundary_reached {
        boundary_bindings
    } else {
        let mut merged = BTreeMap::new();
        for slot in reachable {
            for (bindings, key) in slot {
                insert_dp_state(&mut merged, bindings, key);
            }
        }
        merged
    };

    let mut ranked: Vec<(Vec<usize>, VarBindings)> = final_map
        .into_iter()
        .map(|(bindings, key)| (key, bindings))
        .collect();
    ranked.sort();
    ranked.into_iter().map(|(_, bindings)| bindings).collect()
}

fn match_right_context(
    snapshot: &[Segment],
    end: usize,
    right: &[ContextElement],
    initial_bindings: &VarBindings,
) -> Option<VarBindings> {
    if right.is_empty() {
        return Some(initial_bindings.clone());
    }
    if !context_has_unbound_vars(right, initial_bindings) {
        return match_right_fast(snapshot, end, right, initial_bindings)
            .then(|| initial_bindings.clone());
    }

    let n = snapshot.len();
    let mut reachable: Vec<BTreeMap<VarBindings, Vec<usize>>> = vec![BTreeMap::new(); n + 1];
    reachable[end].insert(initial_bindings.clone(), Vec::new());
    let mut boundary_bindings = BTreeMap::new();
    let mut boundary_reached = false;

    for elem in right {
        if boundary_reached {
            return None;
        }
        match elem {
            ContextElement::WordBoundary => {
                if reachable[n].is_empty() {
                    return None;
                }
                boundary_bindings = std::mem::take(&mut reachable[n]);
                for slot in &mut reachable {
                    slot.clear();
                }
                boundary_reached = true;
            }
            ContextElement::Segment(pat) => {
                let mut next: Vec<BTreeMap<VarBindings, Vec<usize>>> = vec![BTreeMap::new(); n + 1];
                for idx in 0..n {
                    if reachable[idx].is_empty() {
                        continue;
                    }
                    let dist = idx.saturating_sub(end);
                    step_pattern_states(
                        &reachable[idx],
                        &mut next[idx + 1],
                        pat,
                        &snapshot[idx],
                        dist,
                    );
                }
                if next.iter().all(BTreeMap::is_empty) {
                    return None;
                }
                reachable = next;
            }
            ContextElement::Optional(pat) => {
                let mut next = reachable.clone();
                for idx in 0..n {
                    if reachable[idx].is_empty() {
                        continue;
                    }
                    let dist = idx.saturating_sub(end);
                    step_pattern_states(
                        &reachable[idx],
                        &mut next[idx + 1],
                        pat,
                        &snapshot[idx],
                        dist,
                    );
                }
                reachable = next;
            }
            ContextElement::ZeroOrMore(pat) => {
                for idx in 0..n {
                    if reachable[idx].is_empty() {
                        continue;
                    }
                    let dist = idx.saturating_sub(end);
                    let current_slot = reachable[idx].clone();
                    step_pattern_states(
                        &current_slot,
                        &mut reachable[idx + 1],
                        pat,
                        &snapshot[idx],
                        dist,
                    );
                }
            }
        }
    }

    let final_map = if boundary_reached {
        boundary_bindings
    } else {
        let mut merged = BTreeMap::new();
        for slot in reachable {
            for (bindings, key) in slot {
                insert_dp_state(&mut merged, bindings, key);
            }
        }
        merged
    };

    final_map
        .into_iter()
        .map(|(bindings, key)| (key, bindings))
        .min()
        .map(|(_, bindings)| bindings)
}

fn build_replacement(
    rule: &Rule,
    matched: &[Segment],
    bindings: &VarBindings,
    inventory: &Inventory,
) -> Result<Vec<Segment>, EngineError> {
    let mut built = Vec::with_capacity(rule.replacement().len());
    append_replacement(&mut built, rule, matched, bindings, inventory)?;
    Ok(built)
}

fn append_replacement(
    output: &mut Vec<Segment>,
    rule: &Rule,
    matched: &[Segment],
    bindings: &VarBindings,
    inventory: &Inventory,
) -> Result<(), EngineError> {
    let replacement = rule.replacement();
    for (position, item) in replacement.iter().enumerate() {
        match item {
            ReplacementItem::Literal(symbol) => {
                let stress = inherited_stress(replacement, matched, position);
                let unknown_symbol = |_| EngineError::UnknownSymbol {
                    rule_id: rule.id().to_owned(),
                    symbol: symbol.clone(),
                };
                output.push(inventory.segment(symbol, stress).map_err(unknown_symbol)?);
            }
            ReplacementItem::LiteralWithStress { symbol, stress } => {
                let resolved_stress = match stress {
                    LiteralStress::Inherit => inherited_stress(replacement, matched, position),
                    LiteralStress::Set(level) => *level,
                    LiteralStress::Var(name) => match bindings.get(name) {
                        Some(FeatureVarValue::Stress(level)) => level,
                        _ => {
                            return Err(EngineError::InvalidRule(RuleError::UnboundVariable {
                                rule_id: rule.id().to_owned(),
                                var: name.clone(),
                            }));
                        }
                    },
                };
                let unknown_symbol = |_| EngineError::UnknownSymbol {
                    rule_id: rule.id().to_owned(),
                    symbol: symbol.clone(),
                };
                output.push(
                    inventory
                        .segment(symbol, resolved_stress)
                        .map_err(unknown_symbol)?,
                );
            }
            ReplacementItem::Mutate {
                source_index,
                mutation,
            } => {
                let source = matched.get(*source_index).ok_or_else(|| {
                    EngineError::InvalidRule(RuleError::InvalidSourceIndex {
                        rule_id: rule.id().to_owned(),
                        source_index: *source_index,
                        target_len: rule.target().len(),
                    })
                })?;
                let mut new_bundle = mutation.features.apply(&source.bundle);
                let mut new_stress = mutation.stress.apply(source.stress);
                for var in &mutation.vars {
                    let resolved = bindings.resolve(var).ok_or_else(|| {
                        EngineError::InvalidRule(RuleError::UnboundVariable {
                            rule_id: rule.id().to_owned(),
                            var: var.name.clone(),
                        })
                    })?;
                    resolved.apply_to(var.field, &mut new_bundle, &mut new_stress);
                }
                let new_symbol = inventory.resolve(&new_bundle).ok_or_else(|| {
                    EngineError::UnmappedFeatureBundle {
                        rule_id: rule.id().to_owned(),
                        bundle: new_bundle,
                    }
                })?;
                output.push(Segment::new(new_symbol, new_bundle, new_stress));
            }
        }
    }
    Ok(())
}

/// Returns the stress inherited by a literal at `position` of `replacement`.
///
/// That is the stress of `matched[position]`, unless no such segment exists (epenthesis or a
/// longer replacement), a [`ReplacementItem::Mutate`] of the same replacement copies that source
/// index, or another item in the same replacement assigns stress explicitly via
/// [`LiteralStress::Set`] (`Primary` / `Secondary`) or [`LiteralStress::Var`].
fn inherited_stress(
    replacement: &[ReplacementItem],
    matched: &[Segment],
    position: usize,
) -> Stress {
    let copied_by_mutation = replacement.iter().any(|item| {
        matches!(item, ReplacementItem::Mutate { source_index, .. } if *source_index == position)
    });
    let explicit_stress_assigned = replacement.iter().any(|item| {
        matches!(
            item,
            ReplacementItem::LiteralWithStress {
                stress: LiteralStress::Set(Stress::Primary | Stress::Secondary)
                    | LiteralStress::Var(_),
                ..
            }
        )
    });
    match matched.get(position) {
        Some(source) if !copied_by_mutation && !explicit_stress_assigned => source.stress,
        _ => Stress::Unstressed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonology::{
        Backness, FeatureBundle, FeatureField, FeatureMutation, FeaturePattern, FeatureVar, Height,
        Manner, Place, StressMatch, StressUpdate,
    };

    fn test_inventory() -> Result<Inventory, crate::phonology::InventoryError> {
        let mut inv = Inventory::new();
        inv.insert(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
        )?;
        inv.insert(
            "a:",
            FeatureBundle::vowel(Height::Low, Backness::Central, false).with_long(true),
        )?;
        inv.insert(
            "e",
            FeatureBundle::vowel(Height::MidHigh, Backness::Front, false),
        )?;
        inv.insert(
            "i",
            FeatureBundle::vowel(Height::High, Backness::Front, false),
        )?;
        inv.insert(
            "o",
            FeatureBundle::vowel(Height::MidHigh, Backness::Back, true),
        )?;
        inv.insert(
            "u",
            FeatureBundle::vowel(Height::High, Backness::Back, true),
        )?;
        inv.insert(
            "j",
            FeatureBundle::consonant(Manner::Approximant, Place::Palatal, true),
        )?;
        inv.insert(
            "p",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false),
        )?;
        inv.insert(
            "ph",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false).with_aspirated(true),
        )?;
        inv.insert(
            "b",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, true),
        )?;
        inv.insert(
            "t",
            FeatureBundle::consonant(Manner::Stop, Place::Alveolar, false),
        )?;
        inv.insert(
            "th",
            FeatureBundle::consonant(Manner::Stop, Place::Alveolar, false).with_aspirated(true),
        )?;
        inv.insert(
            "d",
            FeatureBundle::consonant(Manner::Stop, Place::Alveolar, true),
        )?;
        inv.insert(
            "k",
            FeatureBundle::consonant(Manner::Stop, Place::Velar, false),
        )?;
        inv.insert(
            "kh",
            FeatureBundle::consonant(Manner::Stop, Place::Velar, false).with_aspirated(true),
        )?;
        inv.insert(
            "f",
            FeatureBundle::consonant(Manner::Fricative, Place::Labiodental, false),
        )?;
        inv.insert(
            "s",
            FeatureBundle::consonant(Manner::Fricative, Place::Alveolar, false),
        )?;
        inv.insert(
            "m",
            FeatureBundle::consonant(Manner::Nasal, Place::Bilabial, true),
        )?;
        inv.insert(
            "n",
            FeatureBundle::consonant(Manner::Nasal, Place::Alveolar, true),
        )?;
        inv.insert(
            "r",
            FeatureBundle::consonant(Manner::Rhotic, Place::Alveolar, true),
        )?;
        Ok(inv)
    }

    #[test]
    fn deletion_1_to_0_removes_final_m() -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let rule = Rule::new(
            "R_DEL",
            "Final m deletion",
            vec![Pattern::Literal("m".to_owned())],
            vec![],
            vec![],
            vec![ContextElement::WordBoundary],
            ApplicationMode::Simultaneous,
        )?;
        let word = Word::from_spaced("a m a m", &inv)?;
        let out = apply_rule(&word, &rule, &inv)?;
        assert_eq!(out.to_spaced(), "a m a");
        Ok(())
    }

    #[test]
    fn epenthesis_0_to_1_supports_initial_medial_and_final_contexts()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let prothesis = Rule::new(
            "R_PROTH",
            "Initial e-prothesis before s+stop",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![ContextElement::WordBoundary],
            vec![
                ContextElement::Segment(Pattern::Literal("s".to_owned())),
                ContextElement::Segment(Pattern::Features(
                    FeaturePattern::new().with_manner(Manner::Stop),
                )),
            ],
            ApplicationMode::Simultaneous,
        )?;
        let medial = Rule::new(
            "R_MED",
            "Insert b between m and r",
            vec![],
            vec![ReplacementItem::Literal("b".to_owned())],
            vec![ContextElement::Segment(Pattern::Literal("m".to_owned()))],
            vec![ContextElement::Segment(Pattern::Literal("r".to_owned()))],
            ApplicationMode::Simultaneous,
        )?;
        let paragoge = Rule::new(
            "R_FIN",
            "Insert a after final r",
            vec![],
            vec![ReplacementItem::Literal("a".to_owned())],
            vec![ContextElement::Segment(Pattern::Literal("r".to_owned()))],
            vec![ContextElement::WordBoundary],
            ApplicationMode::Simultaneous,
        )?;

        assert_eq!(
            apply_rule(&Word::from_spaced("s p a t a", &inv)?, &prothesis, &inv)?.to_spaced(),
            "e s p a t a"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("a m r a", &inv)?, &medial, &inv)?.to_spaced(),
            "a m b r a"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("m a r", &inv)?, &paragoge, &inv)?.to_spaced(),
            "m a r a"
        );
        Ok(())
    }

    #[test]
    fn diphthongization_1_to_2_and_compensatory_lengthening_2_to_1()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let diphthongize = Rule::new(
            "R_DIPH",
            "e diphthongization",
            vec![Pattern::Literal("e".to_owned())],
            vec![
                ReplacementItem::Literal("j".to_owned()),
                ReplacementItem::Mutate {
                    source_index: 0,
                    mutation: FeatureMutation::new().into(),
                },
            ],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        let stressed_e = Word::new(vec![
            inv.segment("t", Stress::Unstressed)?,
            inv.segment("e", Stress::Primary)?,
            inv.segment("r", Stress::Unstressed)?,
        ]);
        let diph_out = apply_rule(&stressed_e, &diphthongize, &inv)?;
        assert_eq!(diph_out.to_spaced(), "t j ˈe r");
        assert_eq!(diph_out.segments()[1].stress, Stress::Unstressed);
        assert_eq!(diph_out.segments()[2].stress, Stress::Primary);

        let comp_len = Rule::new(
            "R_COMP",
            "Compensatory lengthening before s",
            vec![
                Pattern::Literal("a".to_owned()),
                Pattern::Literal("n".to_owned()),
            ],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().set_long(true).into(),
            }],
            vec![],
            vec![ContextElement::Segment(Pattern::Literal("s".to_owned()))],
            ApplicationMode::Simultaneous,
        )?;
        let comp_out = apply_rule(&Word::from_spaced("m a n s a", &inv)?, &comp_len, &inv)?;
        assert_eq!(comp_out.to_spaced(), "m a: s a");
        Ok(())
    }

    #[test]
    fn metathesis_2_to_2_swaps_segments_and_preserves_stress()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let metathesis = Rule::new(
            "R_META",
            "a r -> r a metathesis",
            vec![
                Pattern::Literal("a".to_owned()),
                Pattern::Literal("r".to_owned()),
            ],
            vec![
                ReplacementItem::Mutate {
                    source_index: 1,
                    mutation: FeatureMutation::new().into(),
                },
                ReplacementItem::Mutate {
                    source_index: 0,
                    mutation: FeatureMutation::new().into(),
                },
            ],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        let input = Word::new(vec![
            inv.segment("p", Stress::Unstressed)?,
            inv.segment("a", Stress::Primary)?,
            inv.segment("r", Stress::Unstressed)?,
        ]);
        let out = apply_rule(&input, &metathesis, &inv)?;
        assert_eq!(out.to_spaced(), "p r ˈa");
        assert_eq!(out.segments()[1].stress, Stress::Unstressed);
        assert_eq!(out.segments()[2].stress, Stress::Primary);
        Ok(())
    }

    #[test]
    fn simultaneous_snapshot_prevents_cascading_and_overlapping_matches()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let chain = Rule::new(
            "R_SNAP",
            "a -> b before a",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::Literal("b".to_owned())],
            vec![],
            vec![ContextElement::Segment(Pattern::Literal("a".to_owned()))],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a a a", &inv)?, &chain, &inv)?.to_spaced(),
            "b b a"
        );

        let degeminate = Rule::new(
            "R_DEGEM",
            "a a -> b",
            vec![
                Pattern::Literal("a".to_owned()),
                Pattern::Literal("a".to_owned()),
            ],
            vec![ReplacementItem::Literal("b".to_owned())],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a a a", &inv)?, &degeminate, &inv)?.to_spaced(),
            "b a"
        );
        Ok(())
    }

    #[test]
    fn non_local_zero_or_more_supports_grassmann_boundaries_and_stacked_closures()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;

        // Grassmann's Law: [+stop, +aspirated] > [-aspirated] / _ [-aspirated]* [+aspirated]
        let grassmann = Rule::new(
            "R_GRASSMANN",
            "Grassmann's Law",
            vec![Pattern::Features(
                FeaturePattern::new()
                    .with_manner(Manner::Stop)
                    .with_aspirated(true),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().set_aspirated(false).into(),
            }],
            vec![],
            vec![
                ContextElement::ZeroOrMore(Pattern::Features(
                    FeaturePattern::new().with_aspirated(false),
                )),
                ContextElement::Segment(Pattern::Features(
                    FeaturePattern::new().with_aspirated(true),
                )),
            ],
            ApplicationMode::LeftToRight,
        )?;

        assert_eq!(
            apply_rule(&Word::from_spaced("th i kh e", &inv)?, &grassmann, &inv)?.to_spaced(),
            "t i kh e"
        );
        // Three aspirates: LeftToRight deaspirates first and second, keeping the last.
        assert_eq!(
            apply_rule(&Word::from_spaced("ph a th e kh", &inv)?, &grassmann, &inv)?.to_spaced(),
            "p a t e kh"
        );
        // Single aspirate must remain aspirated.
        assert_eq!(
            apply_rule(&Word::from_spaced("th e o s", &inv)?, &grassmann, &inv)?.to_spaced(),
            "th e o s"
        );

        // Non-local left and right contexts combined with WordBoundary and stacked ZeroOrMore.
        let vowels = Pattern::Features(FeaturePattern::new().with_manner(Manner::Vowel));
        let boundary_and_stacked = Rule::new(
            "R_NONLOCAL_BOUND",
            "Delete t after # V* and before V* V* #",
            vec![Pattern::Literal("t".to_owned())],
            vec![],
            vec![
                ContextElement::WordBoundary,
                ContextElement::ZeroOrMore(vowels.clone()),
            ],
            vec![
                ContextElement::ZeroOrMore(vowels.clone()),
                ContextElement::ZeroOrMore(vowels),
                ContextElement::WordBoundary,
            ],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(
                &Word::from_spaced("a e t o u", &inv)?,
                &boundary_and_stacked,
                &inv
            )?
            .to_spaced(),
            "a e o u"
        );
        // Consonant before `#` blocks right context `V* V* #`.
        assert_eq!(
            apply_rule(
                &Word::from_spaced("a e t o s", &inv)?,
                &boundary_and_stacked,
                &inv
            )?
            .to_spaced(),
            "a e t o s"
        );
        Ok(())
    }

    #[test]
    fn directional_modes_feed_bleed_and_terminate_for_n_to_m_and_epenthesis()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;

        // LeftToRight progressive feeding: `a > e / e C* _` across consonants.
        let non_vowels = Pattern::Features(FeaturePattern::new().with_syllabic(false));
        let prog_harmony = Rule::new(
            "R_PROG",
            "Progressive vowel harmony",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![
                ContextElement::Segment(Pattern::Literal("e".to_owned())),
                ContextElement::ZeroOrMore(non_vowels.clone()),
            ],
            vec![],
            ApplicationMode::LeftToRight,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("e t a k a", &inv)?, &prog_harmony, &inv)?.to_spaced(),
            "e t e k e"
        );

        // RightToLeft regressive feeding: `a > e / _ C* e`.
        let reg_harmony = Rule::new(
            "R_REG",
            "Regressive vowel harmony",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![],
            vec![
                ContextElement::ZeroOrMore(non_vowels),
                ContextElement::Segment(Pattern::Literal("e".to_owned())),
            ],
            ApplicationMode::RightToLeft,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a t a k e", &inv)?, &reg_harmony, &inv)?.to_spaced(),
            "e t e k e"
        );

        // RightToLeft vs LeftToRight on overlapping `a a > b` over `"a a a"`.
        let degem_ltr = Rule::new(
            "R_LTR",
            "Degeminate LTR",
            vec![
                Pattern::Literal("a".to_owned()),
                Pattern::Literal("a".to_owned()),
            ],
            vec![ReplacementItem::Literal("b".to_owned())],
            vec![],
            vec![],
            ApplicationMode::LeftToRight,
        )?;
        let degem_rtl = Rule::new(
            "R_RTL",
            "Degeminate RTL",
            vec![
                Pattern::Literal("a".to_owned()),
                Pattern::Literal("a".to_owned()),
            ],
            vec![ReplacementItem::Literal("b".to_owned())],
            vec![],
            vec![],
            ApplicationMode::RightToLeft,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a a a", &inv)?, &degem_ltr, &inv)?.to_spaced(),
            "b a"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("a a a", &inv)?, &degem_rtl, &inv)?.to_spaced(),
            "a b"
        );

        // LeftToRight 1 -> 2 (`e > e a`) must not re-scan its own emitted `e`.
        let diph_ltr = Rule::new(
            "R_DIPH_LTR",
            "Non-looping 1->2",
            vec![Pattern::Literal("e".to_owned())],
            vec![
                ReplacementItem::Literal("e".to_owned()),
                ReplacementItem::Literal("a".to_owned()),
            ],
            vec![],
            vec![],
            ApplicationMode::LeftToRight,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("t e t e", &inv)?, &diph_ltr, &inv)?.to_spaced(),
            "t e a t e a"
        );

        // Directional epenthesis (0 -> 1) in both LeftToRight and RightToLeft.
        let epen_ltr = Rule::new(
            "R_EPEN_LTR",
            "Insert e after t",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![ContextElement::Segment(Pattern::Literal("t".to_owned()))],
            vec![],
            ApplicationMode::LeftToRight,
        )?;
        let epen_rtl = Rule::new(
            "R_EPEN_RTL",
            "Insert e before t",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![],
            vec![ContextElement::Segment(Pattern::Literal("t".to_owned()))],
            ApplicationMode::RightToLeft,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("t s t", &inv)?, &epen_ltr, &inv)?.to_spaced(),
            "t e s t e"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("t s t", &inv)?, &epen_rtl, &inv)?.to_spaced(),
            "e t s e t"
        );
        Ok(())
    }

    #[test]
    fn derive_excludes_vacuous_matches_from_trace() -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let vacuous_stop_voicing = Rule::new(
            "R_VAC",
            "Voice any stop between vowels",
            vec![Pattern::Features(
                FeaturePattern::new().with_manner(Manner::Stop),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().set_voiced(true).into(),
            }],
            vec![ContextElement::Segment(Pattern::Features(
                FeaturePattern::new().with_manner(Manner::Vowel),
            ))],
            vec![ContextElement::Segment(Pattern::Features(
                FeaturePattern::new().with_manner(Manner::Vowel),
            ))],
            ApplicationMode::Simultaneous,
        )?;
        let delete_final_m = Rule::new(
            "R_DEL_M",
            "Delete final m",
            vec![Pattern::Literal("m".to_owned())],
            vec![],
            vec![],
            vec![ContextElement::WordBoundary],
            ApplicationMode::Simultaneous,
        )?;

        let word_already_voiced = Word::from_spaced("a b a m", &inv)?;
        let trace = derive(
            &word_already_voiced,
            &[vacuous_stop_voicing.clone(), delete_final_m],
            &inv,
        )?;
        assert_eq!(trace.final_word().to_spaced(), "a b a");
        assert_eq!(trace.fired_rule_ids(), vec!["R_DEL_M"]);

        let vacuous_only = derive(
            &Word::from_spaced("a b a", &inv)?,
            &[vacuous_stop_voicing],
            &inv,
        )?;
        assert!(vacuous_only.steps().is_empty());
        assert!(vacuous_only.fired_rule_ids().is_empty());
        assert_eq!(vacuous_only.initial(), vacuous_only.final_word());
        Ok(())
    }

    #[test]
    fn engine_errors_cover_unmapped_bundle_and_unknown_symbol()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;

        let voice_f = Rule::new(
            "R_UNMAPPED",
            "Voice f",
            vec![Pattern::Literal("f".to_owned())],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().set_voiced(true).into(),
            }],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        let unmapped = apply_rule(&Word::from_spaced("f a", &inv)?, &voice_f, &inv);
        assert_eq!(
            unmapped,
            Err(EngineError::UnmappedFeatureBundle {
                rule_id: "R_UNMAPPED".to_owned(),
                bundle: FeatureBundle::consonant(Manner::Fricative, Place::Labiodental, true),
            })
        );

        let unknown_lit = Rule::new(
            "R_UNKNOWN",
            "Rewrite a to z",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::Literal("z".to_owned())],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a", &inv)?, &unknown_lit, &inv),
            Err(EngineError::UnknownSymbol {
                rule_id: "R_UNKNOWN".to_owned(),
                symbol: "z".to_owned(),
            })
        );
        Ok(())
    }

    #[test]
    fn literal_replacements_inherit_stress_by_position() -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let lit = |sym: &str| ReplacementItem::Literal(sym.to_owned());
        let pat = |sym: &str| Pattern::Literal(sym.to_owned());
        let copy = |source_index: usize| ReplacementItem::Mutate {
            source_index,
            mutation: FeatureMutation::new().into(),
        };
        let rule = |target: Vec<Pattern>, replacement: Vec<ReplacementItem>| {
            Rule::new(
                "R_STRESS",
                "Stress inheritance",
                target,
                replacement,
                vec![],
                vec![],
                ApplicationMode::Simultaneous,
            )
        };
        let stresses = |word: &Word| -> Vec<(String, Stress)> {
            word.segments()
                .iter()
                .map(|seg| (seg.symbol.clone(), seg.stress))
                .collect()
        };
        let expect = |pairs: &[(&str, Stress)]| -> Vec<(String, Stress)> {
            pairs.iter().map(|(s, st)| ((*s).to_owned(), *st)).collect()
        };
        let (u, p) = (Stress::Unstressed, Stress::Primary);
        let p_stressed_o = Word::new(vec![inv.segment("p", u)?, inv.segment("o", p)?]);
        let t_stressed_e_r = Word::new(vec![
            inv.segment("t", u)?,
            inv.segment("e", p)?,
            inv.segment("r", u)?,
        ]);
        let p_stressed_a_r = Word::new(vec![
            inv.segment("p", u)?,
            inv.segment("a", p)?,
            inv.segment("r", u)?,
        ]);

        // 1 -> 1: `o > u` keeps the stress of `o`.
        let raise = rule(vec![pat("o")], vec![lit("u")])?;
        assert_eq!(
            stresses(&apply_rule(&p_stressed_o, &raise, &inv)?),
            expect(&[("p", u), ("u", p)])
        );

        // 2 -> 2 literal: stress stays on its position, unlike `$2 $1` metathesis.
        let literal_swap = rule(vec![pat("a"), pat("r")], vec![lit("r"), lit("a")])?;
        assert_eq!(
            stresses(&apply_rule(&p_stressed_a_r, &literal_swap, &inv)?),
            expect(&[("p", u), ("r", p), ("a", u)])
        );

        // 1 -> 2: `e > i e` stresses position 0; `e > i $1` keeps it on the copied `e`.
        let diph_literal = rule(vec![pat("e")], vec![lit("i"), lit("e")])?;
        assert_eq!(
            stresses(&apply_rule(&t_stressed_e_r, &diph_literal, &inv)?),
            expect(&[("t", u), ("i", p), ("e", u), ("r", u)])
        );
        let diph_copy = rule(vec![pat("e")], vec![lit("i"), copy(0)])?;
        assert_eq!(
            stresses(&apply_rule(&t_stressed_e_r, &diph_copy, &inv)?),
            expect(&[("t", u), ("i", u), ("e", p), ("r", u)])
        );

        // 2 -> 1: `a r > o` inherits the stress of the first matched segment.
        let merge = rule(vec![pat("a"), pat("r")], vec![lit("o")])?;
        assert_eq!(
            stresses(&apply_rule(&p_stressed_a_r, &merge, &inv)?),
            expect(&[("p", u), ("o", p)])
        );

        // Epenthesis has no matched segment: the inserted literal is unstressed.
        let prothesis = Rule::new(
            "R_PROTH_STRESS",
            "Prothesis before p",
            vec![],
            vec![lit("e")],
            vec![ContextElement::WordBoundary],
            vec![ContextElement::Segment(pat("p"))],
            ApplicationMode::LeftToRight,
        )?;
        assert_eq!(
            stresses(&apply_rule(&p_stressed_o, &prothesis, &inv)?),
            expect(&[("e", u), ("p", u), ("o", p)])
        );

        // Directional modes share the same replacement builder.
        let raise_rtl = Rule::new(
            "R_RAISE_RTL",
            "o > u (right-to-left)",
            vec![pat("o")],
            vec![lit("u")],
            vec![],
            vec![],
            ApplicationMode::RightToLeft,
        )?;
        assert_eq!(
            stresses(&apply_rule(&p_stressed_o, &raise_rtl, &inv)?),
            expect(&[("p", u), ("u", p)])
        );
        Ok(())
    }

    fn m5_inventory() -> Result<Inventory, crate::phonology::InventoryError> {
        let mut inv = test_inventory()?;
        inv.insert(
            "ŋ",
            FeatureBundle::consonant(Manner::Nasal, Place::Velar, true),
        )?;
        inv.insert(
            "g",
            FeatureBundle::consonant(Manner::Stop, Place::Velar, true),
        )?;
        inv.insert(
            "y",
            FeatureBundle::vowel(Height::High, Backness::Front, true),
        )?;
        inv.insert(
            "ɯ",
            FeatureBundle::vowel(Height::High, Backness::Back, false),
        )?;
        inv.insert(
            "ɨ",
            FeatureBundle::vowel(Height::High, Backness::Central, false),
        )?;
        Ok(inv)
    }

    #[test]
    fn stress_conditioned_rules_syncope_and_literal_stress_copy()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::super::rule::{ConstrainedPattern, PatternBase, SegmentMutation};

        let inv = m5_inventory()?;

        // Stressed e -> j ˈe (explicit stress assignment on second segment only).
        let stressed_e = Pattern::Constrained(
            ConstrainedPattern::new(PatternBase::Literal("e".to_owned()))
                .with_stress(StressMatch::Stressed),
        );
        let diph = Rule::new(
            "R_STRESS_DIPH",
            "Diphthongize stressed e",
            vec![stressed_e],
            vec![
                ReplacementItem::LiteralWithStress {
                    symbol: "j".to_owned(),
                    stress: LiteralStress::Set(Stress::Unstressed),
                },
                ReplacementItem::LiteralWithStress {
                    symbol: "e".to_owned(),
                    stress: LiteralStress::Set(Stress::Primary),
                },
            ],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("t ˈe n e", &inv)?, &diph, &inv)?.to_spaced(),
            "t j ˈe n e"
        );
        // Secondary stress also matches StressMatch::Stressed.
        assert_eq!(
            apply_rule(&Word::from_spaced("t ˌe n", &inv)?, &diph, &inv)?.to_spaced(),
            "t j ˈe n"
        );

        // Plain `Literal("j")` followed by `e[+stressed]` does not double-stress `j`.
        let diph_plain_j = Rule::new(
            "R_STRESS_DIPH_PLAIN",
            "Diphthongize stressed e with plain literal j",
            vec![Pattern::Constrained(
                ConstrainedPattern::new(PatternBase::Literal("e".to_owned()))
                    .with_stress(StressMatch::Stressed),
            )],
            vec![
                ReplacementItem::Literal("j".to_owned()),
                ReplacementItem::LiteralWithStress {
                    symbol: "e".to_owned(),
                    stress: LiteralStress::Set(Stress::Primary),
                },
            ],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("t ˈe n e", &inv)?, &diph_plain_j, &inv)?.to_spaced(),
            "t j ˈe n e"
        );

        // Exact StressMatch::Is(Secondary) vs StressUpdate::Set(Unstressed) and LiteralStress::Var.
        let unstressed_syncope = Rule::new(
            "R_SYNCOPE",
            "Delete unstressed i between t and r",
            vec![Pattern::Constrained(
                ConstrainedPattern::new(PatternBase::Literal("i".to_owned()))
                    .with_stress(StressMatch::Is(Stress::Unstressed)),
            )],
            vec![],
            vec![ContextElement::Segment(Pattern::Literal("t".to_owned()))],
            vec![ContextElement::Segment(Pattern::Literal("r".to_owned()))],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(
                &Word::from_spaced("p ˈe t i r", &inv)?,
                &unstressed_syncope,
                &inv
            )?
            .to_spaced(),
            "p ˈe t r"
        );
        assert_eq!(
            apply_rule(
                &Word::from_spaced("p t ˈi r", &inv)?,
                &unstressed_syncope,
                &inv
            )?
            .to_spaced(),
            "p t ˈi r"
        );

        // Mutate StressUpdate::Set(Secondary) and LiteralStress::Var copying stress from right context.
        let de_stress = Rule::new(
            "R_SEC_STRESS",
            "Demote primary stress on vowel before r",
            vec![Pattern::Constrained(
                ConstrainedPattern::new(PatternBase::Features(
                    FeaturePattern::new().with_manner(Manner::Vowel),
                ))
                .with_stress(StressMatch::Is(Stress::Primary)),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: SegmentMutation::new().with_stress(StressUpdate::Set(Stress::Secondary)),
            }],
            vec![],
            vec![ContextElement::Segment(Pattern::Literal("r".to_owned()))],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("p ˈa r t", &inv)?, &de_stress, &inv)?.to_spaced(),
            "p ˌa r t"
        );

        let copy_stress = Rule::new(
            "R_COPY_STRESS",
            "Copy stress from following vowel onto e",
            vec![Pattern::Literal("e".to_owned())],
            vec![ReplacementItem::LiteralWithStress {
                symbol: "a".to_owned(),
                stress: LiteralStress::Var("s".to_owned()),
            }],
            vec![],
            vec![ContextElement::Segment(Pattern::Constrained(
                ConstrainedPattern::new(PatternBase::Features(
                    FeaturePattern::new().with_manner(Manner::Vowel),
                ))
                .with_var(FeatureVar::same(FeatureField::Stress, "s")),
            ))],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("t e ˈi", &inv)?, &copy_stress, &inv)?.to_spaced(),
            "t ˈa ˈi"
        );
        Ok(())
    }

    #[test]
    fn alpha_variables_support_nasal_assimilation_polarity_sharing_and_harmony()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::super::rule::{ConstrainedPattern, PatternBase, SegmentMutation};

        let inv = m5_inventory()?;

        // `[nasal] > [αplace] / _ [stop, αplace]`
        let nasal_assim = Rule::new(
            "R_NASAL_ASSIM",
            "Homorganic nasal place assimilation",
            vec![Pattern::Features(
                FeaturePattern::new().with_manner(Manner::Nasal),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: SegmentMutation::new()
                    .with_var(FeatureVar::same(FeatureField::Place, "α")),
            }],
            vec![],
            vec![ContextElement::Segment(Pattern::Constrained(
                ConstrainedPattern::new(PatternBase::Features(
                    FeaturePattern::new().with_manner(Manner::Stop),
                ))
                .with_var(FeatureVar::same(FeatureField::Place, "α")),
            ))],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("i n p a", &inv)?, &nasal_assim, &inv)?.to_spaced(),
            "i m p a"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("a m k a", &inv)?, &nasal_assim, &inv)?.to_spaced(),
            "a ŋ k a"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("a m t a", &inv)?, &nasal_assim, &inv)?.to_spaced(),
            "a n t a"
        );

        // Negated polarity `-αvoiced`: voicing dissimilation `[stop] > [-αvoiced] / _ [stop, αvoiced]`
        let voice_dissim = Rule::new(
            "R_DISSIM",
            "Stop voicing dissimilation before stop",
            vec![Pattern::Features(
                FeaturePattern::new().with_manner(Manner::Stop),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: SegmentMutation::new()
                    .with_var(FeatureVar::opposite(FeatureField::Voiced, "α")),
            }],
            vec![],
            vec![ContextElement::Segment(Pattern::Constrained(
                ConstrainedPattern::new(PatternBase::Features(
                    FeaturePattern::new().with_manner(Manner::Stop),
                ))
                .with_var(FeatureVar::same(FeatureField::Voiced, "α")),
            ))],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a p t a", &inv)?, &voice_dissim, &inv)?.to_spaced(),
            "a b t a"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("a d k a", &inv)?, &voice_dissim, &inv)?.to_spaced(),
            "a d k a"
        );

        // Multi-variable vowel harmony across consonants: `[high, vowel] > [αbackness, βrounded] / [vowel, αbackness, βrounded] C* _`
        let not_vowel = Pattern::Features(FeaturePattern::new().with_syllabic(false));
        let harmony = Rule::new(
            "R_HARMONY",
            "Progressive high-vowel backness and rounding harmony",
            vec![Pattern::Features(
                FeaturePattern::new()
                    .with_manner(Manner::Vowel)
                    .with_height(crate::phonology::FieldMatch::Is(Height::High)),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: SegmentMutation::new()
                    .with_var(FeatureVar::same(FeatureField::Backness, "α"))
                    .with_var(FeatureVar::same(FeatureField::Rounded, "β")),
            }],
            vec![
                ContextElement::Segment(Pattern::Constrained(
                    ConstrainedPattern::new(PatternBase::Features(
                        FeaturePattern::new().with_manner(Manner::Vowel),
                    ))
                    .with_var(FeatureVar::same(FeatureField::Backness, "α"))
                    .with_var(FeatureVar::same(FeatureField::Rounded, "β")),
                )),
                ContextElement::ZeroOrMore(not_vowel),
            ],
            vec![],
            ApplicationMode::LeftToRight,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("y t u p i", &inv)?, &harmony, &inv)?.to_spaced(),
            "y t y p y"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("a t i p y", &inv)?, &harmony, &inv)?.to_spaced(),
            "a t ɨ p ɨ"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("t i p u", &inv)?, &harmony, &inv)?.to_spaced(),
            "t i p i"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("u t i", &inv)?, &harmony, &inv)?.to_spaced(),
            "u t u"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("o t i", &inv)?, &harmony, &inv)?.to_spaced(),
            "o t u"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("ɯ t i", &inv)?, &harmony, &inv)?.to_spaced(),
            "ɯ t ɯ"
        );

        // Cross-boolean sharing: `[+stop, αvoiced] > [αaspirated]` (voiceless stops deaspirate, voiced would aspirate — check `pʰ` -> `p`).
        let cross_bool = Rule::new(
            "R_CROSS_BOOL",
            "Link aspiration to voicing",
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
        assert_eq!(
            apply_rule(&Word::from_spaced("ph a", &inv)?, &cross_bool, &inv)?.to_spaced(),
            "p a"
        );
        Ok(())
    }

    #[test]
    fn closest_to_focus_tie_breaking_multi_environment_and_optional()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::super::rule::{ConstrainedPattern, PatternBase, SegmentMutation};

        let inv = m5_inventory()?;
        let vowel = Pattern::Features(FeaturePattern::new().with_manner(Manner::Vowel));
        let alpha_vowel = Pattern::Constrained(
            ConstrainedPattern::new(PatternBase::Features(
                FeaturePattern::new().with_manner(Manner::Vowel),
            ))
            .with_var(FeatureVar::same(FeatureField::Backness, "α")),
        );

        // Closest-to-focus: `[vowel, αbackness] V* _` over `i u t`:
        // `u` is closer to `t` (dist 0) than `i` (dist 1), so `α` binds to `Back` and `t` -> `k`?
        // Test directly on high vowel `y` after `i u`: `[vowel, αbackness] V* _`.
        let left_closest = Rule::new(
            "R_CLOSEST",
            "Assimilate vowel backness to closest preceding vowel across V*",
            vec![Pattern::Features(
                FeaturePattern::new()
                    .with_manner(Manner::Vowel)
                    .with_height(crate::phonology::FieldMatch::Is(Height::High)),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: SegmentMutation::new()
                    .with_var(FeatureVar::same(FeatureField::Backness, "α")),
            }],
            vec![
                ContextElement::Segment(alpha_vowel.clone()),
                ContextElement::ZeroOrMore(vowel.clone()),
            ],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        // Neither `e` (MidHigh) nor `o` (MidHigh) matches `target` (`Height::High`), isolating `y`.
        // Closest preceding vowel to `y` (idx 2) is `o` (Back, dist 0) rather than `e` (Front, dist 1).
        assert_eq!(
            apply_rule(&Word::from_spaced("e o y", &inv)?, &left_closest, &inv)?.to_spaced(),
            "e o u"
        );

        // Cross-context compatibility: `[vowel, αbackness] V* _ [vowel, αbackness]` over `i u t e`.
        // Closest in left is `u` (Back), which disagrees with right `e` (Front), so `{α: Front}`
        // from `i` is selected instead and `t` voices to `d`.
        let cross_ctx = Rule::new(
            "R_CROSS_CTX",
            "Voice t between matching-backness vowels across V*",
            vec![Pattern::Literal("t".to_owned())],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().set_voiced(true).into(),
            }],
            vec![
                ContextElement::Segment(alpha_vowel.clone()),
                ContextElement::ZeroOrMore(vowel),
            ],
            vec![ContextElement::Segment(alpha_vowel.clone())],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("i u t e", &inv)?, &cross_ctx, &inv)?.to_spaced(),
            "i u d e"
        );

        // Multi-environment disjunction priority + Optional `(r)` element.
        let r = Pattern::Literal("r".to_owned());
        let p_lit = Pattern::Literal("p".to_owned());
        let stop = Pattern::Constrained(
            ConstrainedPattern::new(PatternBase::Features(
                FeaturePattern::new().with_manner(Manner::Stop),
            ))
            .with_var(FeatureVar::same(FeatureField::Place, "α")),
        );
        let multi = Rule::with_environments(
            "R_MULTI_ENV",
            "Assimilate nasal across optional r before stop, else word-finally to velar",
            vec![Pattern::Features(
                FeaturePattern::new().with_manner(Manner::Nasal),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: SegmentMutation::new()
                    .with_var(FeatureVar::same(FeatureField::Place, "α")),
            }],
            vec![
                Environment::new(
                    vec![],
                    vec![ContextElement::Optional(r), ContextElement::Segment(stop)],
                ),
                Environment::new(
                    vec![ContextElement::Segment(Pattern::Constrained(
                        ConstrainedPattern::new(PatternBase::Features(
                            FeaturePattern::new().with_manner(Manner::Stop),
                        ))
                        .with_var(FeatureVar::same(FeatureField::Place, "α")),
                    ))],
                    vec![ContextElement::WordBoundary],
                ),
            ],
            ApplicationMode::Simultaneous,
        )?;
        // Branch 1 matches with optional `r` present (`n r k` -> `ŋ r k`) and absent (`n p` -> `m p`).
        assert_eq!(
            apply_rule(&Word::from_spaced("a n r k a", &inv)?, &multi, &inv)?.to_spaced(),
            "a ŋ r k a"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("a n p a", &inv)?, &multi, &inv)?.to_spaced(),
            "a m p a"
        );
        // Branch 2 fires word-finally (`k m #` -> `k ŋ #`).
        assert_eq!(
            apply_rule(&Word::from_spaced("a k m", &inv)?, &multi, &inv)?.to_spaced(),
            "a k ŋ"
        );

        // Optional in RightToLeft mode (`a > e / p (r) _`): works both for `p a` and `p r a`.
        let opt_rtl = Rule::new(
            "R_OPT_RTL",
            "Raise a after p with optional r",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![
                ContextElement::Segment(p_lit),
                ContextElement::Optional(Pattern::Literal("r".to_owned())),
            ],
            vec![],
            ApplicationMode::RightToLeft,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("p a p r a", &inv)?, &opt_rtl, &inv)?.to_spaced(),
            "p e p r e"
        );
        Ok(())
    }
}
