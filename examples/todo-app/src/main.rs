use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;
use todo_app::{AuthSession, TodoFilter, TodoStore};

#[derive(Parser)]
#[command(
    name = "todo-app",
    version = "0.1.0",
    about = "Realistic CLI Todo Application built with Specification-as-Source & gvcs Intersection Cells"
)]
struct Cli {
    /// Authenticated Google OAuth user email (`mkt:google-login`).
    #[arg(short, long, default_value = "alice@example.com", global = true)]
    user: String,

    /// Path to the persistent JSON database file (`DBTable:todos`).
    #[arg(long, default_value = ".todo-db.json", global = true)]
    db: PathBuf,

    /// Enable or disable the `flag_recurring_tasks` canary flag (`@lifecycle: CANARY(25%)`).
    #[arg(long, default_value_t = true, global = true)]
    canary_recurring: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new Todo item (supports inline `#tags`, priority 1..5, due offset, and recurrence).
    Add {
        /// Task title (include inline tags like `#work` or `#rust`).
        title: String,
        /// Priority weight from 1 (lowest) to 5 (critical).
        #[arg(short, long, default_value_t = 3)]
        priority: u8,
        /// Hours overdue (positive = overdue, negative = due in future hours).
        #[arg(short, long, default_value_t = 0)]
        overdue_hours: i64,
        /// Optional recurrence interval in days (`feat:recurring-tasks`).
        #[arg(short, long)]
        recur_days: Option<u32>,
        /// Additional explicit project tags (`feat:tags-and-search`).
        #[arg(short, long)]
        tag: Vec<String>,
    },
    /// List and filter tasks on `Viewport:TodoList`, sorted by the `@native` Urgency Score kernel.
    List {
        /// Filter by `#tag` (e.g., `rust` or `#work`).
        #[arg(short, long)]
        tag: Option<String>,
        /// Filter by search keyword in title or tags.
        #[arg(short, long)]
        search: Option<String>,
        /// Show only pending tasks.
        #[arg(long, default_value_t = false)]
        pending_only: bool,
    },
    /// Complete a task by ID (atomically spawns next cycle if recurring, guarded by `INC-001` idempotency).
    Done {
        /// Task ID to mark completed.
        id: u64,
    },
    /// Delete a single task by ID.
    Delete {
        /// Task ID to delete.
        id: u64,
    },
    /// 1-Click GDPR Account Export & Erasure (`cap:gdpr-privacy` ∩ `cap:fast-cache-sla`).
    GdprErase {
        /// Explicit confirmation guard required by `cap:cli-ux-shell` for destructive actions.
        #[arg(long, default_value_t = false)]
        confirm: bool,
    },
    /// Run a complete interactive walkthrough demonstrating all 8 intersecting features & capabilities.
    Demo,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let session = AuthSession::verified(&cli.user);

