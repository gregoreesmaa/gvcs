use anyhow::Result;
use gvcs_core::*;
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;

#[derive(Serialize)]
struct UiPayload {
    project_root: String,
    graph_hash: String,
    is_synced: bool,
    features: Vec<FeatureVector>,
    spans: Vec<SpanWithSource>,
    decisions: Vec<ExceptionPin>,
    incidents: Vec<IncidentRecord>,
}

#[derive(Serialize)]
struct SpanWithSource {
    span: CodeProvenanceSpan,
    human_features: String,
    code_snippet: String,
}

/// Launches the universal `gvcs ui` interactive Feature Map & Code Provenance Inspector
/// in a local web server (`http://127.0.0.1:4400`).
///
/// Works for *any* `gvcs` project (CLI, backend, iOS, or Web) by reading `specs/`,
/// `provenance.lock`, and `src/` directly—zero coupling to the target application's UI.
pub fn serve_gvcs_ui(root: &std::path::Path, addr: &str) -> Result<()> {
    let listener = TcpListener::bind(addr)?;
    println!("================================================================================");
    println!("🔬 gvcs ui — Universal Feature Map & Provenance Explorer");
    println!("   Project:   {}", root.display());
    println!("   Listening: http://{}", addr);
    println!("================================================================================");

    for mut stream in listener.incoming().flatten() {
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).unwrap_or(0);
        if n == 0 {
            continue;
        }
        let req = String::from_utf8_lossy(&buf[..n]);
        let first_line = req.lines().next().unwrap_or("GET / HTTP/1.1");
        let path = first_line.split_whitespace().nth(1).unwrap_or("/");

        let (status, content_type, body) = match path {
            "/" | "/index.html" => (200, "text/html; charset=utf-8", GVCS_UI_HTML.to_string()),
            "/api/graph" => match build_ui_payload(root) {
                Ok(json) => (200, "application/json", json),
                Err(e) => (500, "application/json", format!("{{\"error\":\"{}\"}}", e)),
            },
            _ => (404, "application/json", "{\"error\":\"Not Found\"}".to_string()),
        };

        let resp = format!(
            "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            content_type,
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.flush();
    }
    Ok(())
}

