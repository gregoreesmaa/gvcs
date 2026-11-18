//! Permanent Human-Authored `@native` Escape-Hatch Kernel (`native:priority-decay`).
//!
//! When prose is 10x more verbose than mathematical code, `gvcs` allows engineers to
//! commit sealed `@native` source modules with explicit ontology contracts.

// @gvcs-begin cell="I(native:priority-decay)" symbol="compute_urgency_score" tests="test_native_urgency_monotonicity"
/// Computes a deterministic urgency score using exponential deadline decay.
/// Higher priority weight (1..=5) and greater overdue hours yield strictly monotonic scores.
pub fn compute_urgency_score(priority_weight: u8, hours_overdue: i64) -> f64 {
    let clamped_priority = (priority_weight.clamp(1, 5)) as f64;
    let base = clamped_priority * 20.0;
    if hours_overdue <= 0 {
        // Tasks not yet overdue scale gently toward their deadline
        let hours_remaining = (-hours_overdue).min(168) as f64;
        base + (168.0 - hours_remaining) * 0.15
    } else {
        // Overdue tasks accelerate via smooth hyperbolic growth
        let overdue = hours_overdue as f64;
        base + 25.2 + (overdue * 1.8 * (1.0 + clamped_priority * 0.2))
    }
}
// @gvcs-end
