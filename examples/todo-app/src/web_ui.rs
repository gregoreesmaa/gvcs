//! Web UI & REST API Projection (`cap:web-ui`).
//!
//! Serves an interactive Single-Page Web Application + JSON REST API with a built-in
//! **gvcs Provenance X-Ray** toggle so both engineers and laypeople can visually inspect
//! which features, product decisions, and bug guards power every UI element and API action.

use crate::{TodoFilter, TodoStore};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

// @gvcs-begin cell="I(cap:web-ui ∩ feat:tags-and-search ∩ feat:todo-crud ∩ native:priority-decay)" symbol="web_ui::dispatch_http_request" tests="test_web_ui_rest_api_and_dashboard_html"
#[derive(Debug, Deserialize)]
pub struct CreateTodoRequest {
    pub title: String,
    pub priority_weight: Option<u8>,
    pub hours_overdue: Option<i64>,
    pub recurrence_days: Option<u32>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ListTodosResponse {
    pub items: Vec<crate::TodoItem>,
    pub cache_hit: bool,
    pub canary_recurring_enabled: bool,
}

/// Processes a single HTTP request (method, path+query, body) against `TodoStore` and returns
/// `(status_code, content_type, response_body)`.
pub fn dispatch_http_request(
    store: &mut TodoStore,
    method: &str,
    path_and_query: &str,
    body: &str,
) -> (u16, &'static str, String) {
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            (200, "text/html; charset=utf-8", WEB_UI_HTML.to_string())
        }

        ("GET", "/api/todos") => {
            let filter = parse_query_filter(query);
            match store.list_todos_filtered(&filter) {
                Ok((items, cache_hit)) => {
                    let resp = ListTodosResponse {
                        items,
                        cache_hit,
                        canary_recurring_enabled: store.flag_recurring_tasks,
                    };
                    (
                        200,
                        "application/json",
                        serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string()),
                    )
                }
                Err(e) => (400, "application/json", format!("{{\"error\":\"{}\"}}", e)),
            }
        }

        ("POST", "/api/todos") => match serde_json::from_str::<CreateTodoRequest>(body) {
            Ok(req) => {
                let tags = req.tags.unwrap_or_default();
                match store.create_todo_with_tags(
                    &req.title,
                    req.priority_weight.unwrap_or(3),
                    req.hours_overdue.unwrap_or(0),
                    req.recurrence_days,
                    &tags,
                ) {
                    Ok(created) => (
                        201,
                        "application/json",
                        serde_json::to_string(&created).unwrap_or_else(|_| "{}".to_string()),
                    ),
                    Err(e) => (400, "application/json", format!("{{\"error\":\"{}\"}}", e)),
                }
            }
            Err(e) => (
                400,
                "application/json",
                format!("{{\"error\":\"Invalid JSON: {}\"}}", e),
            ),
        },

        ("POST", p) if p.starts_with("/api/todos/") && p.ends_with("/complete") => {
            let id_str = p
                .trim_start_matches("/api/todos/")
                .trim_end_matches("/complete");
            if let Ok(id) = id_str.parse::<u64>() {
                match store.complete_todo(id) {
                    Ok((completed, spawned)) => (
                        200,
                        "application/json",
                        serde_json::to_string(&serde_json::json!({
                            "completed": completed,
                            "spawned_next": spawned,
                            "governed_by": {
                                "cell": "I(feat:recurring-tasks ∩ feat:tags-and-search ∩ feat:todo-crud)",
                                "decisions": ["EXC-002-crud-vs-recurring", "EXC-004-recurring-vs-tags"],
                                "bug_guard": "INC-001-double-complete-spawn"
                            }
                        }))
                        .unwrap_or_else(|_| "{}".to_string()),
                    ),
                    Err(e) => (404, "application/json", format!("{{\"error\":\"{}\"}}", e)),
                }
            } else {
                (
                    400,
                    "application/json",
                    "{\"error\":\"Invalid task ID\"}".to_string(),
                )
            }
        }

        ("DELETE", p) if p.starts_with("/api/todos/") => {
            let id_str = p.trim_start_matches("/api/todos/");
            if let Ok(id) = id_str.parse::<u64>() {
                match store.delete_todo(id) {
                    Ok(deleted) => (
                        200,
                        "application/json",
                        serde_json::to_string(&deleted).unwrap_or_else(|_| "{}".to_string()),
                    ),
                    Err(e) => (404, "application/json", format!("{{\"error\":\"{}\"}}", e)),
                }
            } else {
                (
                    400,
                    "application/json",
                    "{\"error\":\"Invalid task ID\"}".to_string(),
                )
            }
        }

        ("POST", "/api/gdpr-erase") => match store.gdpr_erase_and_export() {
            Ok((exported_json, wiped_count)) => (
                200,
                "application/json",
                serde_json::to_string(&serde_json::json!({
                    "wiped_count": wiped_count,
                    "exported_archive": serde_json::from_str::<serde_json::Value>(&exported_json).unwrap_or(serde_json::Value::Null),
                    "governed_by": {
                        "cell": "I(cap:fast-cache-sla ∩ cap:gdpr-privacy ∩ feat:todo-crud)",
                        "decision": "PARETO-001-cache-vs-gdpr"
                    }
                }))
                .unwrap_or_else(|_| "{}".to_string()),
            ),
            Err(e) => (500, "application/json", format!("{{\"error\":\"{}\"}}", e)),
        },

        ("POST", "/api/toggle-canary") => {
            store.flag_recurring_tasks = !store.flag_recurring_tasks;
            let _ = store.flush_to_disk();
            (
                200,
                "application/json",
                format!(
                    "{{\"canary_recurring_enabled\":{}}}",
                    store.flag_recurring_tasks
                ),
            )
        }

        ("POST", "/api/seed") => {
            store.seed_default_demo_tasks();
            let _ = store.flush_to_disk();
            (200, "application/json", "{\"status\":\"seeded\"}".to_string())
        }

        _ => (404, "application/json", "{\"error\":\"Not Found\"}".to_string()),
    }
}

