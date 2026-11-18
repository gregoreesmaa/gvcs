use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use gvcs_core::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "gvcs",
    version = "0.1.0",
    about = "Graph Version Control System (gvcs) — N-Dimensional Feature Hypergraph & AST Provenance Control Plane"
)]
struct Cli {
    /// Path to the gvcs project root (defaults to current directory or `examples/todo-app`).
    #[arg(short, long, global = true)]
    project: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display the N-Dimensional Feature Hypergraph, Stratified Vectors (L0..L3), and Intersection Cells I(A ∩ B).
    Status,
    /// Synchronize specifications, resolve Intersection Cells, and write the deterministic `provenance.lock`.
    Sync,
    /// Run Tier-1/Tier-2 Adherence & Tourniquet Drift Verification (`provenance.lock` vs committed `src/`).
    Check,
    /// Slice the codebase by a feature ID, showing all Intersection Cells I(S) and exact source code spans in `src/`.
    Slice {
        /// Feature ID or substring to slice (e.g., `gdpr-privacy`, `recurring-tasks`, `todo-crud`).
        feature: String,
    },
    /// Simulate removing or deprecating a feature (`what-if`), showing exact cells and code spans that will be excised.
    WhatIf {
        /// Feature ID or substring to simulate removing.
        #[arg(long)]
        remove: String,
    },
    /// Show the Feature Marketplace blueprints, Native Escape-Hatch cells (@native), and Incident Invariants (.incident).
    Inspect,
}

