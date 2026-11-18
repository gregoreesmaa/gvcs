use crate::model::*;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Compute a hex-encoded SHA-256 digest of normalized content (CRLF normalized to LF).
pub fn sha256_normalized(content: &str) -> String {
    let normalized = content.replace("\r\n", "\n");
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

/// Parses a human-friendly `.feature` or `.cap` file with optional frontmatter directives
/// and auto-infers Stratification Level ($L_0 \dots L_3$), Ontology Tags, and Quantifiers
/// when not explicitly specified (so Product Owners never have to program a Datalog hypergraph by hand).
pub fn parse_spec_file(root: &Path, path: &Path) -> Result<FeatureVector> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Failed to read spec file: {}", path.display()))?;
    let spec_digest = sha256_normalized(&raw);
    let rel_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let mut id: Option<String> = None;
    let mut title = file_stem.replace('-', " ");
    let mut stratum: Option<Stratum> = None;
    let mut lifecycle = LifecycleState::Ga;
    let mut kind = if ext == "cap" {
        SpecKind::Capability
    } else {
        SpecKind::Feature
    };
    let mut ontology_tags = BTreeSet::new();
    let mut substrates = BTreeSet::new();
    let mut quantifiers = Vec::new();
    let mut scenarios = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse structured metadata headers if present (`@id:`, `@stratum:`, `@tag:`, `@substrate:`, `@uses:`, `@lifecycle:`, `@quantifier:`)
        if let Some(val) = trimmed.strip_prefix("@id:") {
            id = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("@stratum:") {
            stratum = Some(match val.trim().to_uppercase().as_str() {
                "L_BASE" | "LBASE" => Stratum::LBase,
                "L0" => Stratum::L0Domain,
                "L1" => Stratum::L1Policy,
                "L2" => Stratum::L2Projection,
                "L3" => Stratum::L3Envelope,
                _ => Stratum::L0Domain,
            });
        } else if let Some(val) = trimmed.strip_prefix("@tag:") {
            for t in val.split(',') {
                let tag = t.trim();
                if !tag.is_empty() {
                    ontology_tags.insert(tag.to_string());
                }
            }
        } else if let Some(val) = trimmed.strip_prefix("@substrate:") {
            for s in val.split(',') {
                let sub = s.trim();
                if !sub.is_empty() {
                    substrates.insert(sub.to_string());
                }
            }
        } else if let Some(val) = trimmed.strip_prefix("@uses:") {
            let pkg = val.trim();
            let (package, version) = pkg.split_once('@').unwrap_or((pkg, "1.0.0"));
            kind = SpecKind::MarketplaceBlueprint {
                package: package.trim().to_string(),
                version: version.trim().to_string(),
            };
        } else if let Some(val) = trimmed.strip_prefix("@native:") {
            let parts: Vec<&str> = val.splitn(2, "::").collect();
            let source_path = parts[0].trim().to_string();
            let entrypoint = parts.get(1).copied().unwrap_or("main").trim().to_string();
            kind = SpecKind::NativeCell {
                source_path,
                entrypoint,
            };
        } else if let Some(val) = trimmed.strip_prefix("@lifecycle:") {
            let v = val.trim();
            if let Some(rest) = v.strip_prefix("CANARY(").and_then(|s| s.strip_suffix(')')) {
                let parts: Vec<&str> = rest.split(',').collect();
                let pct = parts
                    .first()
                    .and_then(|p| p.trim().trim_end_matches('%').parse::<u8>().ok())
                    .unwrap_or(10);
                let flag = parts
                    .get(1)
                    .map(|f| f.trim().to_string())
                    .unwrap_or_else(|| format!("flag_{}", file_stem.replace('-', "_")));
                lifecycle = LifecycleState::Canary {
                    percentage: pct,
                    flag,
                };
            } else if let Some(rest) = v
                .strip_prefix("DEPRECATED(")
                .and_then(|s| s.strip_suffix(')'))
            {
                lifecycle = LifecycleState::Deprecated {
                    min_client_version: rest.trim().to_string(),
                    tombstone_migration: None,
                };
            }
        } else if let Some(val) = trimmed
            .strip_prefix("Feature:")
            .or_else(|| trimmed.strip_prefix("Capability:"))
        {
            title = val.trim().to_string();
        } else if let Some(val) = trimmed
            .strip_prefix("Quantifier:")
            .or_else(|| trimmed.strip_prefix("Rule: For ANY"))
        {
            let statement = if trimmed.starts_with("Rule: For ANY") {
                trimmed.trim_start_matches("Rule:").trim().to_string()
            } else {
                val.trim().to_string()
            };
            quantifiers.push(infer_quantifier_from_statement(&statement));
        } else if let Some(val) = trimmed
            .strip_prefix("Scenario:")
            .or_else(|| trimmed.strip_prefix("Invariant:"))
        {
            scenarios.push(val.trim().to_string());
        }
    }

    // Auto-infer ontology tags & stratum from natural-language content ONLY if the spec didn't declare explicit @tag: headers
    let lower = raw.to_lowercase();
    if ontology_tags.is_empty() {
        if lower.contains("customer")
            || lower.contains("user")
            || lower.contains("email")
            || lower.contains("assignee")
            || lower.contains("owner")
            || lower.contains("pii")
        {
            ontology_tags.insert("pii".to_string());
            ontology_tags.insert("user_data".to_string());
        }
        if lower.contains("create")
            || lower.contains("save")
            || lower.contains("store")
            || lower.contains("persist")
            || lower.contains("complete")
            || lower.contains("delete")
        {
            ontology_tags.insert("mutable_state".to_string());
            ontology_tags.insert("persistent_storage".to_string());
        }
        if lower.contains("list")
            || lower.contains("search")
            || lower.contains("filter")
            || lower.contains("query")
            || lower.contains("view")
        {
            ontology_tags.insert("read_query".to_string());
        }
        if lower.contains("screen")
            || lower.contains("cli")
            || lower.contains("display")
            || lower.contains("button")
            || lower.contains("badge")
        {
            ontology_tags.insert("user_facing".to_string());
        }
    }

    let inferred_stratum = stratum.unwrap_or_else(|| {
        if ext == "cap" {
            if lower.contains("latency")
                || lower.contains("scalable")
                || lower.contains("cache")
                || lower.contains("rps")
            {
                Stratum::L3Envelope
            } else if lower.contains("gdpr")
                || lower.contains("privacy")
                || lower.contains("auth")
                || lower.contains("audit")
            {
                Stratum::L1Policy
            } else {
                Stratum::L2Projection
            }
        } else if !quantifiers.is_empty() {
            Stratum::L1Policy
        } else {
            Stratum::L0Domain
        }
    });

    let resolved_id = id.unwrap_or_else(|| {
        let prefix = match kind {
            SpecKind::Feature => "feat",
            SpecKind::Capability => "cap",
            SpecKind::MarketplaceBlueprint { .. } => "mkt",
            SpecKind::NativeCell { .. } => "native",
        };
        format!("{}:{}", prefix, file_stem)
    });

    Ok(FeatureVector {
        id: resolved_id,
        title,
        spec_path: rel_path,
        kind,
        stratum: inferred_stratum,
        lifecycle,
        ontology_tags,
        substrates,
        quantifiers,
        scenarios,
        spec_digest,
    })
}

