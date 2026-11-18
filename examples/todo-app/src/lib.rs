//! Reference Todo Application — Committed Fused Code with `gvcs` Feature Provenance.
//!
//! Every code block in this crate is linked 1-to-1 in `provenance.lock` to its exact
//! Feature Overlap (`I(A ∩ B)`), Product Decision (`specs/exceptions/*.toml`), and
//! Bug Guard (`specs/incidents/*.toml`):
//!
//! 1. `src/native_priority.rs` -> `I(native:priority-decay)` (`compute_urgency_score`)
//! 2. `src/lib.rs`             -> `I(feat:tags-and-search ∩ feat:todo-crud)` (`TodoStore::create_todo`)
//! 3. `src/lib.rs`             -> `I(feat:recurring-tasks ∩ feat:tags-and-search ∩ feat:todo-crud)` (`TodoStore::complete_todo`)
//! 4. `src/lib.rs`             -> `I(cap:fast-cache-sla ∩ feat:tags-and-search ∩ feat:todo-crud ∩ native:priority-decay)` (`TodoStore::list_todos_sorted`)
//! 5. `src/lib.rs`             -> `I(cap:fast-cache-sla ∩ cap:gdpr-privacy ∩ feat:todo-crud)` (`TodoStore::gdpr_erase_and_export`)
//! 6. `src/web_ui.rs`          -> `I(cap:web-ui ∩ feat:tags-and-search ∩ feat:todo-crud ∩ native:priority-decay)` (`web_ui::dispatch_http_request`)

pub mod native_priority;
pub mod web_ui;

use chrono::{DateTime, Duration, Utc};
use native_priority::compute_urgency_score;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

// @gvcs-begin cell="I(feat:tags-and-search ∩ feat:todo-crud)" symbol="TodoStore::create_todo" tests="test_create_and_complete_todo,test_tags_extraction_and_search_filter,test_persistent_disk_roundtrip"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoStatus {
    Pending,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: u64,
    pub title: String,
    pub status: TodoStatus,
    pub priority_weight: u8,
    pub hours_overdue: i64,
    /// Optional recurrence interval in days (`feat:recurring-tasks`).
    pub recurrence_days: Option<u32>,
    /// Project & context tags (`feat:tags-and-search`), e.g. `["#rust", "#work"]`.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Cached urgency score from `@native` kernel (`native:priority-decay`) for Web UI projection.
    pub urgency_score: f64,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Filter criteria for querying the Todo board (`feat:tags-and-search`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoFilter {
    pub tag: Option<String>,
    pub search_query: Option<String>,
    pub only_pending: bool,
}

/// Extracts inline `#tags` from a task title and merges them with explicit tags (`EXC-003-crud-vs-tags`).
pub fn normalize_tags(title: &str, explicit_tags: &[String]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for word in title.split_whitespace() {
        if word.starts_with('#') && word.len() > 1 {
            set.insert(word.to_lowercase());
        }
    }
    for t in explicit_tags {
        let trimmed = t.trim().to_lowercase();
        if !trimmed.is_empty() {
            if trimmed.starts_with('#') {
                set.insert(trimmed);
            } else {
                set.insert(format!("#{}", trimmed));
            }
        }
    }
    set.into_iter().collect()
}

/// Persistent & in-memory database substrate (`DBTable:todos`) + write-through read cache (`cap:fast-cache-sla`).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TodoStore {
    next_id: u64,
    rows: BTreeMap<u64, TodoItem>,
    /// Write-through read cache (`cap:fast-cache-sla`).
    #[serde(skip)]
    read_cache: Option<Vec<TodoItem>>,
    /// Runtime canary flag for `feat:recurring-tasks` (`Rollout: Canary 25% (flag_recurring_tasks)`).
    pub flag_recurring_tasks: bool,
    /// Optional disk backing path for persistent storage.
    #[serde(skip)]
    storage_path: Option<PathBuf>,
}

impl TodoStore {
    pub fn new(flag_recurring_tasks: bool) -> Self {
        Self {
            next_id: 1,
            rows: BTreeMap::new(),
            read_cache: None,
            flag_recurring_tasks,
            storage_path: None,
        }
    }

    /// Loads or initializes a persistent `TodoStore` backed by a JSON database file on disk.
    pub fn load_or_init(path: &Path, flag_recurring_tasks: bool) -> Result<Self, String> {
        if path.exists() {
            let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
            let mut store: Self = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            store.flag_recurring_tasks = flag_recurring_tasks;
            store.storage_path = Some(path.to_path_buf());
            Ok(store)
        } else {
            let mut store = Self::new(flag_recurring_tasks);
            store.storage_path = Some(path.to_path_buf());
            store.seed_default_demo_tasks();
            store.flush_to_disk()?;
            Ok(store)
        }
    }

