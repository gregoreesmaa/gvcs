Native: src/native_priority.rs::compute_urgency_score

Feature: Real-time Urgency Scoring Kernel
  Tasks are ranked by a deterministic urgency score based on priority weight
  and how many hours overdue they are, so critical work floats to the top of the todo list.

  Invariant: Urgency score is strictly monotonic with respect to priority weight and hours overdue.
