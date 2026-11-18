use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Stratification levels ($L_{\text{base}}, L_0 \dots L_3$) that guarantee quantifier
/// evaluation terminates without $2^N$ cyclic cascades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Stratum {
    /// Legacy Bedrock (`L_base`): Pre-existing brownfield code mounted via `gvcs init --brownfield`.
    LBase = 0,
    /// Domain & Functional Verticals (`L0`): Core entities and business behaviors (e.g., `todo-crud`, `subtasks`).
    L0Domain = 1,
    /// Cross-Cutting Policies & Compliance (`L1`): Quantifiers over L0 (e.g., `gdpr-privacy`, `google-auth`, `offline-sync`).
    L1Policy = 2,
    /// Target Projections & UX Affordances (`L2`): Platforms, themes, and spatial widgets (e.g., `cli-tui`, `ios-app`, `minimalist-theme`).
    L2Projection = 3,
    /// Operational & Physical Envelopes (`L3`): Scale, latency, and infrastructure bounds (e.g., `fast-cache-sla`, `sqlite-wal`).
    L3Envelope = 4,
}

impl Stratum {
    pub fn label(&self) -> &'static str {
        match self {
            Stratum::LBase => "L_base (Legacy Bedrock)",
            Stratum::L0Domain => "L0 (Domain Behavior)",
            Stratum::L1Policy => "L1 (Policy & Compliance)",
            Stratum::L2Projection => "L2 (UX & Platform Projection)",
            Stratum::L3Envelope => "L3 (Operational Envelope)",
        }
    }
}

/// Deployment lifecycle state of a Feature Vector or Intersection Cell.
/// Solves the Canary Rollout, A/B testing, and Mobile Client Version Skew paradoxes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LifecycleState {
    /// Compiled in shadow/dry-run mode; does not mutate user-visible state.
    Shadow,
    /// Dual-write migration phase for database schema evolution.
    DualWrite,
    /// Active behind a runtime feature/canary percentage gate.
    Canary { percentage: u8, flag: String },
    /// Generally available (100% materialized).
    Ga,
    /// UI stripped, backend contract kept alive until old client binaries drop below threshold.
    Deprecated {
        min_client_version: String,
        tombstone_migration: Option<String>,
    },
    /// Completely excised from the codebase.
    Purged,
}

impl Default for LifecycleState {
    fn default() -> Self {
        LifecycleState::Ga
    }
}

/// Origin kind of a specification node in the repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecKind {
    /// Standard Gherkin-like business feature (`specs/features/*.feature`).
    Feature,
    /// Layperson technical or UX capability (`specs/capabilities/*.cap`).
    Capability,
    /// Parametric blueprint imported from the Feature Marketplace (`@marketplace/*`).
    MarketplaceBlueprint { package: String, version: String },
    /// Permanent human-authored native code escape hatch (`specs/native/*.native.rs`).
    NativeCell {
        source_path: String,
        entrypoint: String,
    },
}

/// A deterministic Datalog quantifier rule (`For ANY feature that...`) compiled from natural language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantifierRule {
    /// Human-readable quantifier statement from the `.feature` or `.cap` file.
    pub statement: String,
    /// Ontology tags that trigger this quantifier (e.g., `pii`, `user_data`, `mutable_state`, `read_query`, `user_facing`).
    pub matches_any_tag: BTreeSet<String>,
    /// Physical substrates that trigger this quantifier (e.g., `DBTable:todos`, `Viewport:MainList`).
    pub matches_any_substrate: BTreeSet<String>,
    /// Effect tags synthesized into the resulting Intersection Cell ($I_{A \cap B}$).
    pub emits_obligation: String,
}

/// A Feature Vector ($\vec{F}$) decomposed into its basis projections, ontology tags,
/// physical substrates, and universal quantifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureVector {
    /// Canonical identifier (e.g., `feat:todo-crud`, `cap:gdpr-privacy`, `mkt:google-login`).
    pub id: String,
    /// Human-friendly title.
    pub title: String,
    /// Path to the specification file relative to project root.
    pub spec_path: String,
    /// Kind of specification (Feature, Capability, MarketplaceBlueprint, NativeCell).
    pub kind: SpecKind,
    /// Inferred or declared stratification level ($L_{\text{base}}, L_0 \dots L_3$).
    pub stratum: Stratum,
    /// Current lifecycle state (`GA`, `CANARY`, `DEPRECATED`, etc.).
    #[serde(default)]
    pub lifecycle: LifecycleState,
    /// Locked ontology/effect tags (e.g., `pii`, `persistent_storage`, `user_facing`, `network_io`).
    pub ontology_tags: BTreeSet<String>,
    /// Physical & ambient substrate nodes touched (e.g., `DBTable:todos`, `Viewport:TodoList`, `Budget:Latency25ms`).
    pub substrates: BTreeSet<String>,
    /// Universal quantifiers (`For ANY...`) that spawn Intersection Cells with matching features.
    #[serde(default)]
    pub quantifiers: Vec<QuantifierRule>,
    /// Gherkin scenarios or invariant clauses defined in this spec.
    pub scenarios: Vec<String>,
    /// SHA-256 digest of the specification file content.
    pub spec_digest: String,
}

