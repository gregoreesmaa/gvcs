use crate::model::*;
use std::collections::{BTreeMap, BTreeSet};

/// A detected conflict or tradeoff between two features that requires a signed-off
/// Product Decision Record (`specs/decisions/*.toml` or `specs/exceptions/*.toml`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedConflict {
    pub suggested_pin_id: String,
    pub features: BTreeSet<String>,
    pub summary: String,
    pub is_resolved: bool,
    pub resolved_by_pin: Option<String>,
    pub options: Vec<TradeoffOption>,
}

/// Central helper to construct an `IntersectionCell` with consistent shared resources,
/// applied Product Decisions, and Bug Guards.
pub fn build_intersection_cell(
    feature_set: BTreeSet<String>,
    reason: String,
    all_features: &BTreeMap<String, FeatureVector>,
    exceptions: &BTreeMap<String, ExceptionPin>,
    incidents: &BTreeMap<String, IncidentRecord>,
) -> IntersectionCell {
    let coordinate = IntersectionCell::format_coordinate(&feature_set);

    // Compute shared resources across participating features
    let mut shared_substrates: Option<BTreeSet<String>> = None;
    for fid in &feature_set {
        if let Some(f) = all_features.get(fid) {
            shared_substrates = Some(match shared_substrates {
                None => f.substrates.clone(),
                Some(acc) => acc.intersection(&f.substrates).cloned().collect(),
            });
        }
    }

    IntersectionCell {
        coordinate,
        applied_pins: find_matching_pins(&feature_set, exceptions),
        incident_guards: find_matching_incidents(&feature_set, incidents),
        features: feature_set,
        derivation_reason: reason,
        shared_substrates: shared_substrates.unwrap_or_default(),
    }
}

/// Computes the Feature Overlap Graph (`IntersectionCell`s) across the 4-Layer Priority Stack
/// and detects tradeoffs or shared-resource collisions.
pub fn resolve_hypergraph(
    features: &BTreeMap<String, FeatureVector>,
    exceptions: &BTreeMap<String, ExceptionPin>,
    incidents: &BTreeMap<String, IncidentRecord>,
) -> (BTreeMap<String, IntersectionCell>, Vec<DetectedConflict>) {
    let mut cells = BTreeMap::new();

    // 1. Every active feature registers its own base coordinate I({F})
    for (id, feat) in features {
        let mut set = BTreeSet::new();
        set.insert(id.clone());
        let cell = build_intersection_cell(
            set,
            format!("Base specification [{}]", feat.stratum.label()),
            features,
            exceptions,
            incidents,
        );
        cells.insert(cell.coordinate.clone(), cell);
    }

    // 2. Stratified Rule Resolution (higher layers apply global rules to lower layers)
    let feat_list: Vec<&FeatureVector> = features.values().collect();

    for higher in &feat_list {
        for q in &higher.quantifiers {
            for lower in &feat_list {
                if higher.id == lower.id || higher.stratum <= lower.stratum {
                    continue;
                }

                let tag_matches: Vec<String> = q
                    .matches_any_tag
                    .intersection(&lower.ontology_tags)
                    .cloned()
                    .collect();
                let sub_matches: Vec<String> = q
                    .matches_any_substrate
                    .intersection(&lower.substrates)
                    .cloned()
                    .collect();

                if !tag_matches.is_empty() || !sub_matches.is_empty() {
                    let mut set = BTreeSet::new();
                    set.insert(lower.id.clone());
                    set.insert(higher.id.clone());

                    let match_basis = if !tag_matches.is_empty() && !sub_matches.is_empty() {
                        format!(
                            "tags [{}] & resource [{}]",
                            tag_matches.join(", "),
                            sub_matches.join(", ")
                        )
                    } else if !tag_matches.is_empty() {
                        format!("tags [{}]", tag_matches.join(", "))
                    } else {
                        format!("shared resource [{}]", sub_matches.join(", "))
                    };

                    let reason = format!(
                        "Rule in '{}' applies to '{}' (matched {}) -> {}",
                        higher.id, lower.id, match_basis, q.emits_obligation
                    );

                    let cell =
                        build_intersection_cell(set, reason, features, exceptions, incidents);
                    cells.insert(cell.coordinate.clone(), cell);
                }
            }
        }
    }

    // 3. Shared Resource Overlap Discovery (e.g., two features touching `DBTable:todos` or `Viewport:TodoList`)
    for i in 0..feat_list.len() {
        for j in (i + 1)..feat_list.len() {
            let f1 = feat_list[i];
            let f2 = feat_list[j];

            let shared_subs: Vec<String> =
                f1.substrates.intersection(&f2.substrates).cloned().collect();

            if !shared_subs.is_empty() {
                let mut set = BTreeSet::new();
                set.insert(f1.id.clone());
                set.insert(f2.id.clone());
                let coord = IntersectionCell::format_coordinate(&set);

                cells.entry(coord).or_insert_with(|| {
                    build_intersection_cell(
                        set,
                        format!("Both touch shared resource [{}]", shared_subs.join(", ")),
                        features,
                        exceptions,
                        incidents,
                    )
                });
            }
        }
    }

    // 4. Detect Tradeoffs & Shared-Resource Mutations requiring a Product Decision
    let conflicts = detect_pareto_and_domain_conflicts(features, exceptions, incidents, &mut cells);

    (cells, conflicts)
}

