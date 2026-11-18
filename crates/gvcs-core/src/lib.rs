//! `gvcs-core`: The N-Dimensional Feature Hypergraph & Provenance Engine.
//!
//! Implements the iterated Specification-as-Source + Graph Version Control System (`gvcs`)
//! architecture:
//!
//! 1. **Features as Composite Vectors**: Every `.feature`, `.cap`, `@marketplace/*` blueprint,
//!    and `@native` sealed kernel decomposes into basis projections, substrate references
//!    (e.g., `DBTable:todos`, `Viewport:TodoList`), ontology tags (`@touches(PII)`), and
//!    universal quantifiers (`For ANY feature that...`).
//! 2. **Stratified Quantifier Resolution ($L_0 \to L_3$)**: Prevents $2^N$ quantifier cascades
//!    by enforcing that stratum $L_k$ can only quantify over features at $< L_k$ (or explicit
//!    basis tags), computing deterministic **Intersection Cells ($I_{A \cap B \cap \dots}$)**.
//! 3. **Committed Fused Code + Bidirectional Provenance (`provenance.lock`)**: Solves the
//!    LLM-in-the-build-loop and regulatory auditability paradoxes by treating materialized code
//!    in `src/` like `package-lock.json`—committed, hermetic, and mapped span-by-span to
//!    Intersection Cells ($I_S$) via `// @gvcs-cell: I(...)` and content-addressed SHA-256 hashes.
//! 4. **Exception & Pareto Tradeoff Pins ($\mathcal{E}$)**: Resolves non-commutative collisions
//!    and CAP/PACELC tradeoffs (`scale.cap <25ms` vs `gdpr.cap 0ms cache revocation`) via
//!    contrastive business choices rather than cryptic logic errors.
//! 5. **The Tourniquet-to-Invariant Loop (`.incident`)**: Detects manual 3 AM code hotfixes in
//!    `src/` via AST hash drift against `provenance.lock`, allows them as TTL-bounded
//!    Tourniquet Pins, and enforces lifting them into permanent Negative Invariant tests.

pub mod engine;
pub mod model;
pub mod parser;
pub mod provenance;
pub mod solver;

pub use engine::*;
pub use model::*;
pub use parser::*;
pub use provenance::*;
pub use solver::*;
