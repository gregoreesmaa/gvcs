# `gvcs` — Graph Version Control System & Specification-as-Source Control Plane

> **Replacing 1D chronological code amnesia with an $N$-Dimensional Feature Hypergraph (`I(A ∩ B)`), Executable Pareto Tradeoff Pins ($\mathcal{E}$), and the Tourniquet-to-Invariant CI Gate.**

---

## 1. The Core Problem: Why "AI Agents + Normal Git" Hits a Wall at Month 6

In traditional software engineering, human-written source code is the primary artifact, while specifications rot in forgotten PRDs or chat threads. With frontier AI coding agents, teams can generate 100,000 lines of code in weeks—**which accelerates architectural entropy by 10x**:

1. **Intent Amnesia:** Git tracks *which lines of text changed at timestamp $T$*, completely discarding *which lines exist solely because of the intersection of Feature A and Compliance Policy B*.
2. **Regression Roulette:** Adding Feature #20 silently overwrites a tradeoff agreed upon for Feature #4 six months ago, or leaves untraceable glue code behind when Feature #12 is deprecated.
3. **Hotfix Rot:** 3 AM production bugfixes are committed directly to code and never back-ported into executable specifications.

---

## 2. The Evolution of the `gvcs` Architecture (3 Rounds of Adversarial Design)

This repository implements the battle-tested architecture synthesized across three rounds of multi-agent systems critique:

### Why Naive "Pure Specification-as-Source" Fails (And How `gvcs` Fixes It)
| Naive Idea | Why It Collapses under Scrutiny | The `gvcs` Architectural Solution |
| :--- | :--- | :--- |
| **"Compile features into isolated runtime Nano-Lambdas & ECS facts"** | Collapses under $O(F \times E)$ distributed read-joins, 2PC lock contention, 120fps optimistic UI physics, and historical event-schema evolution. | **Temporal/Logical Isolation over Normal Fused Code:** Keep normal, fast, single-process Rust/TypeScript/SQL code at runtime; isolate features in the **Provenance Hypergraph (`provenance.lock`)**. |
| **"Hardcode 5 fixed feature dimensions"** | Fails under *Multi-Dimensional Separation of Concerns (MDSOC)*: features like `Subscriptions` are composite vectors that both add vertical flows *and* horizontally gate other features. | **$N$-Dimensional Feature Vectors & Quantifiers:** Every feature decomposes into basis projections, physical substrates (`DBTable:todos`, `Viewport:TodoList`), and Universal Quantifiers (`For ANY feature touching PII...`). |
| **"Hide generated code in `.gitignore` / `.build/` and run an LLM optimizer at build time"** | Violates build hermeticity (slow, non-reproducible CI builds), creates a "test-unfused, ship-fused" `-O3` miscompilation hazard, and fails SOC2/PCI/FDA regulatory audits. | **Treat Fused Code Like `package-lock.json`:** `gvcs sync` synthesizes and verifies Joint Multi-Cell Fusions *once*, committing the deterministic `src/` code alongside `provenance.lock`. `gvcs build` (`cargo build`) is 100% LLM-free, hermetic, and millisecond-fast. |
| **"Use AST `WRAP_FUNCTION` decorators for intersection cells"** | Stacking wrappers around database queries causes $N+1$ round-trips and breaks pagination. | **Joint Multi-Cell Fusion Spans:** Overlapping features touching the same symbol/query fuse into a single optimal function anchored to the multi-feature cell coordinate `I(A ∩ B ∩ C)`. |
| **"Expose raw Datalog & CAP/PACELC errors to Product Owners"** | Turns Product Owners into Datalog ontologists, causing "Cookie Banner Fatigue." | **Auto-Stratification ($L_{\text{base}}, L_0 \dots L_3$) & Contrastive Tradeoff Cards ($\mathcal{E}$):** `gvcs` auto-infers tags and presents conflicts as concrete business options with latency/cost simulations. |
| **"Ban humans from ever writing raw code"** | Fails on 200k-line brownfield repos (`L_base`) and low-level math/shader kernels where prose is 10x harder than code. | **First-Class `@native` Sealed Cells & 72h `.incident` Tourniquets:** Engineers can write permanent `@native` kernels with signature envelopes, and 3 AM code hotfixes are caught by SHA-256 span drift and lifted into permanent **Negative Invariant Tests**. |

