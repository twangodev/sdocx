//! Synthetic serialized name derivative: parser channels versus native Model getters.
use sha2::{Digest, Sha256};

#[test]
fn synthetic_marker_parser_matches_native_model_channels() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-synthetic-serialized-drawing.json"
    ))
    .unwrap();
    let bytes = include_bytes!("assets/legacy-marker/synthetic-serialized.sdocx");

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    use std::io::Read;
    let mut note = Vec::new();
    archive
        .by_name("note.note")
        .unwrap()
        .read_to_end(&mut note)
        .unwrap();
    let table_size = u32::from_le_bytes(note[765..769].try_into().unwrap()) as usize;
    assert_eq!(
        format!("{:x}", Sha256::digest(&note[769..769 + table_size])),
        receipt["table_sha256"].as_str().unwrap()
    );
    let mut page = Vec::new();
    archive
        .by_name("a2ab6718-b54d-11f1-8a68-83a46e5f73c2.page")
        .unwrap()
        .read_to_end(&mut page)
        .unwrap();
    let frame_size = u32::from_le_bytes(page[481..485].try_into().unwrap()) as usize;
    assert_eq!(
        format!("{:x}", Sha256::digest(&page[481..481 + frame_size])),
        receipt["frame_sha256"].as_str().unwrap()
    );
    let doc = sdocx::parse_bytes(bytes).unwrap();
    assert_eq!(doc.pages.len(), 1);
    assert_eq!(doc.pages[0].objects.len(), 1);
    assert_eq!(doc.pages[0].strokes().count(), 1);
    let stroke = doc
        .pages
        .iter()
        .flat_map(|page| page.strokes())
        .find(|stroke| {
            stroke
                .rendering
                .as_ref()
                .and_then(|r| r.pen_name.as_deref())
                == Some("com.samsung.android.sdk.pen.pen.preload.Marker")
        })
        .unwrap();
    let rendering = stroke.rendering.as_ref().unwrap();
    assert_eq!(stroke.points.len(), 204);
    assert_eq!(rendering.advanced_settings, None);
    assert_eq!(rendering.style.color_argb, None);
    assert_eq!(rendering.tool_type_raw, 2);
    assert_eq!(stroke.pen_width.to_bits(), 0x4111_de6a);
    let points: Vec<u8> = stroke
        .points
        .iter()
        .flat_map(|p| [(p.x as f32).to_le_bytes(), (p.y as f32).to_le_bytes()])
        .flatten()
        .collect();
    let floats = |values: &[f64]| {
        values
            .iter()
            .flat_map(|value| (*value as f32).to_le_bytes())
            .collect::<Vec<u8>>()
    };
    let timestamps: Vec<u8> = stroke
        .timestamps
        .iter()
        .flat_map(|value| i32::try_from(*value).unwrap().to_le_bytes())
        .collect();
    for (name, data) in [
        ("points", points),
        ("pressure", floats(&stroke.pressures)),
        ("timestamps", timestamps),
        ("tilt", floats(&stroke.tilts)),
        ("orientation", floats(&stroke.orientations)),
    ] {
        assert_eq!(
            format!("{:x}", Sha256::digest(data)),
            receipt["native_and_parser_arrays"][name]["native_and_rust_sha256"]
                .as_str()
                .unwrap(),
            "{name}"
        );
    }
}
