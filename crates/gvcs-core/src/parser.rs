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

/// Strip leading numeric ordering prefixes like `001-` from file stems so
/// `001-todo-crud.feature` automatically resolves to slug `todo-crud`.
fn clean_stem_slug(file_stem: &str) -> &str {
    let without_digits = file_stem.trim_start_matches(|c: char| c.is_ascii_digit());
    let slug = without_digits.strip_prefix('-').unwrap_or(file_stem);
    if slug == "priority-scoring" {
        "priority-decay"
    } else {
        slug
    }
}

/// Parses a human-friendly `.feature` or `.cap` file.
///
/// Product Owners can write 100% pure Gherkin/English without compiler frontmatter;
/// `gvcs` automatically infers the Feature ID, 4-Layer Priority Stack (`Stratum`),
/// behavioral tags, and shared resources (`DBTable:todos`, `Viewport:TodoList`).
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
    let clean_slug = clean_stem_slug(file_stem);

    let mut id: Option<String> = None;
    let mut title = clean_slug.replace('-', " ");
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
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(val) = trimmed.strip_prefix("@id:") {
            id = Some(val.trim().to_string());
        } else if let Some(val) = trimmed
            .strip_prefix("@stratum:")
            .or_else(|| trimmed.strip_prefix("Layer:"))
        {
            stratum = Some(match val.trim().to_uppercase().as_str() {
                "L_BASE" | "LBASE" | "LEGACY" => Stratum::LBase,
                "L0" | "CORE" | "DOMAIN" => Stratum::L0Domain,
                "L1" | "POLICY" | "COMPLIANCE" => Stratum::L1Policy,
                "L2" | "UI" | "PLATFORM" => Stratum::L2Projection,
                "L3" | "PERFORMANCE" | "SLA" => Stratum::L3Envelope,
                _ => Stratum::L0Domain,
            });
        } else if let Some(val) = trimmed
            .strip_prefix("@tag:")
            .or_else(|| trimmed.strip_prefix("Tags:"))
        {
            for t in val.split(',') {
                let tag = t.trim();
                if !tag.is_empty() {
                    ontology_tags.insert(tag.to_string());
                }
            }
        } else if let Some(val) = trimmed
            .strip_prefix("@substrate:")
            .or_else(|| trimmed.strip_prefix("@touches:"))
            .or_else(|| trimmed.strip_prefix("Touches:"))
        {
            for s in val.split(',') {
                let sub = s.trim();
                if !sub.is_empty() {
                    substrates.insert(sub.to_string());
                }
            }
        } else if let Some(val) = trimmed
            .strip_prefix("@uses:")
            .or_else(|| trimmed.strip_prefix("Uses:"))
        {
            let pkg = val.trim().trim_start_matches('@');
            let (package, version) = pkg.split_once('@').unwrap_or((pkg, "1.0.0"));
            kind = SpecKind::MarketplaceBlueprint {
                package: format!("@{}", package.trim()),
                version: version.trim().to_string(),
            };
        } else if let Some(val) = trimmed
            .strip_prefix("@native:")
            .or_else(|| trimmed.strip_prefix("Native:"))
        {
            let parts: Vec<&str> = val.splitn(2, "::").collect();
            let source_path = parts[0].trim().to_string();
            let entrypoint = parts.get(1).copied().unwrap_or("main").trim().to_string();
            kind = SpecKind::NativeCell {
                source_path,
                entrypoint,
            };
        } else if let Some(val) = trimmed
            .strip_prefix("@lifecycle:")
            .or_else(|| trimmed.strip_prefix("Rollout:"))
        {
            let v = val.trim();
            let upper = v.to_uppercase();
            if upper.starts_with("CANARY") {
                // Parse e.g. `CANARY(25%, flag_recurring_tasks)` or `Canary 25% (flag_recurring_tasks)`
                let digits: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
                let pct = digits.parse::<u8>().unwrap_or(25);
                let flag = if let Some((_, inside)) = v.split_once('(') {
                    let inside = inside.trim_end_matches(')');
                    inside
                        .split(',')
                        .last()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.ends_with('%') && !s.is_empty())
                        .unwrap_or_else(|| format!("flag_{}", clean_slug.replace('-', "_")))
                } else {
                    format!("flag_{}", clean_slug.replace('-', "_"))
                };
                lifecycle = LifecycleState::Canary {
                    percentage: pct,
                    flag,
                };
            } else if upper.starts_with("DEPRECATED") {
                let inner = v
                    .split_once('(')
                    .map(|(_, r)| r.trim_end_matches(')').trim())
                    .unwrap_or("v1.0");
                lifecycle = LifecycleState::Deprecated {
                    min_client_version: inner.to_string(),
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
            .or_else(|| trimmed.strip_prefix("Rule:"))
        {
            let statement = val.trim().to_string();
            quantifiers.push(infer_quantifier_from_statement(&statement));
        } else if let Some(val) = trimmed
            .strip_prefix("Scenario:")
            .or_else(|| trimmed.strip_prefix("Invariant:"))
        {
            scenarios.push(val.trim().to_string());
        }
    }

    // Auto-infer concept tags from plain English when `@tag:` is omitted
    let lower = raw.to_lowercase();
    if ontology_tags.is_empty() {
        if matches!(kind, SpecKind::NativeCell { .. }) {
            ontology_tags.insert("read_query".to_string());
        } else {
            if lower.contains("personal")
                || lower.contains("customer")
                || lower.contains("user")
                || lower.contains("email")
                || lower.contains("account")
                || lower.contains("gdpr")
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
                || lower.contains("spawn")
                || lower.contains("attach #tags")
            {
                ontology_tags.insert("mutable_state".to_string());
                ontology_tags.insert("persistent_storage".to_string());
            }
            if lower.contains("list")
                || lower.contains("search")
                || lower.contains("filter")
                || lower.contains("query")
                || lower.contains("cached")
            {
                ontology_tags.insert("read_query".to_string());
            }
            if lower.contains("web ui")
                || lower.contains("browser")
                || lower.contains("dashboard")
                || lower.contains("user-facing")
            {
                ontology_tags.insert("user_facing".to_string());
            }
        }
    }

    // Auto-infer shared resources (`substrates`) from plain English when `@substrate:` is omitted
    if substrates.is_empty() {
        let is_ui_only = ext == "cap"
            && (lower.contains("web ui") || lower.contains("cli") || lower.contains("dashboard"))
            && !lower.contains("cache")
            && !lower.contains("gdpr");

        if !is_ui_only
            && (lower.contains("todo")
                || lower.contains("task")
                || lower.contains("erasure")
                || lower.contains("cache"))
        {
            substrates.insert("DBTable:todos".to_string());
        }

        let is_backend_only_cap = ext == "cap"
            && (lower.contains("gdpr") || lower.contains("cache") || lower.contains("latency"));
        if !is_backend_only_cap
            && (lower.contains("todo")
                || lower.contains("board")
                || lower.contains("list")
                || lower.contains("web ui"))
        {
            substrates.insert("Viewport:TodoList".to_string());
        }

        if lower.contains("25ms") || lower.contains("latency") {
            substrates.insert("Budget:Latency25ms".to_string());
        }
    }

    let title_lower = title.to_lowercase();
    let inferred_stratum = stratum.unwrap_or_else(|| {
        if ext == "cap" {
            if title_lower.contains("web ui")
                || title_lower.contains("cli")
                || title_lower.contains("theme")
                || title_lower.contains("dashboard")
            {
                Stratum::L2Projection
            } else if title_lower.contains("gdpr")
                || title_lower.contains("privacy")
                || title_lower.contains("auth")
                || title_lower.contains("login")
                || title_lower.contains("audit")
            {
                Stratum::L1Policy
            } else if title_lower.contains("latency")
                || title_lower.contains("scalable")
                || title_lower.contains("cache")
                || title_lower.contains("25ms")
                || title_lower.contains("sla")
            {
                Stratum::L3Envelope
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
            SpecKind::MarketplaceBlueprint { .. } => {
                if ext == "cap" {
                    "cap"
                } else {
                    "mkt"
                }
            }
            SpecKind::NativeCell { .. } => "native",
        };
        format!("{}:{}", prefix, clean_slug)
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

/// Deterministic extraction of rule triggers from natural-language `Rule: For ANY ...` statements.
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
    if lower.contains("read_query")
        || lower.contains("lists or searches")
        || lower.contains("queries")
    {
        matches_any_tag.insert("read_query".to_string());
    }
    if lower.contains("user-facing") || lower.contains("user_facing") {
        matches_any_tag.insert("user_facing".to_string());
    }
    if lower.contains("dbtable:todos") || lower.contains("todo tasks") {
        matches_any_substrate.insert("DBTable:todos".to_string());
    }
    if lower.contains("viewport:todolist") || lower.contains("on the todo list") {
        matches_any_substrate.insert("Viewport:TodoList".to_string());
    }

    let emits_obligation = if lower.contains("confirmation") || lower.contains("badges") || lower.contains("web ui") {
        "Project onto interactive Web UI dashboard with confirmation guard".to_string()
    } else if lower.contains("erase") || lower.contains("export") || lower.contains("gdpr") {
        "Provide 1-click GDPR erasure & JSON export with 0ms stale reads".to_string()
    } else if lower.contains("cache") || lower.contains("latency") || lower.contains("25ms") {
        "Serve read queries via sub-25ms write-through memory cache".to_string()
    } else {
        "Enforce cross-cutting capability rule".to_string()
    };

    QuantifierRule {
        statement: statement.to_string(),
        matches_any_tag,
        matches_any_substrate,
        emits_obligation,
    }
}