/// An Intersection Cell ($I_{A \cap B \cap \dots}$) representing the locus where one or more
/// Feature Vectors meet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntersectionCell {
    /// Canonical cell coordinate string, e.g., `I(feat:todo-crud)` or `I(cap:gdpr-privacy ∩ feat:todo-crud)`.
    pub coordinate: String,
    /// Sorted set of participating feature IDs ($S \subseteq \mathcal{F}$).
    pub features: BTreeSet<String>,
    /// Why this intersection cell exists (e.g., "Self-basis" or "Quantifier match on tag 'pii' + substrate 'DBTable:todos'").
    pub derivation_reason: String,
    /// Shared physical substrates at this intersection.
    pub shared_substrates: BTreeSet<String>,
    /// IDs of Exception or Pareto Tradeoff Pins ($\mathcal{E}$) governing this cell.
    pub applied_pins: Vec<String>,
    /// IDs of Incident Invariants (`.incident`) guarding this cell.
    pub incident_guards: Vec<String>,
}

impl IntersectionCell {
    pub fn format_coordinate(features: &BTreeSet<String>) -> String {
        let joined = features.iter().cloned().collect::<Vec<_>>().join(" ∩ ");
        format!("I({})", joined)
    }
}

/// Resolution option presented to a Product Owner in a Contrastive Litmus Question or
/// Pareto Tradeoff Card ($\mathcal{E}_{\text{pareto}}$).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeoffOption {
    pub option_id: String,
    pub label: String,
    pub business_impact: String,
    pub estimated_cost_or_latency: String,
}

/// An explicit Exception or Pareto Tradeoff Pin ($\mathcal{E}$) resolving a semantic collision,
/// non-commutative ordering ambiguity, or CAP/PACELC contradiction between features.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceptionPin {
    /// Unique identifier (e.g., `EXC-001-offline-vs-priority`, `PARETO-002-cache-vs-gdpr`).
    pub id: String,
    /// Sorted set of conflicting feature IDs.
    pub features: BTreeSet<String>,
    /// Plain-English description of the paradox/conflict presented to the Product Owner.
    pub conflict_summary: String,
    /// Contrastive options that were simulated for the Product Owner.
    pub options: Vec<TradeoffOption>,
    /// The chosen option ID signed off by the Product Owner.
    pub chosen_option: String,
    /// Execution ordering or declarative resolution directive enforced during synthesis.
    pub resolution_directive: String,
}

/// Status of a 3 AM Production Incident Pin (`.incident`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum IncidentStage {
    /// Temporary 3 AM code hotfix applied directly to `src/` with a strict 72h TTL.
    Tourniquet {
        applied_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        touched_file: String,
        touched_symbol: String,
    },
    /// Permanently lifted into a Negative Invariant + Regression Property Test; AST is free to regenerate.
    LiftedInvariant {
        negative_invariant: String,
        regression_test_name: String,
    },
}

/// A first-class mark left by a bugfix or adherence sweep (`specs/incidents/*.incident`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentRecord {
    pub id: String,
    pub title: String,
    pub affected_features: BTreeSet<String>,
    pub what_happened: String,
    pub stage: IncidentStage,
}

/// A tracked code span inside a committed source file in `src/`, mapped to its exact
/// Intersection Cell ($I_S$) in `provenance.lock`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeProvenanceSpan {
    /// Relative path to the committed source file (e.g., `src/store.rs`).
    pub file_path: String,
    /// Symbol or block identifier (e.g., `TodoStore::purge_gdpr_user_data`).
    pub symbol: String,
    /// Intersection Cell coordinate (e.g., `I(cap:gdpr-privacy ∩ feat:todo-crud)`).
    pub cell_coordinate: String,
    /// Start line (1-indexed).
    pub start_line: usize,
    /// End line (1-indexed).
    pub end_line: usize,
    /// SHA-256 hash of the normalized code lines inside this span.
    /// Used to detect 3 AM manual hotfix drift (`Tourniquet` trigger) and ensure hermetic builds.
    pub content_sha256: String,
    /// Associated verification tests that prove adherence for this span.
    pub verification_tests: Vec<String>,
}

/// The committed `provenance.lock` file: the bidirectional anchor between the
/// N-dimensional Feature Hypergraph and the committed, hermetic source code in `src/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceLock {
    pub version: String,
    pub graph_state_hash: String,
    pub features: BTreeMap<String, FeatureVector>,
    pub cells: BTreeMap<String, IntersectionCell>,
    pub exceptions: BTreeMap<String, ExceptionPin>,
    pub incidents: BTreeMap<String, IncidentRecord>,
    pub code_spans: Vec<CodeProvenanceSpan>,
}
