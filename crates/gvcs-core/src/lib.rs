//! `gvcs-core`: Feature-Aware Version Control & Provenance Engine.
//!
//! Git tracks *when* a line of code changed. `gvcs` tracks *which features* own every
//! block of code—and catches when two features collide.
//!
//! Core concepts:
//! 1. **Features & Policies (`FeatureVector`)**: Plain-English `.feature` and `.cap` specs
//!    organized into a 4-Layer Priority Stack (`Core Feature` -> `Policy & Compliance` ->
//!    `UI & Platform` -> `Performance & Scale`).
//! 2. **Feature Overlaps (`IntersectionCell`, `I(A ∩ B)`)**: The exact spots where two or
//!    more features and global rules (`Rule: For ANY...`) meet on shared data or UI.
//! 3. **Product Decisions & Bug Guards (`ExceptionPin`, `IncidentRecord`)**: Durable TOML
//!    records that lock in how competing features combine and ensure 3 AM production
//!    hotfixes become permanent regression tests.
//! 4. **Linked Code Blocks (`CodeProvenanceSpan` & `provenance.lock`)**: Standard, fast
//!    source code in `src/` bookended by `// @gvcs-begin` and `// @gvcs-end`, locked via
//!    SHA-256 digests so `gvcs check` catches drift or regressions in milliseconds.

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