---

## 3. Repository Structure

```text
gvcs/
├── Cargo.toml                        # Rust workspace (gvcs-core, gvcs-cli, examples/todo-app)
├── crates/
│   ├── gvcs-core/                    # Hypergraph solver, parser, stratification & provenance scanner
│   └── gvcs-cli/                     # The `gvcs` command-line tool (status, sync, check, slice, what-if, inspect)
└── examples/
    └── todo-app/                     # Reference Todo App governed by gvcs
        ├── specs/
        │   ├── features/
        │   │   ├── 001-todo-crud.feature          # L0 Domain: Core task creation & completion
        │   │   ├── 002-recurring-tasks.feature    # L0 Domain: Recurring task roll-forward [CANARY(25%)]
        │   │   └── 003-priority-scoring.feature   # L0 Domain: @native Rust urgency decay kernel
        │   ├── capabilities/
        │   │   ├── google-login.cap               # L1 Policy: @marketplace/google-oauth-identity@2.4.0
        │   │   ├── gdpr-privacy.cap               # L1 Policy: @marketplace/eu-gdpr-privacy-safe@3.1.0
        │   │   ├── cli-ux-shell.cap               # L2 Projection: Viewport:TodoList badge & overflow rules
        │   │   └── fast-cache-sla.cap             # L3 Envelope: Sub-25ms write-through memory cache
        │   ├── exceptions/
        │   │   ├── pareto-001-cache-vs-gdpr.pareto.toml     # Resolves L3 Cache vs L1 GDPR 0ms erasure conflict
        │   │   └── exc-002-crud-vs-recurring.exception.toml # Resolves shared DBTable:todos mutation ordering
        │   └── incidents/
        │       └── inc-001-double-complete.incident.toml    # 3 AM retry bug lifted to Negative Invariant test
        ├── src/
        │   ├── lib.rs                             # Fused Rust implementation with @gvcs-begin/end cell spans
        │   ├── native_priority.rs                 # Sealed @native urgency scoring kernel
        │   └── main.rs                            # Runnable Todo App demo
        └── provenance.lock                        # Cryptographic Merkle lock mapping I(A ∩ B) <-> src/ spans
```

---

## 4. Quickstart & CLI Walkthrough

### 1. Run the Complete Test & Invariant Suite
```bash
cargo test --workspace
```

### 2. Inspect the $N$-Dimensional Feature Hypergraph (`gvcs status`)
```bash
cargo run -p gvcs-cli -- status
```
Displays all 7 stratified Feature Vectors ($L_0 \to L_3$), their lifecycle gates (`GA`, `CANARY(25%)`), all 27 resolved Intersection Cells ($I_{A \cap B}$), and the Pareto Tradeoff Pins ($\mathcal{E}$).

### 3. Verify Adherence, Pareto Pins & 3 AM Hotfix Drift (`gvcs check`)
```bash
cargo run -p gvcs-cli -- check
```
Verifies that:
1. Every cross-feature collision and PACELC tradeoff has a signed-off Exception/Pareto Pin ($\mathcal{E}$).
2. Every `@gvcs-begin` code span in `src/` matches its locked SHA-256 digest in `provenance.lock` (or carries an active 72-hour `.incident` Tourniquet Pin).
3. All historical `.incident` records have been lifted into permanent Negative Invariant tests.

### 4. Slice the Codebase by Any Feature or Horizontal Policy (`gvcs slice`)
```bash
cargo run -p gvcs-cli -- slice gdpr-privacy
```
Instantly locates every Intersection Cell (`I(cap:fast-cache-sla ∩ cap:gdpr-privacy ∩ feat:todo-crud)`) and the exact lines of Rust code in `src/lib.rs` (lines 207..236) and verification tests that exist because of `gdpr-privacy`.

### 5. Simulate Feature Removal / Deprecation Blast Radius (`gvcs what-if`)
```bash
cargo run -p gvcs-cli -- what-if --remove recurring-tasks
```
Shows the exact 7 Intersection Cells and 1 fused code span (`TodoStore::complete_todo`) affected by removing `recurring-tasks`, while proving via SHA-256 lock that the other 7 code spans across the repository remain 100% untouched.

### 6. Run the Materialized Todo App Example
```bash
cargo run -p todo-app
```