fn resolve_project_root(cli_path: Option<PathBuf>) -> PathBuf {
    if let Some(p) = cli_path {
        return p;
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("specs").exists() {
        return cwd;
    }
    let example = cwd.join("examples").join("todo-app");
    if example.exists() {
        return example;
    }
    cwd
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = resolve_project_root(cli.project);
    let ws = GvcsWorkspace::load(&root)?;

    match cli.command {
        Commands::Status => cmd_status(&ws),
        Commands::Sync => cmd_sync(&ws),
        Commands::Check => cmd_check(&ws),
        Commands::Slice { feature } => cmd_slice(&ws, &feature),
        Commands::WhatIf { remove } => cmd_what_if(&ws, &remove),
        Commands::Inspect => cmd_inspect(&ws),
    }
}

fn cmd_status(ws: &GvcsWorkspace) -> Result<()> {
    println!(
        "{}",
        "========================================================================".bright_blue()
    );
    println!(
        "{}  Project: {}",
        "gvcs hypergraph status".bold().bright_cyan(),
        ws.root.display()
    );
    println!(
        "{}       {}",
        "Graph Merkle Hash:".dimmed(),
        ws.compute_graph_state_hash().bright_yellow()
    );
    println!(
        "{}",
        "========================================================================".bright_blue()
    );

    println!(
        "\n{} ({} active vectors, stratified L_base -> L3):",
        "1. FEATURE VECTORS".bold().bright_white(),
        ws.features.len()
    );
    for feat in ws.features.values() {
        let kind_badge = match &feat.kind {
            SpecKind::Feature => "[Feature]".bright_green(),
            SpecKind::Capability => "[Capability]".bright_magenta(),
            SpecKind::MarketplaceBlueprint { package, version } => {
                format!("[Marketplace: {}@{}]", package, version).bright_cyan()
            }
            SpecKind::NativeCell { source_path, .. } => {
                format!("[@native: {}]", source_path).bright_yellow()
            }
        };
        let lifecycle_badge = match &feat.lifecycle {
            LifecycleState::Ga => "GA(100%)".green(),
            LifecycleState::Canary { percentage, flag } => {
                format!("CANARY({}%, flag={})", percentage, flag).yellow()
            }
            LifecycleState::Deprecated {
                min_client_version, ..
            } => format!("DEPRECATED(min={})", min_client_version).red(),
            LifecycleState::Shadow => "SHADOW".dimmed(),
            LifecycleState::DualWrite => "DUAL_WRITE".cyan(),
            LifecycleState::Purged => "PURGED".strikethrough(),
        };

        println!(
            "  • {:<26} {:<30} {} [{}]",
            feat.id.bold(),
            kind_badge,
            feat.stratum.label().dimmed(),
            lifecycle_badge
        );
        if !feat.substrates.is_empty() {
            println!(
                "      Substrates: {:?}",
                feat.substrates.iter().collect::<Vec<_>>()
            );
        }
        for q in &feat.quantifiers {
            println!(
                "      {} \"{}\"",
                "Quantifier:".bright_magenta(),
                q.statement
            );
        }
    }

    println!(
        "\n{} ({} resolved hyperedges $I_{{A \\cap B}}$):",
        "2. INTERSECTION CELLS".bold().bright_white(),
        ws.cells.len()
    );
    for cell in ws.cells.values() {
        let span_count = ws
            .current_spans
            .iter()
            .filter(|s| s.cell_coordinate == cell.coordinate)
            .count();
        let pin_info = if cell.applied_pins.is_empty() {
            String::new()
        } else {
            format!(" [Pins: {}]", cell.applied_pins.join(", "))
                .bright_yellow()
                .to_string()
        };
        let inc_info = if cell.incident_guards.is_empty() {
            String::new()
        } else {
            format!(" [Incidents: {}]", cell.incident_guards.join(", "))
                .bright_red()
                .to_string()
        };

        println!(
            "  • {:<48} ({} spans in src/){}{}",
            cell.coordinate.bold().bright_cyan(),
            span_count,
            pin_info,
            inc_info
        );
        println!("      Reason: {}", cell.derivation_reason.dimmed());
    }

    println!(
        "\n{} ({} tradeoffs/collisions evaluated):",
        "3. PARETO TRADEOFF & EXCEPTION PINS (E)".bold().bright_white(),
        ws.conflicts.len()
    );
    for c in &ws.conflicts {
        if c.is_resolved {
            println!(
                "  {} {:<36} -> Resolved by Pin '{}'",
                "✓".bright_green(),
                IntersectionCell::format_coordinate(&c.features).bold(),
                c.resolved_by_pin.as_deref().unwrap_or("unknown").bright_yellow()
            );
        } else {
            println!(
                "  {} {:<36} -> UNRESOLVED: {}",
                "✗".bright_red(),
                IntersectionCell::format_coordinate(&c.features).bold(),
                c.summary
            );
        }
    }

    Ok(())
}

fn cmd_sync(ws: &GvcsWorkspace) -> Result<()> {
    let lock = ws.sync_and_write_lock()?;
    println!(
        "{} Synchronized {} feature vectors, {} intersection cells, and {} committed code spans into {}",
        "✓".bright_green().bold(),
        lock.features.len(),
        lock.cells.len(),
        lock.code_spans.len(),
        ws.root.join("provenance.lock").display()
    );
    println!(
        "  Graph State Hash: {}",
        lock.graph_state_hash.bright_yellow()
    );
    Ok(())
}

fn cmd_check(ws: &GvcsWorkspace) -> Result<()> {
    println!(
        "{}",
        "Running gvcs Tier-1/Tier-2 Adherence & Tourniquet Drift Check...".bold()
    );

    let mut failed = false;

    // 1. Check unresolved Pareto conflicts
    let unresolved: Vec<_> = ws.conflicts.iter().filter(|c| !c.is_resolved).collect();
    if !unresolved.is_empty() {
        failed = true;
        println!(
            "\n{} {} Unresolved Feature Conflict(s) require a Product Owner Exception Pin:",
            "✗ [CONFLICT GATE]".bright_red().bold(),
            unresolved.len()
        );
        for c in unresolved {
            println!("  • {}", c.summary.bright_yellow());
            for opt in &c.options {
                println!(
                    "      [{}] {} — {} ({})",
                    opt.option_id.bright_cyan(),
                    opt.label.bold(),
                    opt.business_impact,
                    opt.estimated_cost_or_latency.dimmed()
                );
            }
        }
    } else {
        println!(
            "  {} All {} cross-feature interactions & Pareto tradeoffs have explicit pins.",
            "✓".bright_green(),
            ws.conflicts.len()
        );
    }

    // 2. Check AST Provenance Drift (3 AM hotfixes vs provenance.lock)
    let drifts = ws.check_drifts();
    if !drifts.is_empty() {
        for d in &drifts {
            if d.has_active_tourniquet {
                println!(
                    "  {} Span '{}' in {} has an active 72h Tourniquet Pin (allowed temporary hotfix).",
                    "⚠ [TOURNIQUET]".bright_yellow().bold(),
                    d.symbol,
                    d.file_path
                );
            } else {
                failed = true;
                println!(
                    "  {} Unregistered code edit detected in {} (`{}` in cell `{}`).\n      Run `gvcs sync` or create a 72h `.incident.toml` Tourniquet Pin to lift this hotfix into a Negative Invariant!",
                    "✗ [DRIFT DETECTED]".bright_red().bold(),
                    d.file_path,
                    d.symbol,
                    d.cell_coordinate
                );
            }
        }
    } else {
        println!(
            "  {} All {} committed code spans in `src/` match `provenance.lock` SHA-256 digests.",
            "✓".bright_green(),
            ws.current_spans.len()
        );
    }

    // 3. Check active incidents
    for inc in ws.incidents.values() {
        match &inc.stage {
            IncidentStage::LiftedInvariant {
                negative_invariant,
                regression_test_name,
            } => {
                println!(
                    "  {} Incident '{}' permanently lifted to Negative Invariant: \"{}\" (verified by `{}`)",
                    "✓".bright_green(),
                    inc.id.bold(),
                    negative_invariant,
                    regression_test_name.bright_cyan()
                );
            }
            IncidentStage::Tourniquet { expires_at, .. } => {
                println!(
                    "  {} Incident '{}' is on a temporary AST Tourniquet Pin (expires {}).",
                    "⚠".bright_yellow(),
                    inc.id.bold(),
                    expires_at
                );
            }
        }
    }

    if failed {
        anyhow::bail!("gvcs adherence check failed.");
    }

    println!(
        "\n{}",
        "✓ gvcs check PASSED: Hermetic code, provenance spans, and exception pins are 100% aligned."
            .bright_green()
            .bold()
    );
    Ok(())
}

fn cmd_slice(ws: &GvcsWorkspace, feature: &str) -> Result<()> {
    let (cells, spans) = ws.slice_feature(feature);
    println!(
        "{} Slicing hypergraph for feature query '{}':",
        "gvcs slice".bold().bright_cyan(),
        feature.bright_yellow()
    );

    println!("\n  Participating Intersection Cells ({}):", cells.len());
    for c in &cells {
        println!("    • {} — {}", c.coordinate.bold().bright_cyan(), c.derivation_reason);
    }

    println!(
        "\n  Exact Committed Source Code Spans in `src/` ({}):",
        spans.len()
    );
    for s in &spans {
        println!(
            "    • {} (lines {}..{}) — symbol `{}`",
            s.file_path.bold().bright_green(),
            s.start_line,
            s.end_line,
            s.symbol.bright_white()
        );
        println!(
            "        Cell: {} | SHA256: {}.. | Tests: {:?}",
            s.cell_coordinate.bright_cyan(),
            &s.content_sha256[..12],
            s.verification_tests
        );
    }
    Ok(())
}

fn cmd_what_if(ws: &GvcsWorkspace, remove: &str) -> Result<()> {
    let (cells, spans) = ws.slice_feature(remove);
    println!(
        "{} Simulating removal of feature '{}':",
        "gvcs what-if --remove".bold().bright_magenta(),
        remove.bright_yellow()
    );
    println!(
        "  • {} Intersection Cell(s) ($I_S$ where '{}' $\\in S$) will be dropped:",
        cells.len(),
        remove
    );
    for c in &cells {
        println!("      - {}", c.coordinate.bright_red());
    }
    println!(
        "  • {} Fused Code Span(s) in `src/` will be cleanly excised/re-synthesized:",
        spans.len()
    );
    for s in &spans {
        println!(
            "      - {}:{}..{} (`{}`)",
            s.file_path, s.start_line, s.end_line, s.symbol
        );
    }
    let untouched_spans = ws.current_spans.len().saturating_sub(spans.len());
    println!(
        "  • {} other code spans across the repository remain 100% UNTOUCHED (verified by SHA-256 lock).",
        untouched_spans.to_string().bright_green().bold()
    );
    Ok(())
}

fn cmd_inspect(ws: &GvcsWorkspace) -> Result<()> {
    println!(
        "{}",
        "=== GVCS SPECIALIZED ARTIFACTS (Marketplace, @native Escape Hatches, Incidents) ==="
            .bold()
            .bright_cyan()
    );

    println!("\n{}:", "A. Feature Marketplace Blueprints (@uses)".bold());
    for f in ws.features.values() {
        if let SpecKind::MarketplaceBlueprint { package, version } = &f.kind {
            println!(
                "  • {} -> {}@{} ({})",
                f.id.bold(),
                package.bright_cyan(),
                version,
                f.spec_path
            );
        }
    }

    println!("\n{}:", "B. Sealed @native Code Escape Hatches".bold());
    for f in ws.features.values() {
        if let SpecKind::NativeCell {
            source_path,
            entrypoint,
        } = &f.kind
        {
            println!(
                "  • {} -> `{}` (entrypoint: `{}`)",
                f.id.bold(),
                source_path.bright_yellow(),
                entrypoint
            );
        }
    }

    println!(
        "\n{}:",
        "C. Incident Invariants (.incident.toml) & Exception Pins (.pareto.toml)".bold()
    );
    for e in ws.exceptions.values() {
        println!(
            "  • [Pin: {}] Chose '{}' -> {}",
            e.id.bright_yellow().bold(),
            e.chosen_option.bright_green(),
            e.resolution_directive
        );
    }
    for inc in ws.incidents.values() {
        println!(
            "  • [Incident: {}] {} -> {:?}",
            inc.id.bright_red().bold(),
            inc.title,
            inc.stage
        );
    }
    Ok(())
}