/// Deterministic extraction of Datalog quantifier triggers from natural-language quantifier statements.
fn infer_quantifier_from_statement(statement: &str) -> QuantifierRule {
    let lower = statement.to_lowercase();
    let mut matches_any_tag = BTreeSet::new();
    let mut matches_any_substrate = BTreeSet::new();

    if lower.contains("pii")
        || lower.contains("user_data")
        || lower.contains("user data")
        || lower.contains("personal")
    {
        matches_any_tag.insert("pii".to_string());
        matches_any_tag.insert("user_data".to_string());
    }
    if lower.contains("mutates") || lower.contains("mutable_state") {
        matches_any_tag.insert("mutable_state".to_string());
    }
    if lower.contains("read_query") {
        matches_any_tag.insert("read_query".to_string());
    }
    if lower.contains("user-facing") || lower.contains("user_facing") {
        matches_any_tag.insert("user_facing".to_string());
    }
    if lower.contains("dbtable:todos") {
        matches_any_substrate.insert("DBTable:todos".to_string());
    }
    if lower.contains("viewport:todolist") {
        matches_any_substrate.insert("Viewport:TodoList".to_string());
    }

    let emits_obligation = if lower.contains("Confirmation guard") || lower.contains("badges") {
        "enforce_viewport_ux_choreography".to_string()
    } else if lower.contains("erase") || lower.contains("export") || lower.contains("gdpr") {
        "enforce_gdpr_erasure_and_export".to_string()
    } else if lower.contains("cache") || lower.contains("latency") {
        "enforce_sub25ms_read_cache".to_string()
    } else if lower.contains("oauth") || lower.contains("auth") || lower.contains("identity") {
        "enforce_google_oauth_identity_guard".to_string()
    } else {
        "enforce_cross_cutting_constraint".to_string()
    };

    QuantifierRule {
        statement: statement.to_string(),
        matches_any_tag,
        matches_any_substrate,
        emits_obligation,
    }
}
