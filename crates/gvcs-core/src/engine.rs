use crate::model::*;
use crate::parser::{parse_spec_file, sha256_normalized};
use crate::provenance::{scan_source_provenance_spans, verify_provenance_drift, SpanDrift};
use crate::solver::{resolve_hypergraph, DetectedConflict};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Loaded workspace state of a `gvcs` project.
#[derive(Debug, Clone)]
pub struct GvcsWorkspace {
    pub root: PathBuf,
    pub features: BTreeMap<String, FeatureVector>,
    pub exceptions: BTreeMap<String, ExceptionPin>,
    pub incidents: BTreeMap<String, IncidentRecord>,
    pub cells: BTreeMap<String, IntersectionCell>,
    pub conflicts: Vec<DetectedConflict>,
    pub current_spans: Vec<CodeProvenanceSpan>,
    pub existing_lock: Option<ProvenanceLock>,
}

impl GvcsWorkspace {
    /// Loads a `gvcs` project from disk, parsing all `.feature`, `.cap`, `.pareto.toml`,
    /// and `.incident.toml` files, resolving the $N$-dimensional Intersection Cell hypergraph,
    /// and scanning `src/` for `@gvcs-begin` provenance spans.
    pub fn load(project_root: &Path) -> Result<Self> {
        let specs_dir = project_root.join("specs");
        let mut features = BTreeMap::new();
        let mut exceptions = BTreeMap::new();
        let mut incidents = BTreeMap::new();

        if specs_dir.exists() {
            for entry in WalkDir::new(&specs_dir).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

                if file_name.ends_with(".pareto.toml") || file_name.ends_with(".exception.toml") {
                    let raw = fs::read_to_string(path)?;
                    let pin: ExceptionPin = toml::from_str(&raw).with_context(|| {
                        format!("Failed to parse Exception/Pareto pin: {}", path.display())
                    })?;
                    exceptions.insert(pin.id.clone(), pin);
                } else if file_name.ends_with(".incident.toml") {
                    let raw = fs::read_to_string(path)?;
                    let inc: IncidentRecord = toml::from_str(&raw).with_context(|| {
                        format!("Failed to parse Incident record: {}", path.display())
                    })?;
                    incidents.insert(inc.id.clone(), inc);
                } else if matches!(ext, "feature" | "cap") {
                    let fv = parse_spec_file(project_root, path)?;
                    features.insert(fv.id.clone(), fv);
                }
            }
        }

        let ( mut cells, conflicts) = resolve_hypergraph(&features, &exceptions, &incidents);
        let current_spans = scan_source_provenance_spans(project_root)?;

        // Register any higher-order k-way fused cells (e.g., 3-way Joint Multi-Cell Fusions) present in `src/`
        for span in &current_spans {
            if !cells.contains_key(&span.cell_coordinate) {
                let inner = span
                    .cell_coordinate
                    .trim_start_matches("I(")
                    .trim_end_matches(')');
                let parsed_feats: std::collections::BTreeSet<String> = inner
                    .split('∩')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                cells.insert(
                    span.cell_coordinate.clone(),
                    IntersectionCell {
                        coordinate: span.cell_coordinate.clone(),
                        features: parsed_feats.clone(),
                        derivation_reason: format!(
                            "Joint Multi-Cell Fused implementation span (`{}`)",
                            span.symbol
                        ),
                        shared_substrates: std::collections::BTreeSet::new(),
                        applied_pins: exceptions
                            .values()
                            .filter(|p| p.features.is_subset(&parsed_feats))
                            .map(|p| p.id.clone())
                            .collect(),
                        incident_guards: incidents
                            .values()
                            .filter(|i| i.affected_features.is_subset(&parsed_feats))
                            .map(|i| i.id.clone())
                            .collect(),
                    },
                );
            }
        }

        let lock_path = project_root.join("provenance.lock");
        let existing_lock = if lock_path.exists() {
            let raw = fs::read_to_string(&lock_path)?;
            serde_json::from_str::<ProvenanceLock>(&raw).ok()
        } else {
            None
        };

