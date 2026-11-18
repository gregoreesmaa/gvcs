use todo_app::{AuthSession, TodoStore};

fn main() {
    println!("--- Running Todo App (Materialized from gvcs Feature Hypergraph) ---\n");

    let mut store = TodoStore::new(true);
    let alice = AuthSession::verified("alice@example.com");

    store
        .create_todo(&alice, "Review quarterly roadmap", 2, -24, None)
        .expect("Failed to create todo");
    let recurring = store
        .create_todo(
            &alice,
            "Run weekly gvcs adherence sweep",
            5,
            6,
            Some(7),
        )
        .expect("Failed to create recurring todo");
    store
        .create_todo(&alice, "Fix production Stripe webhook", 5, 18, None)
        .expect("Failed to create urgent todo");

    println!("1. Initial Board (First Query -> Populates Sub-25ms Cache):");
    println!("{}", store.render_viewport_board(&alice).unwrap());

    println!("2. Completing Recurring Task #{} ('{}'):", recurring.id, recurring.title);
    let (_, spawned) = store.complete_todo(&alice, recurring.id).unwrap();
    if let Some(next) = spawned {
        println!(
            "   -> Spawned next recurring instance #{} (due in 7 days)\n",
            next.id
        );
    }

    println!("3. Updated Board after Completion:");
    println!("{}", store.render_viewport_board(&alice).unwrap());

    println!("4. Triggering 1-Click GDPR Export & Erasure (PARETO-001 Write-Through Cache Purge):");
    let (export_json, wiped) = store.gdpr_erase_and_export(&alice).unwrap();
    println!("   -> Wiped {} rows + evicted memory cache. Exported JSON preview ({} bytes).", wiped, export_json.len());
    println!("\n5. Board After GDPR Erasure (0ms Stale PII):");
    println!("{}", store.render_viewport_board(&alice).unwrap());
}
