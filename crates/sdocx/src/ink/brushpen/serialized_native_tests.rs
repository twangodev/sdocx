//! Synthetic serialized controls: native getters and complete GLV10 geometry.

#[test]
fn independent_serialized_brushpen_native() {
    verify_capture(
        "independent",
        include_bytes!("../../../tests/assets/brushpen-independent/payload.bin"),
        include_bytes!("../../../tests/assets/brushpen-independent/geometry.bin"),
        include_str!("../../../../../tools/brushpen-independent/receipt.json"),
        include_bytes!("../../../../../tools/brushpen-independent/frame.bin"),
        include_bytes!("../../../tests/assets/brushpen-independent/frame.rgba.zlib"),
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
        include_bytes!("../../../../../tools/brushpen-third/frame.bin"),
        include_bytes!("../../../tests/assets/brushpen-third/frame.rgba.zlib"),
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

#[allow(clippy::too_many_arguments)]
fn verify_capture(
    name: &str,
    payload: &[u8],
    geometry: &[u8],
    receipt: &str,
    captured_frame: &[u8],
    compressed_rgba: &[u8],
    pressure: f32,
    delta: u16,
) {
    use sha2::{Digest, Sha256};
    use std::io::Read;
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
    let base_frame = include_bytes!("../../../../../conformance/brushpen-serialized/v10-frame.bin");
    assert_eq!(
        hash(&source),
        "28b0848cb3aa9dadc700bf425fd390281c8de0d222e46f7126d0bcbcd527b60f"
    );
    assert_eq!(
        hash(base_frame),
        "9b495ebc360c03d2b064e21bf97ce5a45d6f02290aa04410ba87316ae59874f3"
    );
    assert_eq!(&source[137..], base_frame);
    let frame = &mut source[137..];
    frame[20 + 16 + 203 * 4..20 + 16 + 204 * 4].copy_from_slice(&pressure.to_le_bytes());
    frame[20 + 16 + 4 * 40..20 + 16 + 4 * 40 + 2].copy_from_slice(&delta.to_le_bytes());
    assert_eq!(hash(frame), receipt["inputs"]["frame.bin"]);
    assert_eq!(frame, captured_frame, "{name} captured frame");
    assert_eq!(source, payload, "{name} source-derived serialized input");
    let mut rgba = Vec::new();
    flate2::read::ZlibDecoder::new(compressed_rgba)
        .read_to_end(&mut rgba)
        .unwrap();
    assert_eq!(hash(&rgba), receipt["frame_sha256"], "{name} native RGBA");
}