pub fn find_matching_pins(
    cell_features: &BTreeSet<String>,
    exceptions: &BTreeMap<String, ExceptionPin>,
) -> Vec<String> {
    exceptions
        .values()
        .filter(|pin| !pin.features.is_empty() && pin.features.is_subset(cell_features))
        .map(|pin| pin.id.clone())
        .collect()
}

pub fn find_matching_incidents(
    cell_features: &BTreeSet<String>,
    incidents: &BTreeMap<String, IncidentRecord>,
) -> Vec<String> {
    incidents
        .values()
        .filter(|inc| {
            !inc.affected_features.is_empty() && inc.affected_features.is_subset(cell_features)
        })
        .map(|inc| inc.id.clone())
        .collect()
}

fn detect_pareto_and_domain_conflicts(
    features: &BTreeMap<String, FeatureVector>,
    exceptions: &BTreeMap<String, ExceptionPin>,
    incidents: &BTreeMap<String, IncidentRecord>,
    cells: &mut BTreeMap<String, IntersectionCell>,
) -> Vec<DetectedConflict> {
    let mut conflicts = Vec::new();

    // Check 1: Performance SLA Caching vs Compliance GDPR Zero-Stale-Read Erasure
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

        cells.entry(coord).or_insert_with(|| {
            build_intersection_cell(
                pair.clone(),
                "Speed SLA vs. Privacy Compliance tradeoff".to_string(),
                features,
                exceptions,
                incidents,
            )
        });

        conflicts.push(DetectedConflict {
            suggested_pin_id: "PARETO-001-cache-vs-gdpr".to_string(),
            features: pair,
            summary: format!(
                "Speed vs. Privacy tradeoff between '{}' (<25ms cached reads) and '{}' (0ms stale data after GDPR erase).",
                cache_feat.title, gdpr_feat.title
            ),
            is_resolved: matching_pin.is_some(),
            resolved_by_pin: matching_pin.map(|p| p.id.clone()),
            options: vec![
                TradeoffOption {
                    option_id: "OPT-SYNCHRONOUS-INVALIDATION".to_string(),
                    label: "Write-Through Cache Purge on GDPR Erase (Recommended)".to_string(),
                    business_impact: "Normal Todo reads stay fast (<2ms via in-memory cache); GDPR erasure synchronously clears the cache before returning.".to_string(),
                    estimated_cost_or_latency: "Read p99: 1.2ms | Erase p99: 4.5ms".to_string(),
                },
                TradeoffOption {
                    option_id: "OPT-NO-CACHE-STRICT".to_string(),
                    label: "Disable Read Caching for Personal Tasks".to_string(),
                    business_impact: "Guaranteed zero cache lag, but list queries hit storage on every view.".to_string(),
                    estimated_cost_or_latency: "Read p99: 18ms | Erase p99: 2.0ms".to_string(),
                },
            ],
        });
    }

    // Check 2: Core Features mutating the same shared storage (`DBTable:todos`)
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
                    let shared: Vec<_> =
                        a.substrates.intersection(&b.substrates).cloned().collect();

                    conflicts.push(DetectedConflict {
                        suggested_pin_id: format!(
                            "EXC-{}-vs-{}",
                            a.id.replace(':', "-"),
                            b.id.replace(':', "-")
                        ),
                        features: pair,
                        summary: format!(
                            "Both '{}' and '{}' modify shared resource [{}]. A Product Decision is required to define how they combine.",
                            a.title,
                            b.title,
                            shared.join(", ")
                        ),
                        is_resolved: matching_pin.is_some(),
                        resolved_by_pin: matching_pin.map(|p| p.id.clone()),
                        options: vec![
                            TradeoffOption {
                                option_id: "OPT-FUSED-ATOMIC".to_string(),
                                label: "Combine atomically in a single step (Recommended)".to_string(),
                                business_impact: "Both features update together immediately without extra user clicks.".to_string(),
                                estimated_cost_or_latency: "0ms overhead".to_string(),
                            },
                            TradeoffOption {
                                option_id: "OPT-INDEPENDENT".to_string(),
                                label: "Keep operations separate".to_string(),
                                business_impact: "Each feature only updates its own fields.".to_string(),
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
