use crate::model::*;
use std::collections::{BTreeMap, BTreeSet};

/// A detected conflict or Pareto deadlock that requires an explicit Exception Pin ($\mathcal{E}$)
/// from the Product Owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedConflict {
    pub suggested_pin_id: String,
    pub features: BTreeSet<String>,
    pub summary: String,
    pub is_resolved: bool,
    pub resolved_by_pin: Option<String>,
    pub options: Vec<TradeoffOption>,
}

/// Computes the stratified $N$-dimensional hypergraph of Intersection Cells ($I_{A \cap B \cap \dots}$)
/// and detects Pareto conflicts or non-commutative substrate collisions.
pub fn resolve_hypergraph(
    features: &BTreeMap<String, FeatureVector>,
    exceptions: &BTreeMap<String, ExceptionPin>,
    incidents: &BTreeMap<String, IncidentRecord>,
) -> (BTreeMap<String, IntersectionCell>, Vec<DetectedConflict>) {
    let mut cells = BTreeMap::new();

    // 1. Every active feature spawns its own self-cell I({F})
    for (id, feat) in features {
        if feat.lifecycle == LifecycleState::Purged {
            continue;
        }
        let mut set = BTreeSet::new();
        set.insert(id.clone());
        let coord = IntersectionCell::format_coordinate(&set);

        cells.insert(
            coord.clone(),
            IntersectionCell {
                coordinate: coord,
                features: set.clone(),
                derivation_reason: format!(
                    "Basis self-slice [{}]",
                    feat.stratum.label()
                ),
                shared_substrates: feat.substrates.clone(),
                applied_pins: find_matching_pins(&set, exceptions),
                incident_guards: find_matching_incidents(&set, incidents),
            },
        );
    }

    // 2. Stratified Quantifier Resolution (L_k quantifies over < L_k)
    // Guarantees termination without 2^N cascades.
    let feat_list: Vec<&FeatureVector> = features
        .values()
        .filter(|f| f.lifecycle != LifecycleState::Purged)
        .collect();

    for higher in &feat_list {
        for q in &higher.quantifiers {
            for lower in &feat_list {
                if higher.id == lower.id {
                    continue;
                }
                // Enforce stratification: a quantifier at stratum L_k can only bind to features at < L_k
                if higher.stratum <= lower.stratum {
                    continue;
                }

                let tag_intersection: Vec<String> = q
                    .matches_any_tag
                    .intersection(&lower.ontology_tags)
                    .cloned()
                    .collect();
                let sub_intersection: Vec<String> = q
                    .matches_any_substrate
                    .intersection(&lower.substrates)
                    .cloned()
                    .collect();

                if !tag_intersection.is_empty() || !sub_intersection.is_empty() {
                    let mut set = BTreeSet::new();
                    set.insert(lower.id.clone());
                    set.insert(higher.id.clone());
                    let coord = IntersectionCell::format_coordinate(&set);

                    let shared_subs: BTreeSet<String> = lower
                        .substrates
                        .intersection(&higher.substrates)
                        .cloned()
                        .collect();

                    let reason = format!(
                        "Quantifier in '{}' matched '{}' on tags {:?} -> obligation '{}'",
                        higher.id, lower.id, tag_intersection, q.emits_obligation
                    );

                    cells.insert(
                        coord.clone(),
                        IntersectionCell {
                            coordinate: coord,
                            features: set.clone(),
                            derivation_reason: reason,
                            shared_substrates: shared_subs,
                            applied_pins: find_matching_pins(&set, exceptions),
                            incident_guards: find_matching_incidents(&set, incidents),
                        },
                    );
                }
            }
        }
    }

    // 3. Physical Substrate Intersection Discovery ("Dark Matter" Non-AST Coupling)
    // Two L0 domain features sharing a physical substrate (like `DBTable:todos` or `Viewport:TodoList`)
    // spawn a 2-way interaction cell so query fusion and UI screen layout choreography happen holistically.
    for i in 0..feat_list.len() {
        for j in (i + 1)..feat_list.len() {
            let f1 = feat_list[i];
            let f2 = feat_list[j];

            let shared_subs: BTreeSet<String> =
                f1.substrates.intersection(&f2.substrates).cloned().collect();

            if !shared_subs.is_empty() {
                let mut set = BTreeSet::new();
                set.insert(f1.id.clone());
                set.insert(f2.id.clone());
                let coord = IntersectionCell::format_coordinate(&set);

                cells.entry(coord.clone()).or_insert_with(|| IntersectionCell {
                    coordinate: coord,
                    features: set.clone(),
                    derivation_reason: format!(
                        "Shared physical substrate coupling: {:?}",
                        shared_subs
                    ),
                    shared_substrates: shared_subs,
                    applied_pins: find_matching_pins(&set, exceptions),
                    incident_guards: find_matching_incidents(&set, incidents),
                });
            }
        }
    }

    // 4. Detect Pareto Conflicts & Non-Commutative Ambiguities across active features
    let conflicts = detect_pareto_and_domain_conflicts(features, exceptions, &mut cells);

    (cells, conflicts)
}

fn find_matching_pins(
    cell_features: &BTreeSet<String>,
    exceptions: &BTreeMap<String, ExceptionPin>,
) -> Vec<String> {
    exceptions
        .values()
        .filter(|pin| pin.features.is_subset(cell_features) || &pin.features == cell_features)
        .map(|pin| pin.id.clone())
        .collect()
}

fn find_matching_incidents(
    cell_features: &BTreeSet<String>,
    incidents: &BTreeMap<String, IncidentRecord>,
) -> Vec<String> {
    incidents
        .values()
        .filter(|inc| {
            inc.affected_features.is_subset(cell_features)
                || &inc.affected_features == cell_features
        })
        .map(|inc| inc.id.clone())
        .collect()
}

