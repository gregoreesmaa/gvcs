use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use todo_app::web_ui::{dispatch_http_request, serve_web_ui};
use todo_app::TodoStore;

#[derive(Parser)]
#[command(
    name = "todo-app",
    version = "0.2.0",
    about = "Todo App with Interactive Web UI Projection (`cap:web-ui`), built with gvcs Intersection Cells"
)]
struct Cli {
    /// Path to the persistent JSON database file (`DBTable:todos`).
    #[arg(long, default_value = ".todo-db.json", global = true)]
    db: PathBuf,

    /// Host and port to bind the Web UI server (`cap:web-ui`).
    #[arg(long, default_value = "127.0.0.1:8080", global = true)]
    addr: String,

    /// Enable or disable the `flag_recurring_tasks` canary flag (`@lifecycle: CANARY(25%)`).
    #[arg(long, default_value_t = true, global = true)]
    canary_recurring: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the interactive Single-Page Web UI & REST API server (default: http://127.0.0.1:8080).
    Serve,
    /// Run a self-contained Web UI & REST API walkthrough in-process (verifying all endpoints in <1s).
    Demo,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Demo) {
        Commands::Serve => {
            let store = TodoStore::load_or_init(&cli.db, cli.canary_recurring)
                .map_err(|e| anyhow::anyhow!(e))?;
            let shared = Arc::new(Mutex::new(store));
            serve_web_ui(shared, &cli.addr)?;
        }
        Commands::Demo => run_web_ui_demo(cli.canary_recurring)?,
    }

    Ok(())
}

fn run_web_ui_demo(canary_recurring: bool) -> Result<()> {
    println!("=== gvcs Web UI Todo App Showcase (After Removing Google Login & CLI, Adding cap:web-ui) ===");
    println!("  (Run `cargo run -p todo-app -- serve` to launch the live browser UI at http://127.0.0.1:8080)\n");

    let mut store = TodoStore::new(canary_recurring);
    store.seed_default_demo_tasks();

    let (status, content_type, html) = dispatch_http_request(&mut store, "GET", "/", "");
    println!(
        "1. GET / -> HTTP {} ({}, {} bytes of Single-Page Web UI HTML)",
        status,
        content_type,
        html.len()
    );

    let (status, _, list_json) = dispatch_http_request(&mut store, "GET", "/api/todos", "");
    println!(
        "2. GET /api/todos (Sorted by @native Urgency Decay Kernel) -> HTTP {}:\n   {}",
        status, list_json
    );

    let (status, _, filtered_json) =
        dispatch_http_request(&mut store, "GET", "/api/todos?tag=work", "");
    println!(
        "\n3. GET /api/todos?tag=work (`cap:fast-cache-sla` Cache Hit = true) -> HTTP {}:\n   {}",
        status, filtered_json
    );

    let (status, _, complete_json) =
        dispatch_http_request(&mut store, "POST", "/api/todos/2/complete", "");
    println!(
        "\n4. POST /api/todos/2/complete (Recurring task #2 spawns next cycle #5 with inherited #ops #rust tags) -> HTTP {}:\n   {}",
        status, complete_json
    );

    let (status, _, gdpr_json) =
        dispatch_http_request(&mut store, "POST", "/api/gdpr-erase", "");
    println!(
        "\n5. POST /api/gdpr-erase (`PARETO-001` Write-Through Cache Purge + Portable Archive) -> HTTP {}:\n   {}",
        status, gdpr_json
    );

    Ok(())
}
