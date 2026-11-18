use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 4-Layer Priority Stack (`SpecLayer` / `Stratum`) that guarantees global rules (`Rule: For ANY...`)
/// cleanly apply to core features without circular infinite loops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Stratum {
    /// Legacy Bedrock (`L_base`): Pre-existing brownfield code mounted via `gvcs init --brownfield`.
    LBase = 0,
    /// Layer 1 — Core Feature (`L0`): Core domain entities and user behaviors (e.g., `todo-crud`, `recurring-tasks`).
    L0Domain = 1,
    /// Layer 2 — Policy & Compliance (`L1`): Global rules over core features (e.g., `gdpr-privacy`).
    L1Policy = 2,
    /// Layer 3 — UI & Platform (`L2`): User interfaces, themes, and projections (e.g., `web-ui`).
    L2Projection = 3,
    /// Layer 4 — Performance & Scale (`L3`): Speed, caching, and infrastructure SLAs (e.g., `fast-cache-sla`).
    L3Envelope = 4,
}

impl Stratum {
    pub fn label(&self) -> &'static str {
        match self {
            Stratum::LBase => "Legacy Bedrock",
            Stratum::L0Domain => "Core Feature",
            Stratum::L1Policy => "Policy & Compliance",
            Stratum::L2Projection => "UI & Platform",
            Stratum::L3Envelope => "Performance & Scale",
        }
    }
}

/// Rollout lifecycle state of a feature or capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LifecycleState {
    /// Generally available (100% live).
    Ga,
    /// Active behind a runtime feature/canary percentage flag.
    Canary { percentage: u8, flag: String },
    /// UI retired; backend contract kept alive until old client binaries drop below threshold.
    Deprecated {
        min_client_version: String,
        tombstone_migration: Option<String>,
    },
}

impl Default for LifecycleState {
    fn default() -> Self {
        LifecycleState::Ga
    }
}

/// Origin kind of a specification node in `specs/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecKind {
    /// Standard Gherkin-like product feature (`specs/features/*.feature`).
    Feature,
    /// Technical, UI, or compliance capability (`specs/capabilities/*.cap`).
    Capability,
    /// Parametric blueprint imported from the Feature Marketplace (`Uses: @marketplace/*`).
    MarketplaceBlueprint { package: String, version: String },
    /// Permanent human-authored native code escape hatch (`Native: src/...`).
    NativeCell {
        source_path: String,
        entrypoint: String,
    },
}

/// A cross-cutting global rule (`Rule: For ANY feature that...`) compiled from plain English.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantifierRule {
    /// Human-readable rule statement from the `.feature` or `.cap` file.
    pub statement: String,
    /// Concept tags that trigger this rule (e.g., `pii`, `user_data`, `mutable_state`, `read_query`, `user_facing`).
    pub matches_any_tag: BTreeSet<String>,
    /// Shared resources/substrates that trigger this rule (e.g., `DBTable:todos`, `Viewport:TodoList`).
    pub matches_any_substrate: BTreeSet<String>,
    /// Obligation enforced at the resulting Feature Overlap (`I(A ∩ B)`).
    pub emits_obligation: String,
}

/// A Feature or Capability Specification (`FeatureVector` / `FeatureSpec`) parsed from `specs/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureVector {
    /// Canonical identifier (e.g., `feat:todo-crud`, `cap:gdpr-privacy`, `native:priority-decay`).
    pub id: String,
    /// Human-friendly title from `Feature:` or `Capability:`.
    pub title: String,
    /// Path to the specification file relative to project root.
    pub spec_path: String,
    /// Kind of specification (Feature, Capability, MarketplaceBlueprint, NativeCell).
    pub kind: SpecKind,
    /// Inferred or declared layer in the 4-Layer Priority Stack.
    pub stratum: Stratum,
    /// Current rollout state (`GA`, `CANARY`, `DEPRECATED`).
    #[serde(default)]
    pub lifecycle: LifecycleState,
    /// Auto-inferred concept tags (e.g., `pii`, `persistent_storage`, `user_facing`, `read_query`).
    pub ontology_tags: BTreeSet<String>,
    /// Shared resources touched (e.g., `DBTable:todos`, `Viewport:TodoList`, `Budget:Latency25ms`).
    pub substrates: BTreeSet<String>,
    /// Global rules (`Rule: For ANY...`) that connect this capability to matching features.
    #[serde(default)]
    pub quantifiers: Vec<QuantifierRule>,
    /// Gherkin scenarios or invariant clauses defined in this spec.
    pub scenarios: Vec<String>,
    /// SHA-256 digest of the specification file content.
    pub spec_digest: String,
}