    pub fn seed_default_demo_tasks(&mut self) {
        if self.rows.is_empty() {
            let _ = self.create_todo_with_tags(
                "Fix production Stripe webhook timeout #urgent",
                5,
                18,
                None,
                &["work".to_string(), "backend".to_string()],
            );
            let _ = self.create_todo_with_tags(
                "Run weekly gvcs adherence sweep #ops",
                5,
                6,
                Some(7),
                &["rust".to_string()],
            );
            let _ = self.create_todo_with_tags(
                "Prepare Q4 product roadmap #strategy",
                2,
                -24,
                None,
                &["work".to_string()],
            );
            let _ = self.create_todo("Pick up espresso beans #home", 1, -8, None);
        }
    }

    pub fn flush_to_disk(&self) -> Result<(), String> {
        if let Some(ref path) = self.storage_path {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
            }
            let serialized = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
            fs::write(path, serialized).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Creates a new Todo item.
    pub fn create_todo(
        &mut self,
        title: &str,
        priority_weight: u8,
        hours_overdue: i64,
        recurrence_days: Option<u32>,
    ) -> Result<TodoItem, &'static str> {
        self.create_todo_with_tags(title, priority_weight, hours_overdue, recurrence_days, &[])
    }

    /// Creates a new Todo item with explicit + inline `#tags` (`EXC-003-crud-vs-tags`).
    pub fn create_todo_with_tags(
        &mut self,
        title: &str,
        priority_weight: u8,
        hours_overdue: i64,
        recurrence_days: Option<u32>,
        explicit_tags: &[String],
    ) -> Result<TodoItem, &'static str> {
        if title.trim().is_empty() {
            return Err("ValidationError: task title cannot be empty");
        }
        let id = self.next_id;
        self.next_id += 1;
        let clamped_p = priority_weight.clamp(1, 5);

        let item = TodoItem {
            id,
            title: title.trim().to_string(),
            status: TodoStatus::Pending,
            priority_weight: clamped_p,
            hours_overdue,
            recurrence_days,
            tags: normalize_tags(title, explicit_tags),
            urgency_score: compute_urgency_score(clamped_p, hours_overdue),
            created_at: Utc::now(),
            completed_at: None,
        };

        self.rows.insert(id, item.clone());
        // Invalidate write-through read cache on mutation
        self.read_cache = None;
        let _ = self.flush_to_disk();
        Ok(item)
    }

    /// Deletes a single todo item by ID.
    pub fn delete_todo(&mut self, todo_id: u64) -> Result<TodoItem, &'static str> {
        let removed = self
            .rows
            .remove(&todo_id)
            .ok_or("NotFound: todo item does not exist")?;
        self.read_cache = None;
        let _ = self.flush_to_disk();
        Ok(removed)
    }
// @gvcs-end

// @gvcs-begin cell="I(feat:recurring-tasks ∩ feat:tags-and-search ∩ feat:todo-crud)" symbol="TodoStore::complete_todo" tests="test_recurring_task_spawns_next_cycle,test_inc_001_idempotent_complete_never_spawns_duplicate_recurrence"
    /// Fused completion & recurring roll-forward mutation.
    /// Governed by:
    /// - Product Decision `EXC-002-crud-vs-recurring` (atomic complete & spawn)
    /// - Product Decision `EXC-004-recurring-vs-tags` (carry forward #tags to spawned cycle)
    /// - Bug Guard `INC-001-double-complete-spawn` (idempotent guard on already-Completed tasks)
    /// - Canary Rollout Flag `flag_recurring_tasks`
    pub fn complete_todo(
        &mut self,
        todo_id: u64,
    ) -> Result<(TodoItem, Option<TodoItem>), &'static str> {
        let existing = self
            .rows
            .get_mut(&todo_id)
            .ok_or("NotFound: todo item does not exist")?;

        // [INC-001 Permanent Bug Guard]: Completing an already-Completed task is a safe
        // no-op and must NEVER spawn duplicate recurring occurrences.
        if existing.status == TodoStatus::Completed {
            return Ok((existing.clone(), None));
        }

        existing.status = TodoStatus::Completed;
        existing.completed_at = Some(Utc::now());
        let completed_snapshot = existing.clone();
        let recurrence = existing.recurrence_days;

        let spawned = if self.flag_recurring_tasks {
            if let Some(days) = recurrence {
                let next_id = self.next_id;
                self.next_id += 1;
                let next_overdue = -(days as i64 * 24);
                let next_item = TodoItem {
                    id: next_id,
                    title: completed_snapshot.title.clone(),
                    status: TodoStatus::Pending,
                    priority_weight: completed_snapshot.priority_weight,
                    hours_overdue: next_overdue,
                    recurrence_days: Some(days),
                    // [EXC-004]: Inherit #tags on spawned recurring occurrence
                    tags: completed_snapshot.tags.clone(),
                    urgency_score: compute_urgency_score(
                        completed_snapshot.priority_weight,
                        next_overdue,
                    ),
                    created_at: Utc::now() + Duration::days(days as i64),
                    completed_at: None,
                };
                self.rows.insert(next_id, next_item.clone());
                Some(next_item)
            } else {
                None
            }
        } else {
            None
        };

        self.read_cache = None;
        let _ = self.flush_to_disk();
        Ok((completed_snapshot, spawned))
    }
