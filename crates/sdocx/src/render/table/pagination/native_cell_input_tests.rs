use super::super::cell_text_bounds;
use super::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    apk_version: String,
    apk_sha256: String,
    allocation_fills: Vec<u8>,
    model_library_sha256: String,
    drawing_library_sha256: String,
    base_library_sha256: String,
    widget_library_sha256: String,
    text_library_sha256: String,
    layout_cell_address: String,
    object_layout_address: String,
    split_address: String,
    measurement_inputs: String,
    null_and_uncached_cell_delta_bits: u32,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    rows: usize,
    columns: usize,
    spans: Vec<[u32; 2]>,
    frames: Vec<[f32; 4]>,
    bands: Vec<Vec<[f32; 4]>>,
    measured_heights: Vec<f32>,
    calls: Vec<Call>,
}

#[derive(Deserialize)]
struct Call {
    slot: usize,
    native_dimensions: [f32; 2],
    text_dimensions: [i32; 2],
    rectangles: Vec<[f32; 4]>,
    height_delta_bits: u32,
}

fn rect([x_min, y_min, x_max, y_max]: [f32; 4]) -> BoundingBox {
    BoundingBox {
        x_min: f64::from(x_min),
        y_min: f64::from(y_min),
        x_max: f64::from(x_max),
        y_max: f64::from(y_max),
    }
}

fn bits(bounds: BoundingBox) -> [u32; 4] {
    [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max].map(|value| (value as f32).to_bits())
}

#[test]
fn native_cell_frames_bands_and_integer_text_dimensions_match() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-cell-inputs.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "0e936c9d767d9db6556d62e4298a2aa63435be41e64d8d31d093f93182249020"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.apk_version, "4.4.45.37");
    assert_eq!(
        capture.apk_sha256,
        "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
    );
    assert_eq!(capture.allocation_fills, [0, 165, 255]);
    assert_eq!(
        capture.model_library_sha256,
        "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
    );
    assert_eq!(
        capture.drawing_library_sha256,
        "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd"
    );
    assert_eq!(
        capture.base_library_sha256,
        "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
    );
    assert_eq!(
        capture.widget_library_sha256,
        "cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9"
    );
    assert_eq!(
        capture.text_library_sha256,
        "5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b"
    );
    assert_eq!(capture.layout_cell_address, "0xb06d4");
    assert_eq!(capture.object_layout_address, "0xd3b88");
    assert_eq!(capture.split_address, "0xae794");
    assert_eq!(
        capture.measurement_inputs,
        "supplied cached frames and measured heights; native table and object layout, text shaping and padding assignment intercepted"
    );
    assert_eq!(capture.null_and_uncached_cell_delta_bits, 0);
    assert_eq!(capture.cases.len(), 138);
    let mut calls = 0;
    for case in capture.cases {
        let count = case.rows * case.columns;
        assert_eq!(case.spans.len(), count);
        assert_eq!(case.frames.len(), count);
        assert_eq!(case.bands.len(), count);
        assert_eq!(case.measured_heights.len(), count);
        assert_eq!(case.calls.len(), count);
        for (slot, call) in case.calls.into_iter().enumerate() {
            calls += 1;
            assert_eq!(call.slot, slot);
            let frame = rect(case.frames[slot]);
            let dimensions = [
                native_sub(frame.x_max, frame.x_min).unwrap(),
                native_sub(frame.y_max, frame.y_min).unwrap(),
            ];
            assert_eq!(
                dimensions.map(|value| (value as f32).to_bits()),
                call.native_dimensions.map(f32::to_bits),
                "{}, slot {slot}",
                case.name
            );
            let bounds = cell_text_bounds(frame).unwrap();
            assert_eq!([bounds.x_min, bounds.y_min], [0.0, 0.0]);
            assert_eq!(
                [bounds.x_max, bounds.y_max],
                call.text_dimensions.map(f64::from),
                "{}, slot {slot}",
                case.name
            );
            let local_bands = BandList::new(case.bands[slot].iter().copied().map(rect).collect())
                .unwrap()
                .for_row(frame.y_min)
                .unwrap();
            assert_eq!(
                local_bands
                    .rectangles
                    .into_iter()
                    .map(bits)
                    .collect::<Vec<_>>(),
                call.rectangles
                    .into_iter()
                    .map(|r| r.map(f32::to_bits))
                    .collect::<Vec<_>>(),
                "{}, slot {slot}",
                case.name
            );
            let delta = native_sub(f64::from(case.measured_heights[slot]), dimensions[1]).unwrap();
            assert_eq!(
                (delta as f32).to_bits(),
                call.height_delta_bits,
                "{}, slot {slot}",
                case.name
            );
        }
    }
    assert_eq!(calls, 850);
}