/// A Feature Overlap (`IntersectionCell`, denoted `I(A ∩ B)`) representing the exact point where
/// one or more features and policies meet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntersectionCell {
    /// Canonical coordinate string, e.g., `I(feat:todo-crud)` or `I(cap:gdpr-privacy ∩ feat:todo-crud)`.
    pub coordinate: String,
    /// Sorted set of participating feature IDs ($S \subseteq \mathcal{F}$).
    pub features: BTreeSet<String>,
    /// Plain-English explanation of why these features overlap.
    pub derivation_reason: String,
    /// Shared resources (tables, UI screens) at this overlap.
    pub shared_substrates: BTreeSet<String>,
    /// IDs of Product Decision Pins (`specs/decisions/*.toml` or `specs/exceptions/*.toml`) governing this overlap.
    pub applied_pins: Vec<String>,
    /// IDs of Bug Guards (`specs/incidents/*.toml`) protecting this overlap.
    pub incident_guards: Vec<String>,
}

impl IntersectionCell {
    /// Formats a sorted feature set into the canonical `I(a ∩ b)` coordinate string.
    pub fn format_coordinate(features: &BTreeSet<String>) -> String {
        let joined = features.iter().cloned().collect::<Vec<_>>().join(" ∩ ");
        format!("I({})", joined)
    }

    /// Formats a friendly human display badge `a + b` for terminal and Web UI output.
    pub fn format_human_list(coordinate: &str) -> String {
        let inner = coordinate
            .trim()
            .trim_start_matches("I(")
            .trim_start_matches('[')
            .trim_end_matches(')')
            .trim_end_matches(']');
        let parts: Vec<&str> = inner
            .split(|c| c == '∩' || c == '+' || c == ',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        parts.join(" + ")
    }

    /// Parses any user-written feature list (`"I(b ∩ a)"`, `"b + a"`, `"a, b"`) into a canonical
    /// `(coordinate_string, sorted_feature_set)` pair.
    pub fn parse_and_canonicalize(raw: &str) -> (String, BTreeSet<String>) {
        let inner = raw
            .trim()
            .trim_start_matches("I(")
            .trim_start_matches('[')
            .trim_end_matches(')')
            .trim_end_matches(']');
        let set: BTreeSet<String> = inner
            .split(|c| c == '∩' || c == '+' || c == ',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let coord = Self::format_coordinate(&set);
        (coord, set)
    }
}

/// Resolution option presented to a Product Owner in a Tradeoff Decision Card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeoffOption {
    pub option_id: String,
    pub label: String,
    pub business_impact: String,
    pub estimated_cost_or_latency: String,
}

/// A Product Decision Record (`ExceptionPin`) resolving a tradeoff or shared-resource collision
/// between two or more features (`specs/decisions/*.toml` or `specs/exceptions/*.toml`).
///
/// Can be written in 3 lines by a Product Owner:
/// ```toml
/// between = ["recurring-tasks", "tags-and-search"]
/// question = "When a recurring task completes and spawns next week's task, what happens to its #tags?"
/// decision = "Carry forward all #tags from the completed task to the newly spawned task."
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceptionPin {
    /// Unique identifier (auto-populated from filename if omitted).
    #[serde(default)]
    pub id: String,
    /// Set of overlapping feature IDs (supports bare slugs like `"recurring-tasks"` or full IDs `"feat:recurring-tasks"`).
    #[serde(alias = "between")]
    pub features: BTreeSet<String>,
    /// Plain-English question or summary of the tradeoff presented to the Product Owner.
    #[serde(alias = "question")]
    pub conflict_summary: String,
    /// Structured options (optional).
    #[serde(default)]
    pub options: Vec<TradeoffOption>,
    /// Concise list of alternatives considered by the Product Owner.
    #[serde(default)]
    pub alternatives: Vec<String>,
    /// The chosen decision signed off by the Product Owner.
    #[serde(alias = "decision")]
    pub chosen_option: String,
    /// Optional explicit engineering directive (defaults to `chosen_option` if omitted).
    #[serde(default, alias = "rule")]
    pub resolution_directive: String,
}

impl ExceptionPin {
    pub fn effective_directive(&self) -> &str {
        if self.resolution_directive.trim().is_empty() {
            &self.chosen_option
        } else {
            &self.resolution_directive
        }
    }
}

/// Lifecycle stage of a Production Bug Guard (`specs/incidents/*.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum IncidentStage {
    /// Temporary 3 AM code hotfix applied directly to `src/` with a 72-hour grace period.
    #[serde(alias = "hotfix_grace_period")]
    Tourniquet {
        applied_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        touched_file: String,
        touched_symbol: String,
    },
    /// Permanently locked into a regression test & invariant rule so AI never re-introduces the bug.
    #[serde(alias = "locked_regression_test")]
    LiftedInvariant {
        negative_invariant: String,
        regression_test_name: String,
    },
}

