#[allow(dead_code)]
#[path = "support/binary_records.rs"]
mod binary_records;
mod support;

use binary_records::{frame, sized};
use sdocx::{NativeShape, PageElement, ShapePaint, ShapePaintSource};
use support::{archive, object, page};

fn bounds() -> Vec<u8> {
    [0.0_f64, 0.0, 100.0, 50.0]
        .into_iter()
        .flat_map(f64::to_le_bytes)
        .collect()
}

fn shape_archive(version: u32, paint: &[u8], after_fill: &[u8]) -> Vec<u8> {
    shape_archive_with_outline(version, paint, after_fill, None)
}

fn shape_archive_with_outline(
    version: u32,
    paint: &[u8],
    after_fill: &[u8],
    outline: Option<&[u8]>,
) -> Vec<u8> {
    shape_archive_with_effect(version, 1, paint, after_fill, outline)
}

fn shape_archive_with_effect(
    version: u32,
    kind: u8,
    paint: &[u8],
    after_fill: &[u8],
    outline: Option<&[u8]>,
) -> Vec<u8> {
    let mut fixed = version.to_le_bytes().to_vec();
    fixed.extend(1_u16.to_le_bytes());
    fixed.push(b's');
    fixed.extend(0_i64.to_le_bytes());
    fixed.extend(bounds());
    fixed.extend([0; 5]);
    let mut base = frame(0, &[], &fixed, &[]);
    base[10] = 1;
    base.insert(11, 8);
    let size = base.len() as u32;
    base[..4].copy_from_slice(&size.to_le_bytes());
    base[6..10].copy_from_slice(&size.to_le_bytes());
    let style = [0_u32, 4, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .chain([0])
        .collect::<Vec<_>>();
    base.extend(match outline {
        Some(effect) => frame(6, &[4], &style, &sized(effect)),
        None => frame(6, &[], &style, &[]),
    });
    let mut geometry = 1_u32.to_le_bytes().to_vec();
    geometry.extend(bounds());
    geometry.extend(0_f32.to_le_bytes());
    geometry.extend([0; 5]);
    geometry.extend(bounds());
    let mut fill = sized(paint);
    fill.insert(4, kind);
    fill.extend(after_fill);
    base.extend(frame(7, &[32], &geometry, &fill));
    archive(&page(&[vec![object(7, &base, &[])]], 0, &[]))
}

fn parsed_shape(bytes: &[u8]) -> NativeShape {
    let document = sdocx::parse_bytes(bytes).unwrap();
    let PageElement::Shape(shape) = document.pages[0].elements().next().unwrap() else {
        panic!("expected shape")
    };
    shape.clone()
}

fn solid_color() -> Vec<u8> {
    let mut bytes = vec![1, 0];
    bytes.extend(0x80445566_u32.to_le_bytes());
    bytes.extend([0; 12]);
    bytes
}

#[test]
fn dormant_source_preserves_all_stops_bits_and_order_without_changing_solid_paint() {
    let mut effect = solid_color();
    effect[1] = 0x82;
    effect[6] = 255;
    effect[7..9].copy_from_slice(&u16::MAX.to_le_bytes());
    effect[9..13].copy_from_slice(&0x7fc12345_u32.to_le_bytes());
    effect[13..17].copy_from_slice(&0x80000000_u32.to_le_bytes());
    effect[17] = 255;
    let positions = [0x7f800000_u32, 0xff800000, 0x80000000, 0x7fc54321];
    for index in 0..255_u32 {
        effect.extend(index.to_le_bytes());
        effect.extend(positions[index as usize % positions.len()].to_le_bytes());
    }
    effect.extend([0xde, 0xad]);
    let archive = shape_archive(5500, &effect, &[]);
    let shape = parsed_shape(&archive);
    assert!(matches!(shape.fill, ShapePaint::Solid(0x80445566)));
    let Some(ShapePaintSource::Color(source)) = &shape.fill_source else {
        panic!("expected color source")
    };
    assert_eq!(source.property_flags, 0x82);
    assert!(source.gradient_rotatable());
    assert_eq!(source.gradient_type, 255);
    assert_eq!(source.linear_angle, u16::MAX);
    assert_eq!(
        source.position.map(|value| value.bits()),
        [0x7fc12345, 0x80000000]
    );
    assert_eq!(source.stops.len(), 255);
    for (index, stop) in source.stops.iter().enumerate() {
        assert_eq!(stop.argb, index as u32);
        assert_eq!(stop.position.bits(), positions[index % positions.len()]);
    }
    assert_eq!(source.trailing_data, [0xde, 0xad]);
    #[cfg(feature = "serde")]
    {
        let encoded = serde_json::to_string(&shape).unwrap();
        let restored: NativeShape = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored.fill_source, shape.fill_source);
    }
    #[cfg(feature = "render")]
    {
        let original = sdocx::parse_bytes(&archive).unwrap();
        let baseline = sdocx::parse_bytes(&shape_archive(5500, &solid_color(), &[])).unwrap();
        let options = Default::default();
        assert_eq!(
            sdocx::render_document_svg(&original, &options)[0].svg,
            sdocx::render_document_svg(&baseline, &options)[0].svg
        );
    }
}