fn parse_query_filter(query: &str) -> TodoFilter {
    let mut filter = TodoFilter::default();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let decoded = v.replace("%23", "#").replace('+', " ");
            match k {
                "tag" if !decoded.is_empty() => filter.tag = Some(decoded),
                "search" if !decoded.is_empty() => filter.search_query = Some(decoded),
                "only_pending" => filter.only_pending = decoded == "true" || decoded == "1",
                _ => {}
            }
        }
    }
    filter
}

/// Starts the live HTTP Web UI server on `addr` (e.g., `127.0.0.1:8080`).
pub fn serve_web_ui(store: Arc<Mutex<TodoStore>>, addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    println!("🚀 gvcs Todo Web UI listening at http://{}", addr);

    for stream in listener.incoming().flatten() {
        let store_ref = Arc::clone(&store);
        let _ = handle_connection(stream, store_ref);
    }
    Ok(())
}

fn handle_connection(
    mut stream: std::net::TcpStream,
    store: Arc<Mutex<TodoStore>>,
) -> std::io::Result<()> {
    let mut buf = [0u8; 16384];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(());
    }
    let raw_req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = raw_req.lines();
    let request_line = lines.next().unwrap_or("GET / HTTP/1.1");
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let method = parts.first().copied().unwrap_or("GET");
    let path_and_query = parts.get(1).copied().unwrap_or("/");

    let body = raw_req
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or("")
        .trim_matches('\0');

    let (status, content_type, resp_body) = {
        let mut guard = store.lock().unwrap();
        dispatch_http_request(&mut guard, method, path_and_query, body)
    };

    let status_text = match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Internal Server Error",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
        status,
        status_text,
        content_type,
        resp_body.len(),
        resp_body
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