/// Checks for fundamental domain or CAP/PACELC tradeoffs (e.g., aggressive caching vs instant GDPR erasure,
/// or recurring tasks vs GDPR full wipe) and validates whether an ExceptionPin ($\mathcal{E}$) resolves them.
fn detect_pareto_and_domain_conflicts(
    features: &BTreeMap<String, FeatureVector>,
    exceptions: &BTreeMap<String, ExceptionPin>,
    cells: &mut BTreeMap<String, IntersectionCell>,
) -> Vec<DetectedConflict> {
    let mut conflicts = Vec::new();

    // Check 1: L3 Caching / Latency Envelope vs L1 GDPR Zero-Stale-Read Erasure (CAP/PACELC Pareto Tradeoff)
    let has_cache_envelope = features
        .values()
        .find(|f| f.stratum == Stratum::L3Envelope && f.ontology_tags.contains("read_query"));
    let has_gdpr = features
        .values()
        .find(|f| f.stratum == Stratum::L1Policy && f.ontology_tags.contains("pii"));

    if let (Some(cache_feat), Some(gdpr_feat)) = (has_cache_envelope, has_gdpr) {
        let mut pair = BTreeSet::new();
        pair.insert(cache_feat.id.clone());
        pair.insert(gdpr_feat.id.clone());

        let coord = IntersectionCell::format_coordinate(&pair);
        let matching_pin = exceptions.values().find(|p| p.features == pair);

        cells.entry(coord.clone()).or_insert_with(|| IntersectionCell {
            coordinate: coord,
            features: pair.clone(),
            derivation_reason: "Pareto Envelope x Compliance Policy intersection".to_string(),
            shared_substrates: cache_feat
                .substrates
                .intersection(&gdpr_feat.substrates)
                .cloned()
                .collect(),
            applied_pins: find_matching_pins(&pair, exceptions),
            incident_guards: Vec::new(),
        });

        conflicts.push(DetectedConflict {
            suggested_pin_id: "PARETO-001-cache-vs-gdpr".to_string(),
            features: pair,
            summary: format!(
                "PACELC Tradeoff between '{}' (<25ms cached reads) and '{}' (0ms stale PII after erasure).",
                cache_feat.title, gdpr_feat.title
            ),
            is_resolved: matching_pin.is_some(),
            resolved_by_pin: matching_pin.map(|p| p.id.clone()),
            options: vec![
                TradeoffOption {
                    option_id: "OPT-SYNCHRONOUS-INVALIDATION".to_string(),
                    label: "Write-Through Cache Purge on GDPR Erase (Recommended)".to_string(),
                    business_impact: "Normal Todo reads stay fast (<2ms via in-memory cache); GDPR erasure synchronously zeroes the cache before returning.".to_string(),
                    estimated_cost_or_latency: "Read p99: 1.2ms | Erase p99: 4.5ms".to_string(),
                },
                TradeoffOption {
                    option_id: "OPT-NO-CACHE-STRICT".to_string(),
                    label: "Disable Read Caching for PII Entities".to_string(),
                    business_impact: "Guaranteed zero cache lag, but list queries hit primary storage on every view.".to_string(),
                    estimated_cost_or_latency: "Read p99: 18ms | Erase p99: 2.0ms".to_string(),
                },
            ],
        });
    }

    // Check 2: Priority / Subtask Ordering vs Auto-Archive / Completion Rules
    let l0_features: Vec<&FeatureVector> = features
        .values()
        .filter(|f| f.stratum == Stratum::L0Domain)
        .collect();

    if l0_features.len() >= 2 {
        for i in 0..l0_features.len() {
            for j in (i + 1)..l0_features.len() {
                let a = l0_features[i];
                let b = l0_features[j];
                if a.ontology_tags.contains("mutable_state")
                    && b.ontology_tags.contains("mutable_state")
                    && !a.substrates.is_disjoint(&b.substrates)
                {
                    let mut pair = BTreeSet::new();
                    pair.insert(a.id.clone());
                    pair.insert(b.id.clone());
                    let matching_pin = exceptions.values().find(|p| p.features == pair);

                    conflicts.push(DetectedConflict {
                        suggested_pin_id: format!(
                            "EXC-{}-vs-{}",
                            a.id.replace(':', "-"),
                            b.id.replace(':', "-")
                        ),
                        features: pair,
                        summary: format!(
                            "Both '{}' and '{}' mutate shared substrate {:?}. Explicit composition rule required.",
                            a.title,
                            b.title,
                            a.substrates.intersection(&b.substrates).collect::<Vec<_>>()
                        ),
                        is_resolved: matching_pin.is_some(),
                        resolved_by_pin: matching_pin.map(|p| p.id.clone()),
                        options: vec![
                            TradeoffOption {
                                option_id: "OPT-PRIORITY-SORT-FIRST".to_string(),
                                label: "Sort by Priority Score first, then Creation Order".to_string(),
                                business_impact: "Urgent/overdue tasks always float to the top of the Todo list.".to_string(),
                                estimated_cost_or_latency: "0ms overhead (fused query comparator)".to_string(),
                            },
                            TradeoffOption {
                                option_id: "OPT-CHRONOLOGICAL-FIRST".to_string(),
                                label: "Strict Chronological Order unless filtered".to_string(),
                                business_impact: "Tasks never jump positions when priority tags change.".to_string(),
                                estimated_cost_or_latency: "0ms overhead".to_string(),
                            },
                        ],
                    });
                }
            }
        }
    }

    conflicts
}