// @gvcs-end

// @gvcs-begin cell="I(cap:fast-cache-sla ∩ feat:tags-and-search ∩ feat:todo-crud ∩ native:priority-decay)" symbol="TodoStore::list_todos_sorted" tests="test_cached_list_sorted_by_native_urgency,test_tags_extraction_and_search_filter"
    /// Joint Fused Query:
    /// Combines `feat:todo-crud` retrieval, `native:priority-decay` urgency scoring,
    /// `feat:tags-and-search` filtering, and `cap:fast-cache-sla` sub-25ms write-through caching.
    pub fn list_todos_sorted(&mut self) -> Result<(Vec<TodoItem>, bool), &'static str> {
        self.list_todos_filtered(&TodoFilter::default())
    }

    pub fn list_todos_filtered(
        &mut self,
        filter: &TodoFilter,
    ) -> Result<(Vec<TodoItem>, bool), &'static str> {
        let (base_items, cache_hit) = if let Some(ref cached) = self.read_cache {
            (cached.clone(), true)
        } else {
            let mut items: Vec<TodoItem> = self.rows.values().cloned().collect();

            // Fused sort using the sealed @native urgency decay kernel
            items.sort_by(|a, b| {
                let score_a = compute_urgency_score(a.priority_weight, a.hours_overdue);
                let score_b = compute_urgency_score(b.priority_weight, b.hours_overdue);
                score_b
                    .partial_cmp(&score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.id.cmp(&b.id))
            });

            self.read_cache = Some(items.clone());
            (items, false)
        };

        let filtered: Vec<TodoItem> = base_items
            .into_iter()
            .filter(|item| {
                if filter.only_pending && item.status != TodoStatus::Pending {
                    return false;
                }
                if let Some(ref tag_q) = filter.tag {
                    let trimmed = tag_q.trim();
                    if !trimmed.is_empty() {
                        let norm_tag = if trimmed.starts_with('#') {
                            trimmed.to_lowercase()
                        } else {
                            format!("#{}", trimmed.to_lowercase())
                        };
                        if !item.tags.iter().any(|t| t == &norm_tag) {
                            return false;
                        }
                    }
                }
                if let Some(ref q) = filter.search_query {
                    let lower_q = q.trim().to_lowercase();
                    if !lower_q.is_empty() {
                        let in_title = item.title.to_lowercase().contains(&lower_q);
                        let in_tags = item.tags.iter().any(|t| t.contains(&lower_q));
                        if !in_title && !in_tags {
                            return false;
                        }
                    }
                }
                true
            })
            .collect();

        Ok((filtered, cache_hit))
    }
// @gvcs-end

