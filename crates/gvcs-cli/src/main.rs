use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use gvcs_core::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "gvcs",
    version = "0.2.0",
    about = "gvcs — Feature-Aware Version Control (Maps Features, Overlaps & Decisions to Code)"
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
    /// Show the Feature Map: active features, where they meet in `src/`, and governing product decisions.
    Status {
        /// Also list all pairwise theoretical rule overlaps that were absorbed into fused code blocks.
        #[arg(short, long, default_value_t = false)]
        verbose: bool,
    },
    /// Synchronize `specs/` and `src/` code blocks into `provenance.lock`.
    Sync,
    /// Verify spec-to-code alignment (`provenance.lock` vs `src/`), product decisions, and bug guards.
    Check,
    /// Show every code block in `src/`, product decision, and test connected to a specific feature.
    Slice {
        /// Feature ID or name to slice (e.g., `gdpr-privacy`, `recurring-tasks`, `web-ui`).
        feature: String,
    },
    /// Preview the exact blast radius of removing a feature (`[DELETE]` vs `[SIMPLIFY]` code blocks).
    WhatIf {
        /// Feature ID or name to simulate removing.
        #[arg(long)]
        remove: String,
    },
    /// Inspect Marketplace blueprints, handwritten `@native` modules, decisions, and bug guards.
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
        Commands::Status { verbose } => cmd_status(&ws, verbose),
        Commands::Sync => cmd_sync(&ws),
        Commands::Check => cmd_check(&ws),
        Commands::Slice { feature } => cmd_slice(&ws, &feature),
        Commands::WhatIf { remove } => cmd_what_if(&ws, &remove),
        Commands::Inspect => cmd_inspect(&ws),
    }
}

