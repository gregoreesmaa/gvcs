//! Reference Todo Application — Committed Fused Code with `gvcs` Intersection Provenance.
//!
//! Every span in this file is bi-directionally anchored in `provenance.lock` to its
//! exact Intersection Cell coordinate ($I_{A \cap B \cap \dots}$), Pareto Tradeoff Pin ($\mathcal{E}$),
//! and Incident Invariant (`.incident`).

pub mod native_priority;

use native_priority::compute_urgency_score;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// @gvcs-begin cell="I(mkt:google-login)" symbol="AuthSession" tests="test_google_oauth_rejects_unauthenticated_mutations"
/// Verified Google OAuth identity token (`@marketplace/google-oauth-identity@2.4.0`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthSession {
    pub user_email: String,
    pub google_sub_id: String,
    pub is_verified: bool,
}

impl AuthSession {
    pub fn verified(email: &str) -> Self {
        Self {
            user_email: email.to_string(),
            google_sub_id: format!("google-oauth2|{}", email),
            is_verified: true,
        }
    }

    pub fn require_authenticated(&self) -> Result<&str, &'static str> {
        if !self.is_verified || self.user_email.trim().is_empty() {
            Err("Unauthorized: valid Google OAuth token required")
        } else {
            Ok(&self.user_email)
        }
    }
}
// @gvcs-end

// @gvcs-begin cell="I(feat:todo-crud)" symbol="TodoItem" tests="test_create_and_complete_todo"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoStatus {
    Pending,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: u64,
    pub owner_email: String,
    pub title: String,
    pub status: TodoStatus,
    pub priority_weight: u8,
    pub hours_overdue: i64,
    /// Optional recurrence interval in days (`feat:recurring-tasks`).
    pub recurrence_days: Option<u32>,
}
// @gvcs-end

// @gvcs-begin cell="I(feat:todo-crud ∩ mkt:google-login)" symbol="TodoStore::create_todo" tests="test_create_and_complete_todo,test_google_oauth_rejects_unauthenticated_mutations"
/// In-memory database substrate (`DBTable:todos`) + write-through read cache (`cap:fast-cache-sla`).
#[derive(Debug, Default)]
pub struct TodoStore {
    next_id: u64,
    rows: BTreeMap<u64, TodoItem>,
    /// Write-through per-user read cache (`cap:fast-cache-sla`).
    read_cache: BTreeMap<String, Vec<TodoItem>>,
    /// Runtime canary flag for `feat:recurring-tasks` (`@lifecycle: CANARY(25%, flag_recurring_tasks)`).
    pub flag_recurring_tasks: bool,
}

impl TodoStore {
    pub fn new(flag_recurring_tasks: bool) -> Self {
        Self {
            next_id: 1,
            rows: BTreeMap::new(),
            read_cache: BTreeMap::new(),
            flag_recurring_tasks,
        }
    }

    /// Creates a new Todo item bound to the caller's verified Google OAuth identity.
    pub fn create_todo(
        &mut self,
        session: &AuthSession,
        title: &str,
        priority_weight: u8,
        hours_overdue: i64,
        recurrence_days: Option<u32>,
    ) -> Result<TodoItem, &'static str> {
        let owner = session.require_authenticated()?;
        let id = self.next_id;
        self.next_id += 1;

        let item = TodoItem {
            id,
            owner_email: owner.to_string(),
            title: title.to_string(),
            status: TodoStatus::Pending,
            priority_weight,
            hours_overdue,
            recurrence_days,
        };

        self.rows.insert(id, item.clone());
        // Invalidate write-through read cache for this owner on mutation
        self.read_cache.remove(owner);
        Ok(item)
    }
// @gvcs-end

// @gvcs-begin cell="I(feat:recurring-tasks ∩ feat:todo-crud)" symbol="TodoStore::complete_todo" tests="test_recurring_task_spawns_next_cycle,test_inc_001_idempotent_complete_never_spawns_duplicate_recurrence"
    /// Fused completion & recurring roll-forward mutation.
    /// Governed by:
    /// - Exception Pin `EXC-002-crud-vs-recurring` (atomic complete & spawn)
    /// - Incident Invariant `INC-001-double-complete-spawn` (idempotent guard on already-Completed tasks)
    /// - Lifecycle Canary Gate `flag_recurring_tasks`
    pub fn complete_todo(
        &mut self,
        session: &AuthSession,
        todo_id: u64,
    ) -> Result<(TodoItem, Option<TodoItem>), &'static str> {
        let caller = session.require_authenticated()?;
        let existing = self
            .rows
            .get_mut(&todo_id)
            .ok_or("NotFound: todo item does not exist")?;

        if existing.owner_email != caller {
            return Err("Forbidden: cannot complete another user's todo");
        }

        // [INC-001 Negative Invariant Guard]: Completing an already-Completed task is an idempotent
        // no-op and must NEVER spawn duplicate recurring occurrences.
        if existing.status == TodoStatus::Completed {
            return Ok((existing.clone(), None));
        }

        existing.status = TodoStatus::Completed;
        let completed_snapshot = existing.clone();
        let recurrence = existing.recurrence_days;

        let spawned = if self.flag_recurring_tasks {
            if let Some(days) = recurrence {
                let next_id = self.next_id;
                self.next_id += 1;
                let next_item = TodoItem {
                    id: next_id,
                    owner_email: caller.to_string(),
                    title: completed_snapshot.title.clone(),
                    status: TodoStatus::Pending,
                    priority_weight: completed_snapshot.priority_weight,
                    hours_overdue: -(days as i64 * 24),
                    recurrence_days: Some(days),
                };
                self.rows.insert(next_id, next_item.clone());
                Some(next_item)
            } else {
                None
            }
        } else {
            None
        };

        self.read_cache.remove(caller);
        Ok((completed_snapshot, spawned))
    }
