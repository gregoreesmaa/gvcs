use gvcs_core::*;
use std::path::Path;

#[test]
fn test_gvcs_workspace_adherence_and_zero_drift() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = GvcsWorkspace::load(manifest_dir).expect("Failed to load todo-app gvcs workspace");

    // 1. Verify all 7 feature vectors are loaded (after removing google-login & cli-ux-shell and adding web-ui)
    assert_eq!(ws.features.len(), 7);
    assert!(ws.features.contains_key("cap:web-ui"));
    assert!(!ws.features.contains_key("mkt:google-login"));
    assert!(!ws.features.contains_key("cap:cli-ux-shell"));

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