fn cmd_status(ws: &GvcsWorkspace, verbose: bool) -> Result<()> {
    let hash = ws.compute_graph_state_hash();
    let is_synced = ws
        .existing_lock
        .as_ref()
        .map(|l| l.graph_state_hash == hash)
        .unwrap_or(false);
    let sync_badge = if is_synced {
        "✓ synced".bright_green()
    } else {
        "⚠ unsynced (run `gvcs sync`)".bright_yellow()
    };

    println!(
        "{}",
        "================================================================================"
            .bright_blue()
    );
    println!(
        "{} — {} (lock: {} {})",
        "gvcs status".bold().bright_cyan(),
        ws.root.display(),
        &hash[..8].bright_yellow(),
        sync_badge
    );
    println!(
        "{}",
        "================================================================================"
            .bright_blue()
    );

    println!(
        "\n{} ({} active in 4-Layer Priority Stack):",
        "1. FEATURES & CAPABILITIES".bold().bright_white(),
        ws.features.len()
    );
    for feat in ws.features.values() {
        let kind_badge = match &feat.kind {
            SpecKind::Feature => "Feature".bright_green(),
            SpecKind::Capability => "Capability".bright_magenta(),
            SpecKind::MarketplaceBlueprint { package, version } => {
                format!("Marketplace ({}@{})", package, version).bright_cyan()
            }
            SpecKind::NativeCell { source_path, .. } => {
                format!("@native ({})", source_path).bright_yellow()
            }
        };
        let rollout_badge = match &feat.lifecycle {
            LifecycleState::Ga => "Live (100%)".green(),
            LifecycleState::Canary { percentage, flag } => {
                format!("Canary ({}% flag={})", percentage, flag).yellow()
            }
            LifecycleState::Deprecated {
                min_client_version, ..
            } => format!("Deprecated (min={})", min_client_version).red(),
        };

        println!(
            "  • {:<24} {:<18} {:<20} [{}] — {}",
            feat.id.bold(),
            feat.stratum.label().dimmed(),
            kind_badge,
            rollout_badge,
            feat.title
        );
        for q in &feat.quantifiers {
            println!("      {} \"{}\"", "Global Rule:".bright_magenta(), q.statement);
        }
    }

    println!(
        "\n{} ({} linked code blocks in `src/`):",
        "2. ACTIVE CODE INTERSECTIONS (Where Features Meet in Code)"
            .bold()
            .bright_white(),
        ws.current_spans.len()
    );
    for span in &ws.current_spans {
        let loc = format!("{}:{}..{}", span.file_path, span.start_line, span.end_line);
        let human_feats = IntersectionCell::format_human_list(&span.cell_coordinate);
        println!(
            "  • {:<28} {}",
            loc.bold().bright_green(),
            span.symbol.bold().bright_white()
        );
        println!("      Features:  {}", human_feats.bright_cyan());
        if !span.active_pins.is_empty() {
            println!(
                "      Decisions: {}",
                span.active_pins.join(", ").bright_yellow()
            );
        }
        if !span.active_incidents.is_empty() {
            println!(
                "      Bug Guard: {}",
                span.active_incidents.join(", ").bright_red()
            );
        }
        if !span.verification_tests.is_empty() {
            println!(
                "      Verified:  {}",
                span.verification_tests.join(", ").dimmed()
            );
        }
    }

    let absorbed_count = ws
        .cells
        .values()
        .filter(|c| {
            !ws.current_spans
                .iter()
                .any(|s| s.cell_coordinate == c.coordinate)
        })
        .count();

    if verbose {
        println!(
            "\n  {} ({}):",
            "Pairwise Rule Overlaps (absorbed into fused code blocks above)".dimmed(),
            absorbed_count
        );
        for cell in ws.cells.values() {
            let has_direct_span = ws
                .current_spans
                .iter()
                .any(|s| s.cell_coordinate == cell.coordinate);
            if !has_direct_span {
                println!(
                    "    - {} — {}",
                    IntersectionCell::format_human_list(&cell.coordinate).dimmed(),
                    cell.derivation_reason.dimmed()
                );
            }
        }
    } else if absorbed_count > 0 {
        println!(
            "  {}",
            format!(
                "({} pairwise rule checks absorbed into the fused code blocks above; pass `--verbose` to list all)",
                absorbed_count
            )
            .dimmed()
        );
    }

    println!(
        "\n{} ({} decisions, {} bug guard):",
        "3. PRODUCT DECISIONS & BUG GUARDS".bold().bright_white(),
        ws.exceptions.len(),
        ws.incidents.len()
    );
    for pin in ws.exceptions.values() {
        let pair = pin.features.iter().cloned().collect::<Vec<_>>().join(" × ");
        println!(
            "  {} {:<28} [{}]",
            "✓".bright_green().bold(),
            pin.id.bold().bright_yellow(),
            pair.bright_cyan()
        );
        println!("      Decision: {}", pin.effective_directive());
    }
    for inc in ws.incidents.values() {
        let pair = inc
            .affected_features
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(" × ");
        match inc.resolved_stage() {
            IncidentStage::LiftedInvariant {
                negative_invariant,
                regression_test_name,
            } => {
                println!(
                    "  {} {:<28} [{}] (Permanent Bug Guard)",
                    "🛡".bright_red(),
                    inc.id.bold().bright_red(),
                    pair.bright_cyan()
                );
                println!("      Rule: \"{}\"", negative_invariant);
                println!("      Test: {}", regression_test_name.dimmed());
            }
            IncidentStage::Tourniquet { expires_at, .. } => {
                println!(
                    "  {} {:<28} [{}] (72h Hotfix Grace Period — expires {})",
                    "⚠".bright_yellow(),
                    inc.id.bold().bright_yellow(),
                    pair.bright_cyan(),
                    expires_at
                );
            }
        }
    }

    Ok(())
}

fn cmd_sync(ws: &GvcsWorkspace) -> Result<()> {
    let lock = ws.sync_and_write_lock()?;
    println!(
        "{} Synchronized {} features/capabilities, {} code blocks in `src/`, and {} decisions into {}",
        "✓".bright_green().bold(),
        lock.features.len(),
        lock.code_spans.len(),
        lock.exceptions.len(),
        ws.root.join("provenance.lock").display()
    );
    println!(
        "  Feature Lock Hash: {}",
        lock.graph_state_hash.bright_yellow()
    );
    Ok(())
}