#[test]
fn noncanonical_legacy_solid_projection_keeps_opaque_original_bytes() {
    let mut effect = solid_color();
    effect[0] = 2;
    effect.insert(2, 0);
    effect.extend([7, 8]);
    let shape = parsed_shape(&shape_archive(5500, &effect, &[]));
    assert!(matches!(shape.fill, ShapePaint::Solid(0x80445566)));
    assert!(matches!(shape.fill_source, Some(ShapePaintSource::Opaque { data }) if data == effect));
}

#[test]
fn truncated_stop_data_cannot_consume_bytes_after_the_sized_fill() {
    let mut effect = solid_color();
    effect[17] = 1;
    effect.extend([0; 7]);
    let bytes = shape_archive(5500, &effect, &[0; 16]);
    assert!(matches!(
        sdocx::parse_bytes(&bytes),
        Err(sdocx::Error::Format(_))
    ));
}

#[cfg(feature = "serde")]
#[test]
fn legacy_shape_json_can_omit_new_source_fields() {
    let shape = parsed_shape(&shape_archive(5500, &solid_color(), &[]));
    let mut json = serde_json::to_value(shape).unwrap();
    json.as_object_mut().unwrap().remove("fill_source");
    json["style"]
        .as_object_mut()
        .unwrap()
        .remove("paint_source");
    let restored: NativeShape = serde_json::from_value(json).unwrap();
    assert!(restored.fill_source.is_none());
    assert!(restored.style.paint_source.is_none());
    assert!(matches!(restored.fill, ShapePaint::Solid(0x80445566)));
}

#[test]
fn no_outline_source_normalizes_any_nonzero_rotatable_byte_without_losing_bits() {
    let mut outline = solid_color();
    outline[1] = 2;
    outline.insert(2, 2);
    outline[10..14].copy_from_slice(&0x7fc12345_u32.to_le_bytes());
    let shape = parsed_shape(&shape_archive_with_outline(
        5500,
        &solid_color(),
        &[],
        Some(&outline),
    ));
    assert!(matches!(shape.style.paint, ShapePaint::None));
    let Some(ShapePaintSource::Color(source)) = shape.style.paint_source else {
        panic!("expected outline color source")
    };
    assert_eq!(source.property_flags, 2);
    assert_eq!(source.outline_color_type, Some(2));
    assert!(source.gradient_rotatable());
    assert_eq!(source.position[0].bits(), 0x7fc12345);
}

#[test]
fn valid_and_short_patterns_keep_the_same_unsupported_render_projection() {
    let mut pattern = vec![0x80, 1, 0x55, 0xaa, 0, 0xff, 3, 4];
    pattern.extend(0x00112233_u32.to_le_bytes());
    pattern.extend(0x80445566_u32.to_le_bytes());
    for bytes in [&pattern[..], &pattern[..15]] {
        let archive = shape_archive_with_effect(5500, 3, bytes, &[], None);
        let parsed = sdocx::parse_bytes_detailed(&archive).unwrap();
        assert!(parsed.report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == sdocx::DiagnosticCode::UnsupportedShapeFeature
        }));
        let PageElement::Shape(shape) = parsed.document.pages[0].elements().next().unwrap() else {
            panic!("expected shape")
        };
        assert!(matches!(&shape.fill, ShapePaint::Unsupported { kind: 3, data } if data == bytes));
        if bytes.len() == 16 {
            let Some(ShapePaintSource::Pattern(source)) = &shape.fill_source else {
                panic!("expected pattern source")
            };
            assert_eq!(source.rows, pattern[..8]);
            assert_eq!(source.foreground_argb, 0x00112233);
            assert_eq!(source.background_argb, 0x80445566);
        } else {
            assert!(shape.fill_source.is_none());
        }
    }
}
