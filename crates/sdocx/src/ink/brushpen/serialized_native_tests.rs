//! Synthetic serialized controls: native getters and complete GLV10 geometry.

#[test]
fn independent_serialized_brushpen_native() {
    verify_capture(
        "independent",
        include_bytes!("../../../tests/assets/brushpen-independent/payload.bin"),
        include_bytes!("../../../tests/assets/brushpen-independent/geometry.bin"),
        include_str!("../../../../../tools/brushpen-independent/receipt.json"),
        0.45,
        96,
    );
    let mut stroke = crate::decode::decode_stroke(
        include_bytes!("../../../tests/assets/brushpen-independent/payload.bin"),
        &crate::ParseLimits::default(),
    )
    .unwrap();
    let rendering = stroke.rendering.as_mut().unwrap();
    rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.BrushPen".into());
    rendering.advanced_settings = Some("10;".into());
    let records = crate::sample_brushpen_glv10_host(&stroke, 1920, 1080).unwrap();
    let bytes: Vec<_> = records
        .iter()
        .flat_map(|r| r.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    assert_eq!(
        bytes,
        include_bytes!("../../../tests/assets/brushpen-independent/geometry.bin")
    );
}
#[test]
fn third_serialized_brushpen_native() {
    verify_capture(
        "third",
        include_bytes!("../../../tests/assets/brushpen-third/payload.bin"),
        include_bytes!("../../../tests/assets/brushpen-third/geometry.bin"),
        include_str!("../../../../../tools/brushpen-third/receipt.json"),
        0.70,
        48,
    );
    let mut stroke = crate::decode::decode_stroke(
        include_bytes!("../../../tests/assets/brushpen-third/payload.bin"),
        &crate::ParseLimits::default(),
    )
    .unwrap();
    let point_bytes: Vec<u8> = stroke
        .points
        .iter()
        .flat_map(|p| {
            [(p.x as f32).to_le_bytes(), (p.y as f32).to_le_bytes()]
                .into_iter()
                .flatten()
        })
        .collect();
    assert_eq!(
        point_bytes,
        include_bytes!("../../../tests/assets/brushpen-third/GetPoint.bin")
    );
    for (actual, native) in [
        (
            &stroke.pressures,
            &include_bytes!("../../../tests/assets/brushpen-third/GetPressure.bin")[..],
        ),
        (
            &stroke.tilts,
            &include_bytes!("../../../tests/assets/brushpen-third/GetTilt.bin")[..],
        ),
        (
            &stroke.orientations,
            &include_bytes!("../../../tests/assets/brushpen-third/GetOrientation.bin")[..],
        ),
    ] {
        let bytes: Vec<u8> = actual
            .iter()
            .flat_map(|v| (*v as f32).to_le_bytes())
            .collect();
        assert_eq!(bytes, native);
    }
    let timestamps: Vec<u8> = stroke
        .timestamps
        .iter()
        .flat_map(|v| (*v as i32).to_le_bytes())
        .collect();
    assert_eq!(
        timestamps,
        include_bytes!("../../../tests/assets/brushpen-third/GetTimeStamp.bin")
    );
    let rendering = stroke.rendering.as_mut().unwrap();
    rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.BrushPen".into());
    rendering.advanced_settings = Some("10;".into());
    let records = crate::sample_brushpen_glv10_host(&stroke, 1920, 1080).unwrap();
    let bytes: Vec<_> = records
        .iter()
        .flat_map(|r| r.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    assert_eq!(
        bytes,
        include_bytes!("../../../tests/assets/brushpen-third/geometry.bin")
    );
}

fn verify_capture(
    name: &str,
    payload: &[u8],
    geometry: &[u8],
    receipt: &str,
    pressure: f32,
    delta: u16,
) {
    use sha2::{Digest, Sha256};
    let receipt: serde_json::Value = serde_json::from_str(receipt).unwrap();
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    assert_eq!(
        hash(payload),
        receipt["inputs"]["payload.bin"],
        "{name} payload"
    );
    assert_eq!(
        hash(geometry),
        receipt["callback"]["vector_sha256"],
        "{name} geometry"
    );
    assert_eq!(
        geometry.len(),
        receipt["callback"]["vector_bytes"].as_u64().unwrap() as usize
    );
    let mut source =
        include_bytes!("../../../../../conformance/brushpen-serialized/v10-payload.bin").to_vec();
    let frame = &mut source[137..];
    frame[20 + 16 + 203 * 4..20 + 16 + 204 * 4].copy_from_slice(&pressure.to_le_bytes());
    frame[20 + 16 + 4 * 40..20 + 16 + 4 * 40 + 2].copy_from_slice(&delta.to_le_bytes());
    assert_eq!(hash(frame), receipt["inputs"]["frame.bin"]);
    assert_eq!(source, payload, "{name} source-derived serialized input");
}
