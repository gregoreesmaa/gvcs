use crate::model::*;
use crate::parser::sha256_normalized;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Scans committed source files in `src/` for `// @gvcs-begin` ... `// @gvcs-end`
/// provenance annotations, computing deterministic content hashes for every
/// Intersection Cell span ($I_{A \cap B}$).
///
/// Annotation syntax inside Rust/TypeScript/Go/SQL files:
/// ```text
/// // @gvcs-begin cell="I(cap:gdpr-privacy ∩ feat:todo-crud)" symbol="TodoStore::gdpr_erase_user" tests="test_gdpr_erasure_wipes_user_todos"
/// pub fn gdpr_erase_user(...) { ... }
/// // @gvcs-end
/// ```
pub fn scan_source_provenance_spans(project_root: &Path) -> Result<Vec<CodeProvenanceSpan>> {
    let src_dir = project_root.join("src");
    if !src_dir.exists() {
        return Ok(Vec::new());
    }

    let mut spans = Vec::new();

    for entry in WalkDir::new(&src_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "rs" | "ts" | "js" | "go" | "sql" | "swift" | "kt") {
            continue;
        }

        let rel_path = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read source file: {}", path.display()))?;
        let lines: Vec<&str> = content.lines().collect();

        let mut active_span: Option<(String, String, Vec<String>, usize, Vec<String>)> = None;

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();

            if let Some(meta) = trimmed.strip_prefix("// @gvcs-begin") {
                let cell = extract_attr(meta, "cell").unwrap_or_else(|| "I(unknown)".to_string());
                let symbol = extract_attr(meta, "symbol").unwrap_or_else(|| "anonymous".to_string());
                let tests_raw = extract_attr(meta, "tests").unwrap_or_default();
                let tests: Vec<String> = tests_raw
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                active_span = Some((cell, symbol, tests, line_num, Vec::new()));
            } else if trimmed.starts_with("// @gvcs-end") {
                if let Some((cell, symbol, tests, start_line, body_lines)) = active_span.take() {
                    let body_joined = body_lines.join("\n");
                    let content_sha256 = sha256_normalized(&body_joined);
                    spans.push(CodeProvenanceSpan {
                        file_path: rel_path.clone(),
                        symbol,
                        cell_coordinate: cell,
                        start_line,
                        end_line: line_num,
                        content_sha256,
                        verification_tests: tests,
                    });
                }
            } else if let Some((_, _, _, _, ref mut body_lines)) = active_span {
                body_lines.push((*line).to_string());
            }
        }
    }

    spans.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.start_line.cmp(&b.start_line))
    });

    Ok(spans)
}

fn extract_attr(meta: &str, key: &str) -> Option<String> {
    let pattern = format!("{}=\"", key);
    let start = meta.find(&pattern)? + pattern.len();
    let rest = &meta[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Drift detection result when comparing current `src/` files against `provenance.lock`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanDrift {
    pub file_path: String,
    pub symbol: String,
    pub cell_coordinate: String,
    pub locked_sha256: String,
    pub current_sha256: String,
    pub has_active_tourniquet: bool,
}

/// Compares the current `src/` code spans against the committed `provenance.lock` to detect
/// un-synced 3 AM manual code hotfixes (the Tourniquet-to-Invariant trigger).
pub fn verify_provenance_drift(
    lock: &ProvenanceLock,
    current_spans: &[CodeProvenanceSpan],
) -> Vec<SpanDrift> {
    let mut drifts = Vec::new();

    for locked in &lock.code_spans {
        if let Some(curr) = current_spans
            .iter()
            .find(|s| s.file_path == locked.file_path && s.symbol == locked.symbol)
        {
            if curr.content_sha256 != locked.content_sha256 {
                let has_tourniquet = lock.incidents.values().any(|inc| match &inc.stage {
                    IncidentStage::Tourniquet {
                        touched_file,
                        touched_symbol,
                        ..
                    } => touched_file == &locked.file_path && touched_symbol == &locked.symbol,
                    _ => false,
                });

                drifts.push(SpanDrift {
                    file_path: locked.file_path.clone(),
                    symbol: locked.symbol.clone(),
                    cell_coordinate: locked.cell_coordinate.clone(),
                    locked_sha256: locked.content_sha256.clone(),
                    current_sha256: curr.content_sha256.clone(),
                    has_active_tourniquet: has_tourniquet,
                });
            }
        }
    }

    drifts
}