/// Builds the JSON payload for `gvcs ui` by loading the workspace and reading each
/// linked code block's source lines from disk.
pub fn build_ui_payload(root: &std::path::Path) -> Result<String> {
    let ws = GvcsWorkspace::load(root)?;
    let hash = ws.compute_graph_state_hash();
    let is_synced = ws
        .existing_lock
        .as_ref()
        .map(|l| l.graph_state_hash == hash)
        .unwrap_or(false);

    let mut spans_with_source = Vec::new();
    for s in &ws.current_spans {
        let full_path = ws.root.join(&s.file_path);
        let snippet = if let Ok(raw) = fs::read_to_string(&full_path) {
            let lines: Vec<&str> = raw.lines().collect();
            let start = s.start_line.saturating_sub(1);
            let end = s.end_line.min(lines.len());
            if start < end {
                lines[start..end].join("\n")
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        spans_with_source.push(SpanWithSource {
            human_features: IntersectionCell::format_human_list(&s.cell_coordinate),
            span: s.clone(),
            code_snippet: snippet,
        });
    }

    let payload = UiPayload {
        project_root: ws.root.display().to_string(),
        graph_hash: hash,
        is_synced,
        features: ws.features.values().cloned().collect(),
        spans: spans_with_source,
        decisions: ws.exceptions.values().cloned().collect(),
        incidents: ws.incidents.values().cloned().collect(),
    };

    Ok(serde_json::to_string(&payload)?)
}

const GVCS_UI_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>gvcs ui — Universal Feature Map & Provenance Explorer</title>
  <style>
    :root {
      --bg: #0b0f19;
      --panel: #111827;
      --panel-hover: #1f2937;
      --border: #374151;
      --text: #f9fafb;
      --muted: #9ca3af;
      --accent: #38bdf8;
      --green: #34d399;
      --amber: #fbbf24;
      --red: #fb7185;
      --purple: #c084fc;
    }
    * { box-sizing: border-box; font-family: system-ui, -apple-system, sans-serif; }
    body { margin: 0; background: var(--bg); color: var(--text); }
    .topbar { display: flex; justify-content: space-between; align-items: center; padding: 16px 28px; background: var(--panel); border-bottom: 1px solid var(--border); }
    .topbar h1 { margin: 0; font-size: 1.25rem; display: flex; align-items: center; gap: 10px; }
    .pill { font-size: 0.75rem; padding: 3px 10px; border-radius: 999px; font-weight: 700; background: rgba(56,189,248,0.15); color: var(--accent); }
    .layout { display: grid; grid-template-columns: 360px 1fr; min-height: calc(100vh - 61px); }
    .sidebar { border-right: 1px solid var(--border); padding: 20px; background: #0d1322; }
    .main { padding: 24px 28px; overflow-y: auto; }
    h2 { font-size: 0.9rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); margin: 0 0 12px 0; }
    .feat-card { padding: 12px 14px; border-radius: 10px; border: 1px solid var(--border); background: var(--panel); margin-bottom: 10px; cursor: pointer; transition: all 0.15s; }
    .feat-card:hover { background: var(--panel-hover); }
    .feat-card.selected { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); background: rgba(56,189,248,0.08); }
    .feat-id { font-family: monospace; font-weight: 700; color: var(--accent); font-size: 0.88rem; }
    .feat-title { font-size: 0.85rem; margin-top: 4px; color: var(--text); }
    .whatif-box { background: rgba(192,132,252,0.1); border: 1px solid rgba(192,132,252,0.35); border-radius: 10px; padding: 14px 18px; margin-bottom: 20px; }
    .span-card { background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 16px 18px; margin-bottom: 16px; }
    .span-header { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px; margin-bottom: 10px; }
    .span-loc { font-family: monospace; color: var(--green); font-weight: 700; font-size: 0.9rem; }
    .badge-row { display: flex; gap: 8px; flex-wrap: wrap; margin-bottom: 10px; }
    .chip { font-size: 0.74rem; font-family: monospace; padding: 3px 8px; border-radius: 6px; }
    .chip-feat { background: rgba(56,189,248,0.15); color: var(--accent); }
    .chip-pin { background: rgba(251,191,36,0.15); color: var(--amber); }
    .chip-guard { background: rgba(251,113,133,0.16); color: var(--red); }
    .chip-delete { background: rgba(251,113,133,0.22); color: var(--red); font-weight: 700; }
    .chip-simplify { background: rgba(251,191,36,0.22); color: var(--amber); font-weight: 700; }
    pre { margin: 0; padding: 12px; background: #060911; border-radius: 8px; border: 1px solid #1e293b; overflow-x: auto; font-size: 0.78rem; color: #e2e8f0; line-height: 1.45; }
    .grid-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; margin-top: 24px; }
    .btn-clear { background: transparent; border: 1px solid var(--border); color: var(--text); padding: 6px 12px; border-radius: 6px; cursor: pointer; font-size: 0.8rem; }
  </style>
</head>
<body>
  <div class="topbar">
    <h1>
      <span>🔬 gvcs ui</span>
      <span class="pill">Universal Feature Map & Provenance Explorer</span>
    </h1>
    <div id="lockStatus" style="font-family:monospace;font-size:0.82rem;color:var(--muted)">Loading provenance.lock...</div>
  </div>

  <div class="layout">
    <div class="sidebar">
      <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:12px">
        <h2 style="margin:0">1. Features & Policies</h2>
        <button class="btn-clear" onclick="selectFeature(null)">Show All</button>
      </div>
      <div style="font-size:0.78rem;color:var(--muted);margin-bottom:12px">Click any feature to slice the codebase & preview removal blast radius:</div>
      <div id="featureList"></div>
    </div>

    <div class="main">
      <div id="whatIfBanner" style="display:none" class="whatif-box"></div>

      <h2 id="spansHeading">2. Linked Code Blocks in src/</h2>
      <div id="spanList"></div>

      <div class="grid-2">
        <div>
          <h2>3. Product Decisions</h2>
          <div id="decisionList"></div>
        </div>
        <div>
          <h2>4. Permanent Bug Guards</h2>
          <div id="incidentList"></div>
        </div>
      </div>
    </div>
  </div>

  <script>
    let graphData = null;
    let selectedFeature = null;

    async function init() {
      const res = await fetch('/api/graph');
      graphData = await res.json();
      document.getElementById('lockStatus').innerHTML =
        `Project: <strong>${escapeHtml(graphData.project_root)}</strong> · Lock: <code>${graphData.graph_hash.slice(0,8)}</code> ${graphData.is_synced ? '<span style="color:var(--green)">✓ synced</span>' : '<span style="color:var(--amber)">⚠ unsynced</span>'}`;
      render();
    }

    function selectFeature(id) {
      selectedFeature = (selectedFeature === id) ? null : id;
      render();
    }

    function render() {
      const featContainer = document.getElementById('featureList');
      featContainer.innerHTML = graphData.features.map(f => {
        const isSel = selectedFeature === f.id;
        return `
          <div class="feat-card ${isSel ? 'selected' : ''}" onclick="selectFeature('${f.id}')">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <span class="feat-id">${escapeHtml(f.id)}</span>
              <span style="font-size:0.72rem;color:var(--muted)">${escapeHtml(f.spec_path)}</span>
            </div>
            <div class="feat-title">${escapeHtml(f.title)}</div>
          </div>
        `;
      }).join('');

      const matchingSpans = selectedFeature
        ? graphData.spans.filter(s => s.span.features.includes(selectedFeature))
        : graphData.spans;

      const banner = document.getElementById('whatIfBanner');
      if (selectedFeature) {
        const untouched = graphData.spans.length - matchingSpans.length;
        const pct = Math.round((untouched * 100) / Math.max(1, graphData.spans.length));
        banner.style.display = 'block';
        banner.innerHTML = `
          <div style="font-weight:700;color:var(--purple);margin-bottom:4px">
            🔮 What-If Removal Preview: <code>gvcs what-if --remove ${escapeHtml(selectedFeature)}</code>
          </div>
          <div style="font-size:0.85rem">
            Affects <strong>${matchingSpans.length} of ${graphData.spans.length}</strong> code block(s) in <code>src/</code>.
            <strong style="color:var(--green)">✓ ${untouched} of ${graphData.spans.length} blocks (${pct}% of codebase) remain 100% untouched.</strong>
          </div>
        `;
        document.getElementById('spansHeading').textContent =
          `2. Code Blocks Implementing '${selectedFeature}' (${matchingSpans.length} of ${graphData.spans.length})`;
      } else {
        banner.style.display = 'none';
        document.getElementById('spansHeading').textContent =
          `2. All Linked Code Blocks in src/ (${graphData.spans.length})`;
      }

      document.getElementById('spanList').innerHTML = matchingSpans.map(item => {
        const s = item.span;
        let actionBadge = '';
        if (selectedFeature) {
          const remaining = s.features.filter(f => f !== selectedFeature);
          actionBadge = remaining.length === 0
            ? `<span class="chip chip-delete">[DELETE BLOCK]</span>`
            : `<span class="chip chip-simplify">[SIMPLIFY -> retains ${escapeHtml(remaining.join(' + '))}]</span>`;
        }
        const featChips = s.features.map(f => `<span class="chip chip-feat">${escapeHtml(f)}</span>`).join('');
        const pinChips = (s.active_pins || []).map(p => `<span class="chip chip-pin">✓ Decision: ${escapeHtml(p)}</span>`).join('');
        const guardChips = (s.active_incidents || []).map(g => `<span class="chip chip-guard">🛡 Guard: ${escapeHtml(g)}</span>`).join('');
        return `
          <div class="span-card">
            <div class="span-header">
              <span class="span-loc">${escapeHtml(s.file_path)}:${s.start_line}..${s.end_line} — <code>${escapeHtml(s.symbol)}</code></span>
              ${actionBadge}
            </div>
            <div class="badge-row">${featChips}${pinChips}${guardChips}</div>
            <pre>${escapeHtml(item.code_snippet)}</pre>
          </div>
        `;
      }).join('');

      document.getElementById('decisionList').innerHTML = graphData.decisions.map(d => `
        <div class="feat-card" style="cursor:default">
          <div class="feat-id" style="color:var(--amber)">✓ ${escapeHtml(d.id)}</div>
          <div style="font-size:0.75rem;color:var(--muted);margin:3px 0">${escapeHtml((d.features || []).join(' × '))}</div>
          <div class="feat-title">${escapeHtml(d.chosen_option)}</div>
        </div>
      `).join('');

      document.getElementById('incidentList').innerHTML = graphData.incidents.map(inc => `
        <div class="feat-card" style="cursor:default">
          <div class="feat-id" style="color:var(--red)">🛡 ${escapeHtml(inc.id)}</div>
          <div style="font-size:0.75rem;color:var(--muted);margin:3px 0">${escapeHtml((inc.affected_features || []).join(' × '))}</div>
          <div class="feat-title">${escapeHtml(inc.never_again || inc.title)}</div>
        </div>
      `).join('');
    }

    function escapeHtml(s) {
      return String(s || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
    }

    init();
  </script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_gvcs_ui_payload_serializes_workspace_and_source_snippets() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let todo_root = manifest_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples")
            .join("todo-app");
        let json = build_ui_payload(&todo_root).expect("Should build universal gvcs ui payload");
        assert!(json.contains("TodoStore::gdpr_erase_and_export"));
        assert!(json.contains("PARETO-001-cache-vs-gdpr"));
        assert!(json.contains("pub fn gdpr_erase_and_export"));
    }
}
