mod support;

use sdocx::{DiagnosticCode, NativeShape, PageElement};
use support::{archive, object, page};

fn frame(kind: i16, properties: u8, fixed: &[u8]) -> Vec<u8> {
    let size = u32::try_from(14 + fixed.len()).unwrap();
    let mut bytes = size.to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(size.to_le_bytes());
    bytes.extend([1, properties, 1, 0]);
    bytes.extend(fixed);
    bytes
}

fn numbers(values: &[f64]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn shape_archive(properties: u8, flip_enabled: bool, path: &[u8]) -> Vec<u8> {
    let bounds = numbers(&[10.0, 20.0, 210.0, 220.0]);
    let mut base = 5500_u32.to_le_bytes().to_vec();
    base.extend(2_u16.to_le_bytes());
    base.extend(b"sh");
    base.extend(1234_i64.to_le_bytes());
    base.extend(&bounds);
    base.extend([0; 5]);
    let mut payload = frame(0, 0x08 | (u8::from(flip_enabled) << 7), &base);
    let outline = [0_u32, 4, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .chain([0])
        .collect::<Vec<_>>();
    payload.extend(frame(6, 0, &outline));
    let mut geometry = 2_u32.to_le_bytes().to_vec();
    geometry.extend(&bounds);
    geometry.extend(30.0_f32.to_le_bytes());
    geometry.extend(u32::try_from(path.len()).unwrap().to_le_bytes());
    geometry.extend(path);
    geometry.push(0);
    geometry.extend(bounds);
    payload.extend(frame(7, properties, &geometry));
    archive(&page(&[vec![object(7, &payload, &[])]], 0, &[]))
}

fn shape(parsed: &sdocx::ParsedDocument) -> &NativeShape {
    let PageElement::Shape(shape) = parsed.document.pages[0].elements().next().unwrap() else {
        panic!("expected native shape");
    };
    shape
}

#[test]
fn orientation_flags_are_independent_of_flip_capability_and_text_editability() {
    for properties in 0..16 {
        for flip_enabled in [false, true] {
            let parsed =
                sdocx::parse_bytes_detailed(&shape_archive(properties, flip_enabled, &[])).unwrap();
            let shape = shape(&parsed);
            assert_eq!(shape.horizontal_flip, properties & 1 != 0);
            assert_eq!(shape.vertical_flip, properties & 2 != 0);
            assert_eq!(shape.text_editable, properties & 4 != 0);
            assert_eq!(shape.metadata.flip_enabled, flip_enabled);
            assert_eq!(shape.geometry_bbox, shape.drawn_bbox);
            assert_eq!(shape.drawn_bbox, shape.metadata.bbox);
            assert_eq!(shape.rotation_degrees, 30.0);
            let warnings = parsed
                .report
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnsupportedShapeFeature)
                .collect::<Vec<_>>();
            assert_eq!(warnings.len(), usize::from(properties & 8 != 0));
            if let [warning] = warnings.as_slice() {
                assert!(
                    warning
                        .message
                        .ends_with("unknown shape geometry properties")
                );
            }
        }
    }
}

#[test]
fn orientation_flags_retain_the_saved_path_coordinates() {
    let mut path = 3_u32.to_le_bytes().to_vec();
    path.push(1);
    path.extend(numbers(&[10.0, 20.0]));
    path.push(2);
    path.extend(numbers(&[190.0, 180.0]));
    path.push(6);
    for properties in 0..4 {
        let parsed = sdocx::parse_bytes_detailed(&shape_archive(properties, true, &path)).unwrap();
        assert_eq!(shape(&parsed).path_data, path);
        assert!(
            parsed
                .report
                .diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.code != DiagnosticCode::UnsupportedShapeFeature })
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn old_json_defaults_orientation_flags_and_explicit_flags_round_trip() {
    for properties in 0..4 {
        let parsed = sdocx::parse_bytes_detailed(&shape_archive(properties, false, &[])).unwrap();
        let json = serde_json::to_value(shape(&parsed)).unwrap();
        let restored: NativeShape = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(restored.horizontal_flip, properties & 1 != 0);
        assert_eq!(restored.vertical_flip, properties & 2 != 0);
        assert_eq!(serde_json::to_value(restored).unwrap(), json);

        let mut legacy = json;
        let fields = legacy.as_object_mut().unwrap();
        fields.remove("horizontal_flip");
        fields.remove("vertical_flip");
        let restored: NativeShape = serde_json::from_value(legacy).unwrap();
        assert!(!restored.horizontal_flip);
        assert!(!restored.vertical_flip);
    }
}