/// A permanent mark left by a production bugfix (`specs/incidents/*.incident.toml`).
///
/// Can be written in 4 lines by a Product Owner or Developer:
/// ```toml
/// title = "Double-clicking Complete on a recurring task spawned two duplicate future tasks"
/// features = ["recurring-tasks", "todo-crud"]
/// what_happened = "Two concurrent requests during a retry both spawned a recurring task."
/// never_again = "Completing an already-Completed task must be a no-op and never spawn a second task."
/// test = "test_inc_001_idempotent_complete_never_spawns_duplicate_recurrence"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentRecord {
    #[serde(default)]
    pub id: String,
    pub title: String,
    #[serde(alias = "features")]
    pub affected_features: BTreeSet<String>,
    pub what_happened: String,
    /// Plain-English "never again" invariant rule.
    #[serde(default, alias = "rule")]
    pub never_again: Option<String>,
    /// Regression test name verifying the rule.
    #[serde(default)]
    pub test: Option<String>,
    #[serde(default)]
    pub stage: Option<IncidentStage>,
}

impl IncidentRecord {
    pub fn resolved_stage(&self) -> IncidentStage {
        if let Some(ref s) = self.stage {
            return s.clone();
        }
        IncidentStage::LiftedInvariant {
            negative_invariant: self
                .never_again
                .clone()
                .unwrap_or_else(|| "Permanent regression guard".to_string()),
            regression_test_name: self
                .test
                .clone()
                .unwrap_or_else(|| "test_incident_regression".to_string()),
        }
    }
}

/// A tracked code block inside `src/` (`// @gvcs-begin` ... `// @gvcs-end`), linked in
/// `provenance.lock` to its exact Feature Overlap (`I(A ∩ B)`), Decisions, and Tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeProvenanceSpan {
    /// Relative path to the committed source file (e.g., `src/lib.rs`).
    pub file_path: String,
    /// Function or struct identifier (e.g., `TodoStore::gdpr_erase_and_export`).
    pub symbol: String,
    /// Canonical Feature Overlap coordinate (e.g., `I(cap:fast-cache-sla ∩ cap:gdpr-privacy ∩ feat:todo-crud)`).
    pub cell_coordinate: String,
    /// Parsed set of feature IDs participating in this code span.
    #[serde(default)]
    pub features: BTreeSet<String>,
    /// Start line (1-indexed).
    pub start_line: usize,
    /// End line (1-indexed).
    pub end_line: usize,
    /// SHA-256 hash of the normalized code lines inside this block.
    /// Detects unapproved manual edits (3 AM hotfix drift) in milliseconds.
    pub content_sha256: String,
    /// Associated verification tests that prove this code block satisfies its specs.
    pub verification_tests: Vec<String>,
    /// Product Decision Pins governing this code block.
    #[serde(default)]
    pub active_pins: Vec<String>,
    /// Bug Guards protecting this code block.
    #[serde(default)]
    pub active_incidents: Vec<String>,
}

/// Resolves a bare feature slug (like `"todo-crud"` or `"gdpr-privacy"`) against known full feature IDs
/// (`"feat:todo-crud"`, `"cap:gdpr-privacy"`, `"native:priority-decay"`).
pub fn resolve_feature_slug(raw: &str, known_features: &BTreeMap<String, FeatureVector>) -> String {
    let trimmed = raw.trim();
    if known_features.contains_key(trimmed) {
        return trimmed.to_string();
    }
    for full_id in known_features.keys() {
        if let Some((_, slug)) = full_id.split_once(':') {
            if slug == trimmed {
                return full_id.clone();
            }
        }
    }
    trimmed.to_string()
}

/// The committed `provenance.lock` file: the `package-lock.json` for feature intent.
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