// @gvcs-end

// @gvcs-begin cell="I(cap:fast-cache-sla ∩ feat:todo-crud ∩ native:priority-decay)" symbol="TodoStore::list_todos_sorted" tests="test_cached_list_sorted_by_native_urgency"
    /// Joint Multi-Cell Fused Query:
    /// Combines `feat:todo-crud` retrieval, `native:priority-decay` urgency scoring, and
    /// `cap:fast-cache-sla` sub-25ms write-through caching in a single pass.
    pub fn list_todos_sorted(
        &mut self,
        session: &AuthSession,
    ) -> Result<(Vec<TodoItem>, bool), &'static str> {
        let owner = session.require_authenticated()?;

        if let Some(cached) = self.read_cache.get(owner) {
            return Ok((cached.clone(), true));
        }

        let mut items: Vec<TodoItem> = self
            .rows
            .values()
            .filter(|t| t.owner_email == owner)
            .cloned()
            .collect();

        // Fused sort using the sealed @native urgency decay kernel
        items.sort_by(|a, b| {
            let score_a = compute_urgency_score(a.priority_weight, a.hours_overdue);
            let score_b = compute_urgency_score(b.priority_weight, b.hours_overdue);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.id.cmp(&b.id))
        });

        self.read_cache.insert(owner.to_string(), items.clone());
        Ok((items, false))
    }
// @gvcs-end

// @gvcs-begin cell="I(cap:fast-cache-sla ∩ cap:gdpr-privacy ∩ feat:todo-crud)" symbol="TodoStore::gdpr_erase_and_export" tests="test_gdpr_erasure_wipes_storage_and_cache_synchronously"
    /// Governed by Pareto Tradeoff Pin `PARETO-001-cache-vs-gdpr` (`OPT-SYNCHRONOUS-INVALIDATION`):
    /// Exports all user data and permanently erases all rows in `DBTable:todos` PLUS synchronously
    /// evicts the `cap:fast-cache-sla` read cache so stale PII can never be served (0ms window).
    pub fn gdpr_erase_and_export(
        &mut self,
        session: &AuthSession,
    ) -> Result<(String, usize), &'static str> {
        let owner = session.require_authenticated()?;

        let user_items: Vec<TodoItem> = self
            .rows
            .values()
            .filter(|t| t.owner_email == owner)
            .cloned()
            .collect();

        let export_json =
            serde_json::to_string_pretty(&user_items).map_err(|_| "Serialization error")?;

        let before_count = self.rows.len();
        self.rows.retain(|_, v| v.owner_email != owner);
        let removed = before_count - self.rows.len();

        // [PARETO-001 Synchronous Cache Purge]: Guarantee 0ms stale PII reads
        self.read_cache.remove(owner);

        Ok((export_json, removed))
    }
// @gvcs-end