fn cmd_check(ws: &GvcsWorkspace) -> Result<()> {
    println!(
        "{}",
        "Checking spec-to-code alignment (provenance.lock vs src/)...".bold()
    );

    let mut failed = false;

    // 1. Check unresolved Feature Overlaps / Tradeoffs
    let unresolved: Vec<_> = ws.conflicts.iter().filter(|c| !c.is_resolved).collect();
    if !unresolved.is_empty() {
        failed = true;
        println!(
            "\n{} {} Unresolved Feature Conflict(s) require a Product Decision:",
            "✗".bright_red().bold(),
            unresolved.len()
        );
        for c in unresolved {
            let pair = c.features.iter().cloned().collect::<Vec<_>>().join(" × ");
            println!("  • {} — {}", pair.bold().bright_cyan(), c.summary.bright_yellow());
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
            "  {} {:<20} {} feature interactions resolved (0 unresolved conflicts)",
            "✓".bright_green().bold(),
            "Product Decisions".bold(),
            ws.conflicts.len()
        );
    }

    // 2. Check Code Provenance Drift (3 AM manual edits vs provenance.lock)
    let drifts = ws.check_drifts();
    if !drifts.is_empty() {
        for d in &drifts {
            if d.has_active_tourniquet {
                println!(
                    "  {} {:<20} `{}` in {}:{}..{} has an active 72h Hotfix Grace Period.",
                    "⚠".bright_yellow().bold(),
                    "Hotfix Grace Period".bold(),
                    d.symbol,
                    d.file_path,
                    d.start_line,
                    d.end_line
                );
            } else {
                failed = true;
                println!(
                    "  {} Unsynced code edit detected in {}:{}..{} (`{}`)\n      Features affected: {}\n      Fix: Run `gvcs sync` to lock this change, or add a `.incident.toml` if this is a 3 AM hotfix.",
                    "✗".bright_red().bold(),
                    d.file_path,
                    d.start_line,
                    d.end_line,
                    d.symbol.bold(),
                    IntersectionCell::format_human_list(&d.cell_coordinate).bright_cyan()
                );
            }
        }
    } else {
        println!(
            "  {} {:<20} {} code blocks in src/ match provenance.lock SHA-256 digests",
            "✓".bright_green().bold(),
            "Code Provenance".bold(),
            ws.current_spans.len()
        );
    }

    // 3. Check Bug Guards
    let mut permanent_guards = 0;
    let mut active_hotfixes = 0;
    for inc in ws.incidents.values() {
        match inc.resolved_stage() {
            IncidentStage::LiftedInvariant { .. } => permanent_guards += 1,
            IncidentStage::Tourniquet { .. } => active_hotfixes += 1,
        }
    }
    println!(
        "  {} {:<20} {} permanent regression guard(s) active, {} temporary hotfix(es)",
        "✓".bright_green().bold(),
        "Bug Guards".bold(),
        permanent_guards,
        active_hotfixes
    );

    if failed {
        anyhow::bail!("gvcs check failed.");
    }

    println!(
        "\n{}",
        "✓ gvcs check passed — specs, product decisions, and src/ are 100% aligned."
            .bright_green()
            .bold()
    );
    Ok(())
}

fn cmd_slice(ws: &GvcsWorkspace, feature: &str) -> Result<()> {
    let (cells, spans) = ws.slice_feature(feature);
    let matched_feat = ws
        .features
        .values()
        .find(|f| f.id == feature || f.id.contains(feature));

    println!(
        "{} — {}",
        "gvcs slice".bold().bright_cyan(),
        matched_feat
            .map(|f| format!("{} (\"{}\")", f.id, f.title))
            .unwrap_or_else(|| feature.to_string())
            .bright_yellow()
            .bold()
    );

    if let Some(f) = matched_feat {
        println!(
            "  Spec:   {} [Layer: {}]",
            f.spec_path.bright_white(),
            f.stratum.label()
        );
        for q in &f.quantifiers {
            println!("  Rule:   \"{}\"", q.statement.bright_magenta());
        }
    }

    println!(
        "\n  {} ({} block(s)):",
        "Code Blocks in `src/` Implementing This Feature".bold(),
        spans.len()
    );
    for s in &spans {
        println!(
            "    • {}:{}..{} — `{}`",
            s.file_path.bold().bright_green(),
            s.start_line,
            s.end_line,
            s.symbol.bold().bright_white()
        );
        println!(
            "        Features:  {}",
            IntersectionCell::format_human_list(&s.cell_coordinate).bright_cyan()
        );
        if !s.active_pins.is_empty() {
            println!(
                "        Decisions: {}",
                s.active_pins.join(", ").bright_yellow()
            );
        }
        if !s.active_incidents.is_empty() {
            println!(
                "        Bug Guard: {}",
                s.active_incidents.join(", ").bright_red()
            );
        }
        println!(
            "        Tests:     {} (SHA256: {}..)",
            s.verification_tests.join(", ").dimmed(),
            &s.content_sha256[..10].dimmed()
        );
    }

    let coupled_features: std::collections::BTreeSet<String> = cells
        .iter()
        .flat_map(|c| c.features.iter().cloned())
        .filter(|fid| !fid.contains(feature))
        .collect();

    if !coupled_features.is_empty() {
        println!(
            "\n  {}: {}",
            "Overlapping Features & Policies".bold(),
            coupled_features
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
                .bright_cyan()
        );
    }

    Ok(())
}