const WEB_UI_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>gvcs Todo App — Feature-Aware Web UI Showcase</title>
  <style>
    :root {
      --bg: #0f172a;
      --panel: #1e293b;
      --border: #334155;
      --text: #f8fafc;
      --muted: #94a3b8;
      --accent: #38bdf8;
      --urgent: #f43f5e;
      --success: #10b981;
      --purple: #a855f7;
      --amber: #f59e0b;
    }
    * { box-sizing: border-box; font-family: system-ui, -apple-system, sans-serif; }
    body { margin: 0; padding: 28px 16px; background: var(--bg); color: var(--text); display: flex; justify-content: center; }
    .container { width: 100%; max-width: 920px; }
    header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 18px; border-bottom: 1px solid var(--border); padding-bottom: 14px; flex-wrap: wrap; gap: 10px; }
    h1 { margin: 0; font-size: 1.35rem; display: flex; align-items: center; gap: 10px; }
    .badge-pill { font-size: 0.75rem; background: #0284c7; padding: 4px 10px; border-radius: 999px; font-weight: 600; }
    .header-controls { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
    .card { background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 16px; margin-bottom: 14px; }
    .form-row { display: grid; grid-template-columns: 1fr 115px 125px 115px auto; gap: 10px; }
    input, select, button { padding: 9px 12px; border-radius: 8px; border: 1px solid var(--border); background: #0f172a; color: var(--text); font-size: 0.88rem; }
    button { cursor: pointer; background: var(--accent); color: #0f172a; font-weight: 700; border: none; transition: opacity 0.15s; }
    button:hover { opacity: 0.9; }
    .btn-outline { background: transparent; border: 1px solid var(--border); color: var(--text); font-weight: 600; }
    .btn-outline.active { border-color: var(--accent); color: var(--accent); background: rgba(56,189,248,0.1); }
    .filters { display: flex; gap: 10px; align-items: center; margin-bottom: 14px; flex-wrap: wrap; }
    .task-list { display: flex; flex-direction: column; gap: 10px; }
    .task-item { display: flex; justify-content: space-between; align-items: center; background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 13px 16px; }
    .task-item.completed { opacity: 0.55; }
    .task-item.completed .task-title { text-decoration: line-through; }
    .task-left { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
    .urgency-chip { font-size: 0.74rem; font-weight: 700; padding: 3px 8px; border-radius: 6px; background: #334155; color: var(--accent); }
    .urgency-chip.high { background: rgba(244,63,94,0.18); color: var(--urgent); border: 1px solid rgba(244,63,94,0.4); }
    .tag-chip { font-size: 0.74rem; padding: 2px 8px; border-radius: 99px; background: rgba(56,189,248,0.15); color: var(--accent); cursor: pointer; }
    .recur-chip { font-size: 0.74rem; padding: 2px 8px; border-radius: 99px; background: rgba(168,85,247,0.18); color: #c084fc; }
    .xray-bar { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin-top: 8px; padding-top: 8px; border-top: 1px dashed rgba(148,163,184,0.25); font-size: 0.73rem; color: var(--muted); }
    .xray-hidden .xray-bar { display: none !important; }
    .xray-cell { background: rgba(16,185,129,0.15); color: #34d399; padding: 2px 7px; border-radius: 4px; font-family: monospace; }
    .xray-pin { background: rgba(245,158,11,0.16); color: #fbbf24; padding: 2px 7px; border-radius: 4px; font-family: monospace; }
    .xray-guard { background: rgba(244,63,94,0.16); color: #fb7185; padding: 2px 7px; border-radius: 4px; font-family: monospace; }
    .footer-bar { display: flex; justify-content: space-between; align-items: center; margin-top: 22px; padding-top: 14px; border-top: 1px solid var(--border); font-size: 0.84rem; color: var(--muted); flex-wrap: wrap; gap: 10px; }
    .btn-danger { background: transparent; border: 1px solid var(--urgent); color: var(--urgent); }
    .btn-danger:hover { background: var(--urgent); color: white; }
    .btn-sm { padding: 6px 10px; font-size: 0.78rem; }
    pre { background: #020617; padding: 12px; border-radius: 8px; overflow-x: auto; font-size: 0.76rem; color: #38bdf8; }
  </style>
</head>
<body>
  <div class="container" id="appRoot">
    <header>
      <h1>
        <span>⚡ gvcs Todo Workspace</span>
        <span class="badge-pill">cap:web-ui</span>
      </h1>
      <div class="header-controls">
        <button id="xrayBtn" class="btn-outline active btn-sm" onclick="toggleXray()">🔬 gvcs Provenance X-Ray: ON</button>
        <button id="canaryBtn" class="btn-outline active btn-sm" onclick="toggleCanary()">🐤 Canary Recurring Tasks: ON</button>
        <span style="font-size:0.78rem;color:var(--muted)" id="cacheStatus">Cache: --</span>
      </div>
    </header>

    <div class="card">
      <div class="form-row">
        <input id="titleInput" placeholder="Add task with inline #tags (e.g. Ship Web UI #rust #release)" />
        <select id="priorityInput">
          <option value="5">P5 (Critical)</option>
          <option value="4">P4 (High)</option>
          <option value="3" selected>P3 (Normal)</option>
          <option value="2">P2 (Low)</option>
          <option value="1">P1 (Backlog)</option>
        </select>
        <input id="overdueInput" type="number" value="0" placeholder="Overdue hrs" title="Positive = hours overdue, Negative = hours until due" />
        <input id="recurInput" type="number" placeholder="Recur (days)" title="Optional recurring interval in days" />
        <button onclick="createTodo()">+ Add Task</button>
      </div>
      <div class="xray-bar">
        <span>🔬 <strong>Code Block:</strong> <code>src/lib.rs::TodoStore::create_todo</code></span>
        <span class="xray-cell">feat:todo-crud + feat:tags-and-search</span>
        <span class="xray-pin">Decision: EXC-003 (Inline #tag extraction)</span>
      </div>
    </div>

    <div class="card" style="padding: 12px 16px;">
      <div class="filters" style="margin-bottom:0">
        <input id="searchInput" placeholder="Search title or #tags..." oninput="loadTodos()" style="flex:1" />
        <input id="tagInput" placeholder="Filter #tag (e.g. #work)" oninput="loadTodos()" style="width:180px" />
        <label style="display:flex;align-items:center;gap:6px;font-size:0.85rem;cursor:pointer">
          <input type="checkbox" id="pendingOnlyInput" onchange="loadTodos()" /> Pending Only
        </label>
      </div>
      <div class="xray-bar">
        <span>🔬 <strong>Fused Query:</strong> <code>src/lib.rs::TodoStore::list_todos_sorted</code></span>
        <span class="xray-cell">feat:todo-crud + feat:tags-and-search + native:priority-decay + cap:fast-cache-sla</span>
      </div>
    </div>

    <div class="task-list" id="taskList"></div>

    <div class="footer-bar">
      <div>
        <div>1-Click GDPR Portability & Right to Erasure</div>
        <div class="xray-bar" style="border:none;padding-top:4px;margin-top:0">
          <span>🔬 <code>src/lib.rs::TodoStore::gdpr_erase_and_export</code></span>
          <span class="xray-cell">feat:todo-crud + cap:gdpr-privacy + cap:fast-cache-sla</span>
          <span class="xray-pin">Decision: PARETO-001 (0ms Sync Cache Purge)</span>
        </div>
      </div>
      <button class="btn-danger btn-sm" onclick="triggerGdprErase()">⋯ 1-Click GDPR Export & Erase</button>
    </div>

    <div id="gdprResult" style="display:none; margin-top:16px;" class="card"></div>
  </div>

  <script>
    let xrayOn = true;

    function toggleXray() {
      xrayOn = !xrayOn;
      document.getElementById('appRoot').classList.toggle('xray-hidden', !xrayOn);
      const btn = document.getElementById('xrayBtn');
      btn.textContent = `🔬 gvcs Provenance X-Ray: ${xrayOn ? 'ON' : 'OFF'}`;
      btn.classList.toggle('active', xrayOn);
    }

    async function toggleCanary() {
      await fetch('/api/toggle-canary', { method: 'POST' });
      loadTodos();
    }

    async function loadTodos() {
      const search = encodeURIComponent(document.getElementById('searchInput').value.trim());
      const tag = encodeURIComponent(document.getElementById('tagInput').value.trim());
      const onlyPending = document.getElementById('pendingOnlyInput').checked;
      const res = await fetch(`/api/todos?search=${search}&tag=${tag}&only_pending=${onlyPending}`);
      const data = await res.json();

      document.getElementById('cacheStatus').textContent =
        `cap:fast-cache-sla Hit: ${data.cache_hit ? 'true (<1ms)' : 'false'}`;

      const canaryBtn = document.getElementById('canaryBtn');
      canaryBtn.textContent = `🐤 Canary Recurring Tasks: ${data.canary_recurring_enabled ? 'ON (25%)' : 'OFF'}`;
      canaryBtn.classList.toggle('active', data.canary_recurring_enabled);

      const list = document.getElementById('taskList');
      if (!data.items.length) {
        list.innerHTML = `<div class="card" style="text-align:center;color:var(--muted)">No tasks match current filter. <button class="btn-sm" style="margin-left:10px" onclick="seedDemo()">Restore Demo Tasks</button></div>`;
        return;
      }
      list.innerHTML = data.items.map(item => {
        const isDone = item.status === 'Completed';
        const highUrgency = item.urgency_score >= 120;
        const dueText = item.hours_overdue > 0 ? `${item.hours_overdue}h overdue` : (item.hours_overdue === 0 ? 'due now' : `due in ${-item.hours_overdue}h`);
        const tagsHtml = (item.tags || []).map(t => `<span class="tag-chip" onclick="filterByTag('${t}')">${t}</span>`).join('');
        const recurHtml = item.recurrence_days ? `<span class="recur-chip">↻ ${item.recurrence_days}d</span>` : '';
        return `
          <div class="task-item ${isDone ? 'completed' : ''}" style="flex-direction:column;align-items:stretch">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <div class="task-left">
                <input type="checkbox" ${isDone ? 'checked disabled' : ''} onchange="completeTodo(${item.id})" />
                <span class="urgency-chip ${highUrgency ? 'high' : ''}" title="@native kernel: src/native_priority.rs">Urgency ${item.urgency_score.toFixed(1)} · P${item.priority_weight} · ${dueText}</span>
                <span class="task-title" style="font-weight:600">${escapeHtml(item.title)}</span>
                ${recurHtml}
                ${tagsHtml}
              </div>
              <button class="btn-danger btn-sm" onclick="deleteTodo(${item.id})">✕</button>
            </div>
            <div class="xray-bar">
              <span>🔬 <strong>Complete Action:</strong> <code>TodoStore::complete_todo</code></span>
              <span class="xray-cell">feat:todo-crud + feat:recurring-tasks + feat:tags-and-search</span>
              <span class="xray-pin">EXC-002 & EXC-004</span>
              <span class="xray-guard">🛡 Bug Guard: INC-001 (No duplicate spawn)</span>
            </div>
          </div>
        `;
      }).join('');
    }

    async function createTodo() {
      const titleEl = document.getElementById('titleInput');
      if (!titleEl.value.trim()) return;
      const priority_weight = parseInt(document.getElementById('priorityInput').value, 10);
      const hours_overdue = parseInt(document.getElementById('overdueInput').value || '0', 10);
      const recurVal = document.getElementById('recurInput').value;
      const recurrence_days = recurVal ? parseInt(recurVal, 10) : null;

      await fetch('/api/todos', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: titleEl.value, priority_weight, hours_overdue, recurrence_days })
      });
      titleEl.value = '';
      document.getElementById('recurInput').value = '';
      loadTodos();
    }

    async function completeTodo(id) {
      await fetch(`/api/todos/${id}/complete`, { method: 'POST' });
      loadTodos();
    }

    async function deleteTodo(id) {
      await fetch(`/api/todos/${id}`, { method: 'DELETE' });
      loadTodos();
    }

    function filterByTag(t) {
      document.getElementById('tagInput').value = t;
      loadTodos();
    }

    async function seedDemo() {
      await fetch('/api/seed', { method: 'POST' });
      document.getElementById('gdprResult').style.display = 'none';
      loadTodos();
    }

    async function triggerGdprErase() {
      if (!confirm('GDPR Right to Erasure (cap:gdpr-privacy ∩ PARETO-001): Export portable JSON archive and permanently wipe all tasks & memory caches?')) return;
      const res = await fetch('/api/gdpr-erase', { method: 'POST' });
      const data = await res.json();
      const box = document.getElementById('gdprResult');
      box.style.display = 'block';
      box.innerHTML = `<strong>✓ GDPR Erasure Complete:</strong> Wiped ${data.wiped_count} row(s) & synchronously purged read cache (0ms stale PII, enforced by <code>${data.governed_by.decision}</code>).<pre>${escapeHtml(JSON.stringify(data.exported_archive, null, 2))}</pre>`;
      loadTodos();
    }

    function escapeHtml(s) {
      return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
    }

    loadTodos();
  </script>
</body>
</html>
"#;
// @gvcs-end

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_ui_rest_api_and_dashboard_html() {
        let mut store = TodoStore::new(true);

        // 1. GET / serves the interactive Web UI Single-Page App with Provenance X-Ray
        let (status, content_type, html) = dispatch_http_request(&mut store, "GET", "/", "");
        assert_eq!(status, 200);
        assert!(content_type.contains("text/html"));
        assert!(html.contains("gvcs Todo Workspace"));
        assert!(html.contains("gvcs Provenance X-Ray"));

        // 2. POST /api/todos creates a task with inline #tags
        let (status, _, body) = dispatch_http_request(
            &mut store,
            "POST",
            "/api/todos",
            r#"{"title":"Launch Web UI #rust #web","priority_weight":5,"hours_overdue":4,"recurrence_days":7}"#,
        );
        assert_eq!(status, 201);
        assert!(body.contains("#rust"));

        // 3. GET /api/todos?tag=rust filters tasks and populates cache
        let (status, _, list_body) =
            dispatch_http_request(&mut store, "GET", "/api/todos?tag=rust", "");
        assert_eq!(status, 200);
        assert!(list_body.contains("Launch Web UI #rust #web"));

        // 4. POST /api/todos/1/complete marks task completed and spawns recurring task #2
        let (status, _, comp_body) =
            dispatch_http_request(&mut store, "POST", "/api/todos/1/complete", "");
        assert_eq!(status, 200);
        assert!(comp_body.contains("spawned_next"));

        // 5. POST /api/gdpr-erase exports archive and wipes storage + cache
        let (status, _, gdpr_body) =
            dispatch_http_request(&mut store, "POST", "/api/gdpr-erase", "");
        assert_eq!(status, 200);
        assert!(gdpr_body.contains("\"wiped_count\":2"));
    }
}