// @gvcs-begin cell="I(cap:cli-ux-shell ∩ feat:todo-crud ∩ native:priority-decay)" symbol="TodoStore::render_viewport_board" tests="test_render_viewport_board_formats_badges"
    /// Renders `Viewport:TodoList` according to `cap:cli-ux-shell` (Urgency badges first,
    /// recurrence indicators, and GDPR privacy footer).
    pub fn render_viewport_board(&mut self, session: &AuthSession) -> Result<String, &'static str> {
        let (items, from_cache) = self.list_todos_sorted(session)?;
        let mut out = String::new();
        out.push_str(&format!(
            "=== TODO BOARD ({}) [Cache Hit: {}] ===\n",
            session.user_email, from_cache
        ));

        if items.is_empty() {
            out.push_str("  (No tasks found)\n");
        } else {
            for item in &items {
                let status_icon = match item.status {
                    TodoStatus::Pending => "[ ]",
                    TodoStatus::Completed => "[✓]",
                };
                let urgency = compute_urgency_score(item.priority_weight, item.hours_overdue);
                let recur_badge = match item.recurrence_days {
                    Some(d) => format!(" [↻ {}d]", d),
                    None => String::new(),
                };
                out.push_str(&format!(
                    "  {} #{:<2} [Urgency: {:>5.1} | P{}] {}{}\n",
                    status_icon, item.id, urgency, item.priority_weight, item.title, recur_badge
                ));
            }
        }
        out.push_str("  [⋯ Overflow Menu: Confirm 1-Click GDPR Export & Account Erasure]\n");
        Ok(out)
    }
}
// @gvcs-end

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_urgency_monotonicity() {
        let low = compute_urgency_score(1, -24);
        let med = compute_urgency_score(3, 0);
        let high_overdue = compute_urgency_score(5, 12);
        assert!(low < med);
        assert!(med < high_overdue);
    }

    #[test]
    fn test_google_oauth_rejects_unauthenticated_mutations() {
        let mut store = TodoStore::new(true);
        let anon = AuthSession {
            user_email: "".to_string(),
            google_sub_id: "".to_string(),
            is_verified: false,
        };
        assert!(store.create_todo(&anon, "Hack task", 3, 0, None).is_err());
    }

    #[test]
    fn test_create_and_complete_todo() {
        let mut store = TodoStore::new(true);
        let alice = AuthSession::verified("alice@example.com");
        let task = store
            .create_todo(&alice, "Write gvcs specs", 4, 2, None)
            .unwrap();
        assert_eq!(task.status, TodoStatus::Pending);

        let (done, spawned) = store.complete_todo(&alice, task.id).unwrap();
        assert_eq!(done.status, TodoStatus::Completed);
        assert!(spawned.is_none());
    }

    #[test]
    fn test_recurring_task_spawns_next_cycle() {
        let mut store = TodoStore::new(true);
        let alice = AuthSession::verified("alice@example.com");
        let task = store
            .create_todo(&alice, "Weekly architecture review", 5, 1, Some(7))
            .unwrap();

        let (done, spawned) = store.complete_todo(&alice, task.id).unwrap();
        assert_eq!(done.status, TodoStatus::Completed);
        let next = spawned.expect("Should spawn next recurring instance");
        assert_eq!(next.status, TodoStatus::Pending);
        assert_eq!(next.recurrence_days, Some(7));
    }

    #[test]
    fn test_inc_001_idempotent_complete_never_spawns_duplicate_recurrence() {
        let mut store = TodoStore::new(true);
        let alice = AuthSession::verified("alice@example.com");
        let task = store
            .create_todo(&alice, "Pay cloud invoice", 5, 0, Some(30))
            .unwrap();

        // First completion spawns occurrence #2
        let (_, first_spawn) = store.complete_todo(&alice, task.id).unwrap();
        assert!(first_spawn.is_some());

        // Second completion (3 AM retry bug INC-001) must be an idempotent no-op!
        let (_, second_spawn) = store.complete_todo(&alice, task.id).unwrap();
        assert!(
            second_spawn.is_none(),
            "INC-001 Regression: Completing an already-completed task spawned a duplicate!"
        );
    }

    #[test]
    fn test_cached_list_sorted_by_native_urgency() {
        let mut store = TodoStore::new(true);
        let alice = AuthSession::verified("alice@example.com");
        store
            .create_todo(&alice, "Low priority task", 1, -48, None)
            .unwrap();
        store
            .create_todo(&alice, "Urgent overdue task", 5, 24, None)
            .unwrap();

        let (list1, hit1) = store.list_todos_sorted(&alice).unwrap();
        assert!(!hit1, "First read populates cache");
        assert_eq!(list1[0].title, "Urgent overdue task");

        let (list2, hit2) = store.list_todos_sorted(&alice).unwrap();
        assert!(hit2, "Second read must be served from sub-25ms memory cache");
        assert_eq!(list2.len(), 2);
    }

    #[test]
    fn test_gdpr_erasure_wipes_storage_and_cache_synchronously() {
        let mut store = TodoStore::new(true);
        let alice = AuthSession::verified("alice@example.com");
        store
            .create_todo(&alice, "Private medical appointment", 5, 0, None)
            .unwrap();

        // Populate the read cache first
        let (_, _) = store.list_todos_sorted(&alice).unwrap();

        // Trigger GDPR 1-click erasure & export
        let (exported_json, wiped_count) = store.gdpr_erase_and_export(&alice).unwrap();
        assert!(exported_json.contains("Private medical appointment"));
        assert_eq!(wiped_count, 1);

        // Verify 0ms stale reads (PARETO-001)
        let (remaining, _) = store.list_todos_sorted(&alice).unwrap();
        assert!(remaining.is_empty(), "GDPR erasure left stale PII in cache or storage!");
    }

    #[test]
    fn test_render_viewport_board_formats_badges() {
        let mut store = TodoStore::new(true);
        let alice = AuthSession::verified("alice@example.com");
        store
            .create_todo(&alice, "Publish gvcs repo", 5, 4, Some(7))
            .unwrap();
        let board = store.render_viewport_board(&alice).unwrap();
        assert!(board.contains("Publish gvcs repo"));
        assert!(board.contains("[↻ 7d]"));
        assert!(board.contains("GDPR Export & Account Erasure"));
    }
}
