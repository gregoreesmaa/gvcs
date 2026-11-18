use gvcs_core::*;
use std::path::Path;

#[test]
fn test_gvcs_workspace_adherence_and_zero_drift() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = GvcsWorkspace::load(manifest_dir).expect("Failed to load todo-app gvcs workspace");

    // 1. Verify all 8 feature vectors are loaded
    assert_eq!(ws.features.len(), 8);

    // 2. Verify all Pareto/Exception conflicts are resolved
    for conflict in &ws.conflicts {
        assert!(
            conflict.is_resolved,
            "Unresolved conflict in todo-app: {}",
            conflict.summary
        );
    }

    // 3. Verify zero untracked code drift between src/ and provenance.lock
    let drifts = ws.check_drifts();
    assert!(
        drifts.is_empty(),
        "Detected untracked code drift against provenance.lock: {:?}",
        drifts
    );
}