// @gvcs-begin cell="I(cap:fast-cache-sla ∩ cap:gdpr-privacy ∩ feat:todo-crud)" symbol="TodoStore::gdpr_erase_and_export" tests="test_gdpr_erasure_wipes_storage_and_cache_synchronously"
    /// Governed by Product Decision `PARETO-001-cache-vs-gdpr`:
    /// Exports all user data and permanently erases all rows in `DBTable:todos` PLUS synchronously
    /// evicts the `cap:fast-cache-sla` read cache so stale personal data can never be served (0ms window).
    pub fn gdpr_erase_and_export(&mut self) -> Result<(String, usize), &'static str> {
        let all_items: Vec<TodoItem> = self.rows.values().cloned().collect();
        let export_json =
            serde_json::to_string_pretty(&all_items).map_err(|_| "Serialization error")?;

        let removed = self.rows.len();
        self.rows.clear();

        // [PARETO-001 Synchronous Cache Purge]: Guarantee 0ms stale reads
        self.read_cache = None;
        let _ = self.flush_to_disk();

        Ok((export_json, removed))
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
    fn test_create_and_complete_todo() {
        let mut store = TodoStore::new(true);
        let task = store.create_todo("Write gvcs specs", 4, 2, None).unwrap();
        assert_eq!(task.status, TodoStatus::Pending);

        let (done, spawned) = store.complete_todo(task.id).unwrap();
        assert_eq!(done.status, TodoStatus::Completed);
        assert!(done.completed_at.is_some());
        assert!(spawned.is_none());
    }

    #[test]
    fn test_recurring_task_spawns_next_cycle() {
        let mut store = TodoStore::new(true);
        let task = store
            .create_todo_with_tags(
                "Weekly architecture review #arch",
                5,
                1,
                Some(7),
                &["team".to_string()],
            )
            .unwrap();

        let (done, spawned) = store.complete_todo(task.id).unwrap();
        assert_eq!(done.status, TodoStatus::Completed);
        let next = spawned.expect("Should spawn next recurring instance");
        assert_eq!(next.status, TodoStatus::Pending);
        assert_eq!(next.recurrence_days, Some(7));
        // Verify EXC-004: spawned recurring task inherits parent #tags
        assert_eq!(next.tags, vec!["#arch".to_string(), "#team".to_string()]);
    }

    #[test]
    fn test_inc_001_idempotent_complete_never_spawns_duplicate_recurrence() {
        let mut store = TodoStore::new(true);
        let task = store
            .create_todo("Pay cloud invoice", 5, 0, Some(30))
            .unwrap();

        // First completion spawns occurrence #2
        let (_, first_spawn) = store.complete_todo(task.id).unwrap();
        assert!(first_spawn.is_some());

        // Second completion (3 AM retry bug INC-001) must be an idempotent no-op!
        let (_, second_spawn) = store.complete_todo(task.id).unwrap();
        assert!(
            second_spawn.is_none(),
            "INC-001 Regression: Completing an already-completed task spawned a duplicate!"
        );
    }

    #[test]
    fn test_cached_list_sorted_by_native_urgency() {
        let mut store = TodoStore::new(true);
        store.create_todo("Low priority task", 1, -48, None).unwrap();
        store
            .create_todo("Urgent overdue task", 5, 24, None)
            .unwrap();

        let (list1, hit1) = store.list_todos_sorted().unwrap();
        assert!(!hit1, "First read populates cache");
        assert_eq!(list1[0].title, "Urgent overdue task");

        let (list2, hit2) = store.list_todos_sorted().unwrap();
        assert!(hit2, "Second read must be served from sub-25ms memory cache");
        assert_eq!(list2.len(), 2);
    }

    #[test]
    fn test_tags_extraction_and_search_filter() {
        let mut store = TodoStore::new(true);
        store
            .create_todo_with_tags(
                "Ship Rust gvcs compiler #rust",
                5,
                2,
                None,
                &["work".to_string()],
            )
            .unwrap();
        store.create_todo("Buy groceries #home", 2, -12, None).unwrap();

        let (rust_only, _) = store
            .list_todos_filtered(&TodoFilter {
                tag: Some("rust".to_string()),
                search_query: None,
                only_pending: true,
            })
            .unwrap();
        assert_eq!(rust_only.len(), 1);
        assert_eq!(
            rust_only[0].tags,
            vec!["#rust".to_string(), "#work".to_string()]
        );

        let (search_hit, _) = store
            .list_todos_filtered(&TodoFilter {
                tag: None,
                search_query: Some("groceries".to_string()),
                only_pending: false,
            })
            .unwrap();
        assert_eq!(search_hit.len(), 1);
        assert_eq!(search_hit[0].title, "Buy groceries #home");
    }

    #[test]
    fn test_persistent_disk_roundtrip() {
        let temp_path = std::env::temp_dir().join("gvcs_todo_web_test_roundtrip.json");
        let _ = fs::remove_file(&temp_path);

        {
            let mut store = TodoStore::new(true);
            store
                .create_todo("Persistent disk task #ops", 4, 0, Some(14))
                .unwrap();
            let serialized = serde_json::to_string_pretty(&store).unwrap();
            fs::write(&temp_path, serialized).unwrap();
        }
        {
            let mut reloaded = TodoStore::load_or_init(&temp_path, true).unwrap();
            let (items, _) = reloaded.list_todos_sorted().unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].title, "Persistent disk task #ops");
            assert_eq!(items[0].tags, vec!["#ops".to_string()]);
        }
        let _ = fs::remove_file(&temp_path);
    }

    #[test]
    fn test_gdpr_erasure_wipes_storage_and_cache_synchronously() {
        let mut store = TodoStore::new(true);
        store
            .create_todo("Private medical appointment", 5, 0, None)
            .unwrap();

        // Populate the read cache first
        let (_, _) = store.list_todos_sorted().unwrap();

        // Trigger GDPR 1-click erasure & export
        let (exported_json, wiped_count) = store.gdpr_erase_and_export().unwrap();
        assert!(exported_json.contains("Private medical appointment"));
        assert_eq!(wiped_count, 1);

        // Verify 0ms stale reads (PARETO-001)
        let (remaining, _) = store.list_todos_sorted().unwrap();
        assert!(
            remaining.is_empty(),
            "GDPR erasure left stale PII in cache or storage!"
        );
    }
}
