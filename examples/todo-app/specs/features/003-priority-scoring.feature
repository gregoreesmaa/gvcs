@id: native:priority-decay
@stratum: L0
@native: src/native_priority.rs::compute_urgency_score
@tag: read_query
@substrate: DBTable:todos, Viewport:TodoList

Feature: Real-time Urgency Scoring Kernel (@native Code Escape Hatch)
  Uses a permanent human-authored Rust mathematical kernel for exponential deadline urgency decay
  rather than verbose natural-language prompt golf.

  Invariant: Urgency score is strictly monotonic with respect to priority weight and hours overdue.
