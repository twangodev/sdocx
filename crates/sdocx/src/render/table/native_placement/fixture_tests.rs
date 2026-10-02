use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

#[derive(Deserialize)]
struct SetterCapture {
    cases: Vec<SetterCase>,
}

#[derive(Deserialize)]
struct SetterCase {
    name: String,
    method: String,
    cell_bbox: [f32; 4],
    content_bbox: [f32; 4],
    requested_bbox: [f32; 4],
    cell_bbox_after: [f32; 4],
    content_bbox_after: [f32; 4],
}

#[test]
fn cell_model_state_matches_all_native_setter_geometry_controls() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-cell-model-bounds.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "898739ecada7c4a2b66f71a4893e712fbd484eb715d96dfdd51cbc882a2210fd"
    );
    let capture: SetterCapture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 60);
    let mut data_only = 0;
    for case in capture.cases {
        data_only += usize::from(case.method == "data-only");
        let state = NativeCellModelState::new(case.cell_bbox, case.content_bbox).unwrap();
        let updated = state.with_saved_rect(case.requested_bbox).unwrap();
        assert_eq!(
            updated.saved_rect().map(f32::to_bits),
            case.cell_bbox_after.map(f32::to_bits),
            "{} saved rectangle",
            case.name
        );
        assert_eq!(
            updated.content_rect().map(f32::to_bits),
            case.content_bbox_after.map(f32::to_bits),
            "{} content rectangle",
            case.name
        );
    }
    assert_eq!(data_only, 15);
}

#[test]
fn feedback_rejects_nonfinite_and_staged_overflow_without_changing_source() {
    let original = NativeCellModelState::new([0.0, 0.0, 10.0, 10.0], [1.0, 2.0, 3.0, 4.0]).unwrap();
    assert!(NativeCellModelState::new([f32::NAN; 4], [0.0; 4]).is_err());
    assert!(
        original
            .apply_feedback([f32::INFINITY, 0.0], [0.0; 4])
            .is_err()
    );
    assert!(original.apply_feedback([0.0; 2], [f32::NAN; 4]).is_err());
    assert!(
        original
            .apply_feedback([f32::MAX; 2], [f32::MAX; 4])
            .is_err()
    );
    assert_eq!(original.saved_rect(), [0.0, 0.0, 10.0, 10.0]);
    assert_eq!(original.content_rect(), [1.0, 2.0, 3.0, 4.0]);
}