        Ok(Self {
            root: project_root.to_path_buf(),
            features,
            exceptions,
            incidents,
            cells,
            conflicts,
            current_spans,
            existing_lock,
        })
    }

    /// Computes a deterministic Merkle state hash over the entire hypergraph and code spans.
    pub fn compute_graph_state_hash(&self) -> String {
        let mut material = String::new();
        for (id, f) in &self.features {
            material.push_str(&format!("F:{}:{}\n", id, f.spec_digest));
        }
        for (id, e) in &self.exceptions {
            material.push_str(&format!("E:{}:{}\n", id, e.chosen_option));
        }
        for (id, inc) in &self.incidents {
            material.push_str(&format!("I:{}:{}\n", id, inc.title));
        }
        for span in &self.current_spans {
            material.push_str(&format!(
                "S:{}:{}:{}:{}\n",
                span.file_path, span.symbol, span.cell_coordinate, span.content_sha256
            ));
        }
        sha256_normalized(&material)
    }

    /// Generates and writes the deterministic `provenance.lock` file (`gvcs sync`).
    pub fn sync_and_write_lock(&self) -> Result<ProvenanceLock> {
        let lock = ProvenanceLock {
            version: "1.0.0".to_string(),
            graph_state_hash: self.compute_graph_state_hash(),
            features: self.features.clone(),
            cells: self.cells.clone(),
            exceptions: self.exceptions.clone(),
            incidents: self.incidents.clone(),
            code_spans: self.current_spans.clone(),
        };

        let serialized = serde_json::to_string_pretty(&lock)?;
        fs::write(self.root.join("provenance.lock"), serialized)?;
        Ok(lock)
    }

    /// Checks for AST hash drift between `src/` and `provenance.lock`.
    pub fn check_drifts(&self) -> Vec<SpanDrift> {
        match &self.existing_lock {
            Some(lock) => verify_provenance_drift(lock, &self.current_spans),
            None => Vec::new(),
        }
    }

    /// Slices the codebase by a Feature ID (`gvcs slice <feature>`), returning all
    /// Intersection Cells ($I_S$ where $F \in S$) and exact code spans in `src/`
    /// that exist because of that feature.
    pub fn slice_feature(
        &self,
        feature_query: &str,
    ) -> (Vec<IntersectionCell>, Vec<CodeProvenanceSpan>) {
        let matched_cells: Vec<IntersectionCell> = self
            .cells
            .values()
            .filter(|c| {
                c.features
                    .iter()
                    .any(|f| f == feature_query || f.contains(feature_query))
            })
            .cloned()
            .collect();

        let matched_coords: Vec<String> =
            matched_cells.iter().map(|c| c.coordinate.clone()).collect();

        let matched_spans: Vec<CodeProvenanceSpan> = self
            .current_spans
            .iter()
            .filter(|s| {
                matched_coords.contains(&s.cell_coordinate)
                    || s.cell_coordinate.contains(feature_query)
            })
            .cloned()
            .collect();

        (matched_cells, matched_spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn test_stratified_quantifier_spawns_intersection_cell() {
        let mut features = BTreeMap::new();

        let mut f0_tags = BTreeSet::new();
        f0_tags.insert("pii".to_string());
        f0_tags.insert("mutable_state".to_string());
        let mut f0_subs = BTreeSet::new();
        f0_subs.insert("DBTable:todos".to_string());

        features.insert(
            "feat:todo-crud".to_string(),
            FeatureVector {
                id: "feat:todo-crud".to_string(),
                title: "Todo CRUD".to_string(),
                spec_path: "specs/features/todo-crud.feature".to_string(),
                kind: SpecKind::Feature,
                stratum: Stratum::L0Domain,
                lifecycle: LifecycleState::Ga,
                ontology_tags: f0_tags,
                substrates: f0_subs,
                quantifiers: vec![],
                scenarios: vec!["Users can create and complete todos".to_string()],
                spec_digest: "abc".to_string(),
            },
        );

        let mut q_tags = BTreeSet::new();
        q_tags.insert("pii".to_string());
        features.insert(
            "cap:gdpr-privacy".to_string(),
            FeatureVector {
                id: "cap:gdpr-privacy".to_string(),
                title: "GDPR Privacy Safe".to_string(),
                spec_path: "specs/capabilities/gdpr-privacy.cap".to_string(),
                kind: SpecKind::Capability,
                stratum: Stratum::L1Policy,
                lifecycle: LifecycleState::Ga,
                ontology_tags: q_tags.clone(),
                substrates: BTreeSet::new(),
                quantifiers: vec![QuantifierRule {
                    statement: "For ANY feature touching PII, support 1-click erasure".to_string(),
                    matches_any_tag: q_tags,
                    matches_any_substrate: BTreeSet::new(),
                    emits_obligation: "enforce_gdpr_erasure".to_string(),
                }],
                scenarios: vec!["Erase all user data in 1 click".to_string()],
                spec_digest: "def".to_string(),
            },
        );

        let (cells, _) = resolve_hypergraph(&features, &BTreeMap::new(), &BTreeMap::new());
        assert!(cells.contains_key("I(feat:todo-crud)"));
        assert!(cells.contains_key("I(cap:gdpr-privacy)"));
        assert!(cells.contains_key("I(cap:gdpr-privacy ∩ feat:todo-crud)"));
    }
}