    match cli.command.unwrap_or(Commands::Demo) {
        Commands::Add {
            title,
            priority,
            overdue_hours,
            recur_days,
            tag,
        } => {
            let mut store =
                TodoStore::load_or_init(&cli.db, cli.canary_recurring).map_err(|e| anyhow::anyhow!(e))?;
            let item = store
                .create_todo_with_tags(&session, &title, priority, overdue_hours, recur_days, &tag)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!(
                "{} Created task #{} '{}' (P{}, tags: {:?})",
                "✓".bright_green().bold(),
                item.id,
                item.title.bold(),
                item.priority_weight,
                item.tags
            );
            print!("{}", store.render_viewport_board(&session).map_err(|e| anyhow::anyhow!(e))?);
        }
        Commands::List {
            tag,
            search,
            pending_only,
        } => {
            let mut store =
                TodoStore::load_or_init(&cli.db, cli.canary_recurring).map_err(|e| anyhow::anyhow!(e))?;
            let filter = TodoFilter {
                tag,
                search_query: search,
                only_pending: pending_only,
            };
            print!(
                "{}",
                store
                    .render_viewport_board_filtered(&session, &filter)
                    .map_err(|e| anyhow::anyhow!(e))?
            );
        }
        Commands::Done { id } => {
            let mut store =
                TodoStore::load_or_init(&cli.db, cli.canary_recurring).map_err(|e| anyhow::anyhow!(e))?;
            let (done, spawned) = store
                .complete_todo(&session, id)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!(
                "{} Marked task #{} ('{}') as Completed.",
                "✓".bright_green().bold(),
                done.id,
                done.title.bold()
            );
            if let Some(next) = spawned {
                println!(
                    "  {} Spawned next recurring instance #{} (due in {} days, inherited tags: {:?})",
                    "↻".bright_magenta().bold(),
                    next.id,
                    next.recurrence_days.unwrap_or(0),
                    next.tags
                );
            }
            print!("{}", store.render_viewport_board(&session).map_err(|e| anyhow::anyhow!(e))?);
        }
        Commands::Delete { id } => {
            let mut store =
                TodoStore::load_or_init(&cli.db, cli.canary_recurring).map_err(|e| anyhow::anyhow!(e))?;
            let deleted = store
                .delete_todo(&session, id)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!(
                "{} Deleted task #{} ('{}').",
                "✓".bright_yellow().bold(),
                deleted.id,
                deleted.title
            );
            print!("{}", store.render_viewport_board(&session).map_err(|e| anyhow::anyhow!(e))?);
        }
        Commands::GdprErase { confirm } => {
            if !confirm {
                bail!(
                    "Destructive action blocked by `cap:cli-ux-shell` overflow guard! Pass `--confirm` to execute 1-click GDPR Export & Erasure."
                );
            }
            let mut store =
                TodoStore::load_or_init(&cli.db, cli.canary_recurring).map_err(|e| anyhow::anyhow!(e))?;
            let (export_json, wiped) = store
                .gdpr_erase_and_export(&session)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!(
                "{} GDPR Erasure complete for '{}': wiped {} rows from DBTable:todos & synchronously purged read cache (PARETO-001).",
                "✓".bright_green().bold(),
                session.user_email,
                wiped
            );
            println!("--- Exported Portability Archive (JSON) ---\n{}", export_json);
        }
        Commands::Demo => run_five_minute_showcase(&session, cli.canary_recurring)?,
    }

    Ok(())
}

fn run_five_minute_showcase(session: &AuthSession, canary_recurring: bool) -> Result<()> {
    println!(
        "{}",
        "=== 5-Minute Todo App Showcase (Built from 8 gvcs Feature & Capability Specs) ==="
            .bold()
            .bright_cyan()
    );
    let mut store = TodoStore::new(canary_recurring);

    store
        .create_todo_with_tags(
            session,
            "Prepare Q4 product roadmap #strategy",
            2,
            -24,
            None,
            &["work".to_string()],
        )
        .unwrap();
    let recurring = store
        .create_todo_with_tags(
            session,
            "Run weekly gvcs adherence sweep #ops",
            5,
            6,
            Some(7),
            &["rust".to_string()],
        )
        .unwrap();
    store
        .create_todo_with_tags(
            session,
            "Fix production Stripe webhook timeout #urgent",
            5,
            18,
            None,
            &["work".to_string(), "backend".to_string()],
        )
        .unwrap();
    store
        .create_todo(session, "Pick up espresso beans #home", 1, -8, None)
        .unwrap();

    println!("\n1. Full Board (Sorted by @native Urgency Decay Kernel):");
    print!("{}", store.render_viewport_board(session).unwrap());

    println!("\n2. Filter by Project Tag `#work` (`feat:tags-and-search` ∩ `cap:fast-cache-sla` -> Cache Hit = true):");
    print!(
        "{}",
        store
            .render_viewport_board_filtered(
                session,
                &TodoFilter {
                    tag: Some("#work".to_string()),
                    search_query: None,
                    only_pending: false,
                }
            )
            .unwrap()
    );

    println!(
        "\n3. Complete Recurring Task #{} ('{}') -> Triggers `EXC-002` & `EXC-004` (Inherits #ops #rust):",
        recurring.id, recurring.title
    );
    let (_, spawned) = store.complete_todo(session, recurring.id).unwrap();
    if let Some(next) = spawned {
        println!(
            "   -> Spawned next cycle task #{} with inherited tags {:?}",
            next.id, next.tags
        );
    }
    print!("{}", store.render_viewport_board(session).unwrap());

    println!("\n4. 1-Click GDPR Export & Erasure (`PARETO-001` Write-Through Cache Purge):");
    let (export_json, wiped) = store.gdpr_erase_and_export(session).unwrap();
    println!(
        "   -> Wiped {} rows + evicted read cache (0ms stale PII). Exported {} bytes of portable JSON.",
        wiped,
        export_json.len()
    );
    print!("{}", store.render_viewport_board(session).unwrap());

    Ok(())
}