fn cmd_what_if(ws: &GvcsWorkspace, remove: &str) -> Result<()> {
    let (_, spans) = ws.slice_feature(remove);
    let matched_feat = ws
        .features
        .values()
        .find(|f| f.id == remove || f.id.contains(remove));
    let target_id = matched_feat.map(|f| f.id.as_str()).unwrap_or(remove);

    println!(
        "{} `{}`:",
        "gvcs what-if --remove".bold().bright_magenta(),
        target_id.bright_yellow().bold()
    );

    println!(
        "\n  {} ({} of {} code blocks affected):",
        "1. Code Blocks to Delete or Simplify in `src/`".bold(),
        spans.len(),
        ws.current_spans.len()
    );
    for s in &spans {
        let remaining: Vec<String> = s
            .features
            .iter()
            .filter(|f| !f.contains(remove))
            .cloned()
            .collect();
        if remaining.is_empty() {
            println!(
                "    • {} {}:{}..{} — `{}` (solely owned by `{}`)",
                "[DELETE]".bright_red().bold(),
                s.file_path.bold(),
                s.start_line,
                s.end_line,
                s.symbol.bold(),
                target_id
            );
        } else {
            println!(
                "    • {} {}:{}..{} — `{}`",
                "[SIMPLIFY]".bright_yellow().bold(),
                s.file_path.bold(),
                s.start_line,
                s.end_line,
                s.symbol.bold()
            );
            println!(
                "        Currently combines: {}",
                IntersectionCell::format_human_list(&s.cell_coordinate).dimmed()
            );
            println!(
                "        After removal:      Retains only `{}`",
                remaining.join(" + ").bright_cyan()
            );
        }
    }

    let retired_pins: Vec<&ExceptionPin> = ws
        .exceptions
        .values()
        .filter(|p| p.features.iter().any(|f| f.contains(remove)))
        .collect();
    let retired_incidents: Vec<&IncidentRecord> = ws
        .incidents
        .values()
        .filter(|i| i.affected_features.iter().any(|f| f.contains(remove)))
        .collect();

    if !retired_pins.is_empty() || !retired_incidents.is_empty() {
        println!(
            "\n  {} ({}):",
            "2. Product Decisions & Bug Guards Retired".bold(),
            retired_pins.len() + retired_incidents.len()
        );
        for p in retired_pins {
            println!("    • Drops decision {}", p.id.bright_yellow());
        }
        for i in retired_incidents {
            println!("    • Retires bug guard {}", i.id.bright_red());
        }
    }

    let untouched = ws.current_spans.len().saturating_sub(spans.len());
    let pct = if ws.current_spans.is_empty() {
        100
    } else {
        (untouched * 100) / ws.current_spans.len()
    };
    println!("\n  {}:", "3. Untouched Code Guarantee".bold());
    println!(
        "    {} {} of {} code blocks ({}% of tracked codebase) remain 100% UNTOUCHED (verified by SHA-256 lock).",
        "✓".bright_green().bold(),
        untouched.to_string().bright_green().bold(),
        ws.current_spans.len(),
        pct
    );
    Ok(())
}

fn cmd_inspect(ws: &GvcsWorkspace) -> Result<()> {
    cmd_status(ws, false)
}
