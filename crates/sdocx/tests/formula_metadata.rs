mod support;

#[cfg(feature = "render")]
use std::io::{Cursor, Write};

use sdocx::{
    DiagnosticCode, Error, FormulaLabelRelationKind, FormulaMetadata, MathAngleType, ObjectType,
    ParseLimits, ParseOptions,
};
use support::{archive, object, page};

fn frame(kind: i16, properties: &[u8], fields: &[u8], fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 12 + properties.len() + fields.len() + fixed.len();
    let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend((offset as u32).to_le_bytes());
    bytes.push(properties.len() as u8);
    bytes.extend(properties);
    bytes.push(fields.len() as u8);
    bytes.extend(fields);
    bytes.extend(fixed);
    bytes.extend(flexible);
    bytes
}

fn bbox(values: [f64; 4]) -> Vec<u8> {
    values.into_iter().flat_map(f64::to_le_bytes).collect()
}

fn base() -> Vec<u8> {
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend(36_u16.to_le_bytes());
    fixed.extend(b"00000000-0000-0000-0000-000000000011");
    fixed.extend(1234_i64.to_le_bytes());
    fixed.extend(bbox([10.0, 20.0, 300.0, 400.0]));
    fixed.extend([0; 5]);
    frame(0, &[1 << 3], &[1], &fixed, &15.5_f32.to_le_bytes())
}

fn formula(mask: u16, flexible: &[u8]) -> Vec<u8> {
    [base(), frame(11, &[3], &mask.to_le_bytes(), &[], flexible)].concat()
}

fn latex_list(values: &[&str]) -> Vec<u8> {
    let mut bytes = (values.len() as u32).to_le_bytes().to_vec();
    for value in values {
        bytes.extend((value.len() as u16).to_le_bytes());
        bytes.extend(value.as_bytes());
    }
    bytes
}

fn answer(value: &str) -> Vec<u8> {
    let units: Vec<_> = value.encode_utf16().collect();
    let mut bytes = (units.len() as u16).to_le_bytes().to_vec();
    bytes.extend(units.into_iter().flat_map(u16::to_le_bytes));
    bytes
}

fn stroke(color: u32) -> Vec<u8> {
    let mut fixed = 1_u16.to_le_bytes().to_vec();
    fixed.extend(12.5_f64.to_le_bytes());
    fixed.extend(25.25_f64.to_le_bytes());
    fixed.extend(0.75_f32.to_le_bytes());
    fixed.extend(123_i32.to_le_bytes());
    fixed.extend([1, 0]);
    let style = [color.to_le_bytes(), 2.5_f32.to_le_bytes()].concat();
    [
        base(),
        frame(1, &[0], &[12], &fixed, &style),
        frame(99, &[], &[], &[0xaa], &[]),
    ]
    .concat()
}

fn stroke_list(color: u32) -> Vec<u8> {
    let data = stroke(color);
    [
        1_u32.to_le_bytes().to_vec(),
        (data.len() as u32).to_le_bytes().to_vec(),
        data,
    ]
    .concat()
}

fn label_graphs() -> Vec<u8> {
    let mut bytes = 1_u32.to_le_bytes().to_vec();
    bytes.extend(2_u32.to_le_bytes());
    for (text, indices) in [("𝑥", vec![0, u32::MAX]), ("+", vec![1])] {
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(text.as_bytes());
        bytes.extend(bbox([-1.0, -2.0, 3.0, 4.0]));
        bytes.extend((indices.len() as u32).to_le_bytes());
        bytes.extend(indices.into_iter().flat_map(u32::to_le_bytes));
    }
    bytes.extend(1_u32.to_le_bytes());
    for value in [0, 1, u32::MAX, 5, u32::MAX] {
        bytes.extend(value.to_le_bytes());
    }
    bytes
}

fn fields() -> Vec<(u16, Vec<u8>)> {
    vec![
        (0, latex_list(&["x+😀", "y"])),
        (1, bbox([1.0, 2.0, 3.0, 4.0])),
        (
            3,
            [-1_i32, 2, 30, 40]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect(),
        ),
        (2, (-1_i32).to_le_bytes().to_vec()),
        (4, latex_list(&["2"])),
        (5, 1_u32.to_le_bytes().to_vec()),
        (6, 18.5_f32.to_le_bytes().to_vec()),
        (7, stroke_list(0xff123456)),
        (8, stroke_list(0xffabcdef)),
        (9, answer("答😀")),
        (10, 0xffaabbcc_u32.to_le_bytes().to_vec()),
        (11, bbox([5.0, 6.0, 7.0, 8.0])),
        (12, bbox([9.0, 10.0, 11.0, 12.0])),
        (13, u32::MAX.to_le_bytes().to_vec()),
        (14, label_graphs()),
        (15, latex_list(&["x=1", "y=2"])),
    ]
}

fn all_fields() -> Vec<u8> {
    formula(
        u16::MAX,
        &fields()
            .into_iter()
            .flat_map(|(_, data)| data)
            .collect::<Vec<_>>(),
    )
}

#[cfg(feature = "render")]
fn utf16(value: &str) -> Vec<u8> {
    let units: Vec<_> = value.encode_utf16().collect();
    [
        (units.len() as u16).to_le_bytes().as_slice(),
        &units
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    ]
    .concat()
}

#[cfg(feature = "render")]
fn note_with_pen_name() -> Vec<u8> {
    note_with_pen_id(12)
}

#[cfg(feature = "render")]
fn note_with_pen_id(pen_name_id: u32) -> Vec<u8> {
    let mut bytes = vec![0; 4];
    bytes.extend([1, 0, 2, 0, 4]); // one-byte property mask; field 10
    bytes.extend(5500_u32.to_le_bytes());
    bytes.extend(utf16("note"));
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend([0; 16]);
    for value in [1080_u32, 1527, 0, 0, 4000] {
        bytes.extend(value.to_le_bytes());
    }
    let mut common = 5500_u32.to_le_bytes().to_vec();
    common.extend([1, 0, b't']);
    common.extend([0; 45]);
    let note_frame = |kind: i16, fixed: &[u8]| {
        let size = (12 + fixed.len()) as u32;
        [
            size.to_le_bytes().as_slice(),
            kind.to_le_bytes().as_slice(),
            size.to_le_bytes().as_slice(),
            &[0, 0],
            fixed,
        ]
        .concat()
    };
    let text = [
        note_frame(0, &common),
        note_frame(6, &[]),
        note_frame(7, &[]),
    ]
    .concat();
    for _ in 0..2 {
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(&text);
    }
    let flex = bytes.len() as u32;
    bytes[..4].copy_from_slice(&flex.to_le_bytes());
    let mut table = 1_u16.to_le_bytes().to_vec();
    table.extend(pen_name_id.to_le_bytes());
    table.extend(utf16("com.samsung.android.sdk.pen.pen.preload.Marker"));
    bytes.extend((table.len() as u32).to_le_bytes());
    bytes.extend(table);
    bytes.extend([0xee; 32]);
    bytes
}

#[cfg(feature = "render")]
fn archive_with(page_bytes: &[u8], note: Option<&[u8]>, media: Option<&[u8]>) -> Vec<u8> {
    archive_with_asset(
        page_bytes,
        note,
        media,
        media.map(|_| include_bytes!("assets/crop-quadrants.png").as_slice()),
    )
}

#[cfg(feature = "render")]
fn archive_with_asset(
    page_bytes: &[u8],
    note: Option<&[u8]>,
    media: Option<&[u8]>,
    image: Option<&[u8]>,
) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        Some(("page.page", page_bytes)),
        note.map(|data| ("note.note", data)),
        media.map(|data| ("media/mediaInfo.dat", data)),
    ]
    .into_iter()
    .flatten()
    {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    if let Some(image) = image {
        writer
            .start_file(
                "media/latex@fixture.png",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(image).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn distinct_stroke(color: u32, x: f64, y: f64, visible: bool) -> Vec<u8> {
    // Native saved Drawing admits a stationary tap as two source events.
    let mut fixed = 2_u16.to_le_bytes().to_vec();
    for _ in 0..2 {
        fixed.extend(x.to_le_bytes());
        fixed.extend(y.to_le_bytes());
    }
    fixed.extend([0.75_f32.to_le_bytes(); 2].concat());
    fixed.extend([123_i32.to_le_bytes(), 124_i32.to_le_bytes()].concat());
    fixed.extend([1, 0]);
    let style = [
        color.to_le_bytes(),
        2.5_f32.to_le_bytes(),
        12_i32.to_le_bytes(),
    ]
    .concat();
    let mut common = base();
    common[11] = u8::from(visible) << 3;
    [common, frame(1, &[1 << 3], &[140], &fixed, &style)].concat()
}

fn formula_with_strokes(image_id: i32, parent_visible: bool) -> Vec<u8> {
    formula_with_strokes_and_result_rect(image_id, parent_visible, None)
}

fn formula_with_strokes_and_result_rect(
    image_id: i32,
    parent_visible: bool,
    result_rect: Option<[f64; 4]>,
) -> Vec<u8> {
    let source = distinct_stroke(0xff123456, 111.0, 222.0, true);
    let hidden = distinct_stroke(0xff00ff00, 200.0, 250.0, false);
    let answer_stroke = distinct_stroke(0xffabcdef, 333.0, 444.0, true);
    let list = |strokes: &[Vec<u8>]| {
        let mut bytes = (strokes.len() as u32).to_le_bytes().to_vec();
        for stroke in strokes {
            bytes.extend((stroke.len() as u32).to_le_bytes());
            bytes.extend(stroke);
        }
        bytes
    };
    let mut parent = base();
    // Deliberately unrelated Formula box; its child coordinates are native absolute coordinates.
    let replacement = bbox([5.0, 7.0, 25.0, 30.0]);
    let base_bbox_offset = 14 + 4 + 2 + 36 + 8;
    parent[base_bbox_offset..base_bbox_offset + 32].copy_from_slice(&replacement);
    parent[11] = u8::from(parent_visible) << 3;
    let mut flex = Vec::new();
    if let Some(rect) = result_rect {
        flex.extend(bbox(rect));
    }
    flex.extend(image_id.to_le_bytes());
    flex.extend(list(&[source, hidden]));
    flex.extend(list(&[answer_stroke]));
    let (properties, fields) = if result_rect.is_some() {
        (&[3][..], &[0x86, 1][..])
    } else {
        (&[3][..], &[0x84, 1][..])
    };
    [parent, frame(11, properties, fields, &[], &flex)].concat()
}

#[cfg(feature = "render")]
fn bound_manifest_entry(id: u32, name: &str) -> Vec<u8> {
    let mut record = id.to_le_bytes().to_vec();
    record.extend(utf16(name));
    record.extend([0, 0]);
    record.extend(1_u16.to_le_bytes());
    record.extend(0_i64.to_le_bytes());
    record.push(0);
    let mut bytes = 3002_u32.to_le_bytes().to_vec();
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend((record.len() as u32).to_le_bytes());
    bytes.extend(record);
    bytes.extend(b"EOFX");
    bytes
}

#[test]
fn formula_inspection_decodes_native_field_order_strokes_and_label_graphs() {
    let payload = all_fields();
    let bytes = page(&[vec![object(11, &payload, &[])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&archive(&bytes)).unwrap();
    let stored = &parsed.stored_pages[0].page.layers.layers[0].objects[0];
    let value = stored.formula_metadata(&bytes).unwrap();
    assert!(value.has_trigonometry_calculation);
    assert!(value.plottable);
    assert_eq!(value.base.uuid, "00000000-0000-0000-0000-000000000011");
    assert_eq!(value.base.modified_time_raw, 1234);
    assert_eq!(value.base.rotation_degrees, Some(15.5));
    assert_eq!(value.latex, ["x+😀", "y"]);
    assert_eq!(value.latex_result_rect.unwrap().x_min, 1.0);
    assert_eq!(value.nine_patch_rect, Some([-1, 2, 30, 40]));
    assert_eq!(value.latex_image_media_id, Some(-1));
    assert_eq!(value.latex_result, ["2"]);
    assert_eq!(value.angle_type, Some(MathAngleType::Radian));
    assert_eq!(value.font_size, Some(18.5));
    for (strokes, color) in [
        (&value.strokes, 0xff123456),
        (&value.answer_strokes, 0xffabcdef),
    ] {
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].object_data, stroke(color));
        assert_eq!(strokes[0].base.modified_time_raw, 1234);
        let ink = &strokes[0].stroke;
        assert_eq!(ink.points, [sdocx::Point { x: 12.5, y: 25.25 }]);
        assert_eq!(ink.pressures, [0.75]);
        assert_eq!(ink.timestamps, [123]);
        assert_eq!(ink.pen_width, 2.5);
        assert_eq!(ink.color.unwrap().r, (color >> 16) as u8);
    }
    assert_eq!(value.answer.as_deref(), Some("答😀"));
    assert_eq!(value.answer_stroke_color, Some(0xffaabbcc));
    assert_eq!(value.relative_original_formula_rect.unwrap().x_min, 5.0);
    assert_eq!(value.relative_original_answer_rect.unwrap().y_max, 12.0);
    assert_eq!(value.expression_type_raw, Some(u32::MAX));
    assert_eq!(value.substitution_latex, ["x=1", "y=2"]);
    assert_eq!(value.label_graphs.len(), 1);
    let graph = &value.label_graphs[0];
    assert_eq!(graph.labels.len(), 2);
    assert_eq!(graph.labels[0].text, "𝑥");
    assert_eq!(graph.labels[0].bbox.x_min, -1.0);
    assert_eq!(graph.labels[0].stroke_indices, [0, u32::MAX]);
    assert_eq!(graph.labels[1].text, "+");
    assert_eq!(graph.labels[1].stroke_indices, [1]);
    assert_eq!(graph.relations.len(), 1);
    assert_eq!(graph.relations[0].from_label, 0);
    assert_eq!(graph.relations[0].to_label, 1);
    assert_eq!(graph.relations[0].kind_raw, u32::MAX);
    assert_eq!(graph.start_label, 5);
    assert_eq!(graph.end_label, u32::MAX);
    assert_eq!(value.property_mask, [3]);
    assert_eq!(value.field_mask, [255, 255]);
    assert!(value.fixed_trailing_data.is_empty());
    assert!(value.flexible_trailing_data.is_empty());
    assert!(value.trailing_data.is_empty());
    assert!(parsed.document.pages[0].elements().next().is_none());
    assert_eq!(parsed.document.pages[0].strokes().count(), 2);
    assert!(
        !parsed
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UnsupportedObjectType)
    );
    #[cfg(feature = "render")]
    {
        let rendered = sdocx::render_document_svg(&parsed.document, &Default::default());
        assert_eq!(rendered.len(), 1);
        // The inspected one-point child strokes are rejected by saved Drawing.
        assert_eq!(rendered[0].svg.matches("<circle").count(), 0);
    }
}

#[cfg(feature = "render")]
#[test]
fn absent_formula_image_renders_source_and_answer_lists_in_native_order() {
    let payload = formula_with_strokes(-1, true);
    let page_bytes = page(&[vec![object(11, &payload, &[])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&archive_with(
        &page_bytes,
        Some(&note_with_pen_name()),
        None,
    ))
    .unwrap();
    let strokes: Vec<_> = parsed.document.pages[0].strokes().collect();
    assert_eq!(strokes.len(), 3);
    assert!(strokes.iter().all(|stroke| {
        stroke
            .rendering
            .as_ref()
            .is_some_and(|value| value.style.pen_name_id == Some(12))
    }));
    assert!(strokes.iter().all(|stroke| {
        stroke
            .rendering
            .as_ref()
            .and_then(|value| value.pen_name.as_deref())
            == Some("com.samsung.android.sdk.pen.pen.preload.Marker")
    }));
    let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
    assert!(svg.contains("x1=\"111"), "{svg}");
    assert!(svg.contains("y1=\"222"), "{svg}");
    assert!(svg.contains("x1=\"333"), "{svg}");
    assert!(svg.contains("y1=\"444"), "{svg}");
    assert!(svg.contains("#123456"), "{svg}");
    assert!(svg.contains("#abcdef"), "{svg}");
    assert_eq!(svg.matches("<line").count(), 3, "{svg}");
    assert!(svg.contains("x1=\"200"), "hidden child was omitted: {svg}");
    assert!(svg.contains("y1=\"250"), "hidden child was omitted: {svg}");
    assert!(svg.contains("#00ff00"), "hidden child was omitted: {svg}");
    #[cfg(feature = "pdf")]
    {
        let options = sdocx::PdfOptions::new(std::sync::Arc::new(Default::default()));
        let bytes =
            sdocx::render_document_pdf(&parsed.document, &Default::default(), &options).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let content =
            lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
                .unwrap();
        assert!(
            content
                .operations
                .iter()
                .any(|operation| { matches!(operation.operator.as_str(), "l" | "c" | "v" | "y") }),
            "Formula ink was not exported as PDF vector paths: {:?}",
            content.operations
        );
        assert!(!pdf.objects.values().any(|object| {
            object.as_stream().is_ok_and(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
            })
        }));
    }
}

#[test]
fn hidden_formula_parent_hides_nested_fallback_strokes() {
    let payload = formula_with_strokes(-1, false);
    assert!(!FormulaMetadata::parse_bytes(&payload).unwrap().base.visible);
    let page_bytes = page(&[vec![object(11, &payload, &[])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&archive(&page_bytes)).unwrap();
    assert_eq!(parsed.document.pages[0].strokes().count(), 0);
    assert!(parsed.document.pages[0].elements().next().is_none());
}

#[test]
fn page_stroke_budget_aggregates_formula_lists_and_ordinary_strokes() {
    let first = object(11, &formula_with_strokes(-1, true), &[]);
    let second = object(11, &formula_with_strokes(-1, true), &[]);
    let ordinary = object(1, &stroke(0xff000000), &[]);
    let bytes = archive(&page(&[vec![first, second, ordinary]], 0, &[]));
    let options = ParseOptions {
        limits: ParseLimits {
            max_strokes_per_page: 4,
            ..ParseLimits::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&bytes, &options),
        Err(Error::LimitExceeded {
            resource: "strokes per page",
            limit: 4,
            actual: 7,
        })
    ));
}

#[cfg(feature = "render")]
#[test]
fn bound_formula_media_suppresses_fallback_ink_and_keeps_unsupported_diagnostic() {
    let payload = formula_with_strokes(77, true);
    assert_eq!(
        FormulaMetadata::parse_bytes(&payload)
            .unwrap()
            .latex_image_media_id,
        Some(77)
    );
    let page_bytes = page(&[vec![object(11, &payload, &[])]], 0, &[]);
    let manifest = bound_manifest_entry(77, "latex@fixture.png");
    let parsed = sdocx::parse_bytes_detailed(&archive_with(
        &page_bytes,
        Some(&note_with_pen_name()),
        Some(&manifest),
    ))
    .unwrap();
    assert_eq!(parsed.document.pages[0].strokes().count(), 0);
    assert!(parsed.report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::UnsupportedObjectType
            && diagnostic.message.contains("Formula")
            && diagnostic.message.contains("image route")
    }));
    let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
    assert!(
        !svg.contains("<circle"),
        "formula strokes rendered despite bound media"
    );
    assert!(
        !svg.contains("<image"),
        "unsupported formula raster fabricated"
    );
    #[cfg(feature = "pdf")]
    {
        let options = sdocx::PdfOptions::new(std::sync::Arc::new(Default::default()));
        let bytes =
            sdocx::render_document_pdf(&parsed.document, &Default::default(), &options).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert!(!pdf.objects.values().any(|object| {
            object.as_stream().is_ok_and(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
            })
        }));
    }
}

#[cfg(feature = "render")]
#[test]
fn decoded_formula_png_uses_native_drawn_origin_and_saved_result_rect() {
    let payload = formula_with_strokes_and_result_rect(77, true, Some([1.0, 2.0, 5.0, 6.0]));
    let page_bytes = page(&[vec![object(11, &payload, &[])]], 0, &[]);
    let manifest = bound_manifest_entry(77, "latex@fixture.png");
    let parsed = sdocx::parse_bytes_detailed(&archive_with(
        &page_bytes,
        Some(&note_with_pen_name()),
        Some(&manifest),
    ))
    .unwrap();

    assert_eq!(parsed.document.pages[0].strokes().count(), 0);
    let images: Vec<_> = parsed.document.pages[0]
        .elements()
        .filter_map(|element| match element {
            sdocx::PageElement::FormulaImage(image) => Some(image),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 1);
    assert_eq!(
        sdocx::PageElement::FormulaImage(images[0].clone()).object_type(),
        ObjectType::Formula
    );
    assert_eq!(images[0].media_id, Some(77));
    assert!(images[0].media_index.is_some());
    let actual = [
        images[0].bbox.x_min,
        images[0].bbox.y_min,
        images[0].bbox.x_max,
        images[0].bbox.y_max,
    ];
    let expected = [
        -42.502_349_853_515_625,
        -17.839_996_337_890_625,
        -38.502_349_853_515_625,
        -13.839_996_337_890_625,
    ];
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 0.000_01,
            "{actual} != {expected}"
        );
    }

    let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
    assert!(svg.contains("<image"), "{svg}");
    assert!(
        !svg.contains("<circle"),
        "Formula ink duplicated its bitmap: {svg}"
    );
    #[cfg(feature = "pdf")]
    {
        let options = sdocx::PdfOptions::new(std::sync::Arc::new(Default::default()));
        let bytes =
            sdocx::render_document_pdf(&parsed.document, &Default::default(), &options).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert!(pdf.objects.values().any(|object| {
            object.as_stream().is_ok_and(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
            })
        }));
    }
}

#[cfg(feature = "render")]
#[test]
fn corrupt_bound_formula_png_falls_back_to_both_ink_lists() {
    let payload = formula_with_strokes_and_result_rect(77, true, Some([1.0, 2.0, 5.0, 6.0]));
    let page_bytes = page(&[vec![object(11, &payload, &[])]], 0, &[]);
    let manifest = bound_manifest_entry(77, "latex@fixture.png");
    let parsed = sdocx::parse_bytes_detailed(&archive_with_asset(
        &page_bytes,
        Some(&note_with_pen_name()),
        Some(&manifest),
        Some(b"not a decodable PNG"),
    ))
    .unwrap();

    let strokes: Vec<_> = parsed.document.pages[0].strokes().collect();
    assert_eq!(strokes.len(), 3);
    let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
    assert_eq!(svg.matches("<line").count(), 3, "{svg}");
    assert!(
        !svg.contains("<image"),
        "corrupt Formula image was emitted: {svg}"
    );
    #[cfg(feature = "pdf")]
    {
        let options = sdocx::PdfOptions::new(std::sync::Arc::new(Default::default()));
        let bytes =
            sdocx::render_document_pdf(&parsed.document, &Default::default(), &options).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert!(!pdf.objects.values().any(|object| {
            object.as_stream().is_ok_and(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
            })
        }));
    }
}

#[cfg(feature = "render")]
#[test]
fn nonfinite_formula_image_placement_is_not_emitted() {
    let payload = formula_with_strokes_and_result_rect(
        77,
        true,
        Some([f64::MAX / 2.0, 2.0, f64::MAX / 2.0, 6.0]),
    );
    let page_bytes = page(&[vec![object(11, &payload, &[])]], 0, &[]);
    let manifest = bound_manifest_entry(77, "latex@fixture.png");
    let parsed = sdocx::parse_bytes_detailed(&archive_with(
        &page_bytes,
        Some(&note_with_pen_name()),
        Some(&manifest),
    ))
    .unwrap();

    assert!(
        parsed.document.pages[0]
            .elements()
            .all(|element| !matches!(element, sdocx::PageElement::FormulaImage(_)))
    );
    assert!(parsed.report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::UnsupportedObjectType
            && diagnostic.message.contains("Formula")
            && diagnostic.message.contains("origin cannot be derived")
    }));
}

#[cfg(feature = "render")]
#[test]
fn complete_string_table_distinguishes_missing_pen_name_from_unparsed_table() {
    let payload = formula_with_strokes_and_result_rect(77, true, Some([1.0, 2.0, 5.0, 6.0]));
    let page_bytes = page(&[vec![object(11, &payload, &[])]], 0, &[]);
    let manifest = bound_manifest_entry(77, "latex@fixture.png");

    let known_missing = sdocx::parse_bytes_detailed(&archive_with(
        &page_bytes,
        Some(&note_with_pen_id(13)),
        Some(&manifest),
    ))
    .unwrap();
    let image = known_missing.document.pages[0]
        .elements()
        .find_map(|element| match element {
            sdocx::PageElement::FormulaImage(image) => Some(image),
            _ => None,
        })
        .expect("a complete table with an absent ID uses the native null-name category");
    assert_eq!(
        sdocx::PageElement::FormulaImage(image.clone()).object_type(),
        ObjectType::Formula
    );
    let expected = [
        -40.963_775_634_765_625,
        -16.301_406_860_351_562,
        -36.963_775_634_765_625,
        -12.301_406_860_351_562,
    ];
    for (actual, expected) in [
        image.bbox.x_min,
        image.bbox.y_min,
        image.bbox.x_max,
        image.bbox.y_max,
    ]
    .into_iter()
    .zip(expected)
    {
        assert!(
            (actual - expected).abs() < 0.000_01,
            "{actual} != {expected}"
        );
    }

    let incomplete_options = ParseOptions {
        limits: ParseLimits {
            max_note_metadata_entries: 0,
            ..ParseLimits::default()
        },
        ..ParseOptions::default()
    };
    let incomplete = sdocx::parse_bytes_detailed_with_options(
        &archive_with(&page_bytes, Some(&note_with_pen_name()), Some(&manifest)),
        &incomplete_options,
    )
    .unwrap();
    assert!(
        incomplete.document.pages[0]
            .elements()
            .all(|element| !matches!(element, sdocx::PageElement::FormulaImage(_)))
    );
    assert!(incomplete.report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::UnsupportedObjectType
            && diagnostic.message.contains("origin cannot be derived")
    }));
}

#[test]
fn every_formula_field_is_optional_and_truncated_fields_cannot_borrow_later_frames() {
    for (bit, data) in fields() {
        FormulaMetadata::parse_bytes(&formula(1 << bit, &data)).unwrap();
        for length in 0..data.len() {
            let payload = [
                formula(1 << bit, &data[..length]),
                frame(99, &[], &[], &[0; 1024], &[]),
            ]
            .concat();
            assert!(
                FormulaMetadata::parse_bytes(&payload).is_err(),
                "bit {bit}, length {length}"
            );
        }
    }
}

#[test]
fn absent_formula_fields_unknown_values_and_frame_extensions_remain_distinct() {
    let mut payload = [base(), frame(11, &[0], &[0, 0], &[], &[])].concat();
    let start = base().len();
    payload[start + 6..start + 10].copy_from_slice(&0_u32.to_le_bytes());
    let value = FormulaMetadata::parse_bytes(&payload).unwrap();
    assert!(!value.has_trigonometry_calculation);
    assert!(!value.plottable);
    assert!(value.latex.is_empty());
    assert!(value.latex_result_rect.is_none());
    assert!(value.nine_patch_rect.is_none());
    assert!(value.latex_image_media_id.is_none());
    assert!(value.latex_result.is_empty());
    assert!(value.angle_type.is_none());
    assert!(value.font_size.is_none());
    assert!(value.strokes.is_empty());
    assert!(value.answer_strokes.is_empty());
    assert!(value.answer.is_none());
    assert!(value.answer_stroke_color.is_none());
    assert!(value.relative_original_formula_rect.is_none());
    assert!(value.relative_original_answer_rect.is_none());
    assert!(value.expression_type_raw.is_none());
    assert!(value.label_graphs.is_empty());
    assert!(value.substitution_latex.is_empty());
    for (raw, expected) in [
        (0_u32, MathAngleType::Degree),
        (1, MathAngleType::Radian),
        (2, MathAngleType::All),
        (99, MathAngleType::Other(99)),
    ] {
        let payload = [
            base(),
            frame(
                11,
                &[2, 128],
                &[32, 0, 1],
                &[0xa1],
                &[raw.to_le_bytes().as_slice(), &[0xb1, 0xb2]].concat(),
            ),
            vec![0xc1],
        ]
        .concat();
        let value = FormulaMetadata::parse_bytes(&payload).unwrap();
        assert!(!value.has_trigonometry_calculation);
        assert!(value.plottable);
        assert_eq!(value.angle_type, Some(expected));
        assert_eq!(value.property_mask, [2, 128]);
        assert_eq!(value.field_mask, [32, 0, 1]);
        assert_eq!(value.fixed_trailing_data, [0xa1]);
        assert_eq!(value.flexible_trailing_data, [0xb1, 0xb2]);
        assert_eq!(value.trailing_data, [0xc1]);
    }
}

#[test]
fn formula_answers_use_the_entire_unsigned_utf16_length_range() {
    for text in [String::new(), "😀".into(), "x".repeat(u16::MAX as usize)] {
        let payload = formula(1 << 9, &answer(&text));
        let limit = text.encode_utf16().count();
        let limits = ParseLimits {
            max_text_characters: limit,
            ..Default::default()
        };
        let value = FormulaMetadata::parse_bytes_with_limits(&payload, &limits).unwrap();
        assert_eq!(value.answer.as_deref(), Some(text.as_str()));
        if limit > 0 {
            let limits = ParseLimits {
                max_text_characters: limit - 1,
                ..limits
            };
            assert!(
                matches!(FormulaMetadata::parse_bytes_with_limits(&payload, &limits), Err(Error::LimitExceeded { resource: "text characters", actual, .. }) if actual == limit as u64)
            );
        }
    }
    assert!(FormulaMetadata::parse_bytes(&formula(1 << 9, &[1, 0, 0, 0xd8])).is_err());
    assert!(FormulaMetadata::parse_bytes(&formula(1 << 9, &[255, 255])).is_err());
}

#[test]
fn formula_budgets_include_nested_labels_indices_relations_and_both_stroke_lists() {
    let payload = all_fields();
    let limits = ParseLimits {
        max_objects_per_page: 14,
        max_strokes_per_page: 2,
        max_points_per_stroke: 1,
        ..Default::default()
    };
    FormulaMetadata::parse_bytes_with_limits(&payload, &limits).unwrap();
    let restricted = ParseLimits {
        max_objects_per_page: 13,
        ..limits
    };
    assert!(matches!(
        FormulaMetadata::parse_bytes_with_limits(&payload, &restricted),
        Err(Error::LimitExceeded {
            resource: "math entries",
            limit: 13,
            actual: 14
        })
    ));
    let restricted = ParseLimits {
        max_strokes_per_page: 1,
        ..limits
    };
    assert!(matches!(
        FormulaMetadata::parse_bytes_with_limits(&payload, &restricted),
        Err(Error::LimitExceeded {
            resource: "formula strokes",
            limit: 1,
            actual: 2
        })
    ));
    let restricted = ParseLimits {
        max_points_per_stroke: 0,
        ..limits
    };
    assert!(matches!(
        FormulaMetadata::parse_bytes_with_limits(&payload, &restricted),
        Err(Error::LimitExceeded {
            resource: "points per stroke",
            limit: 0,
            actual: 1
        })
    ));
    for limit in 0..7 {
        let limits = ParseLimits {
            max_objects_per_page: limit,
            ..Default::default()
        };
        assert!(matches!(
            FormulaMetadata::parse_bytes_with_limits(&formula(1 << 14, &label_graphs()), &limits),
            Err(Error::LimitExceeded {
                resource: "math entries",
                ..
            })
        ));
    }
}

#[test]
fn formula_counts_and_nested_stroke_sizes_are_bounded_before_allocation() {
    for bit in [0, 4, 7, 8, 14, 15] {
        let payload = formula(1 << bit, &u32::MAX.to_le_bytes());
        assert!(matches!(
            FormulaMetadata::parse_bytes(&payload),
            Err(Error::LimitExceeded { .. })
        ));
        let payload = formula(1 << bit, &1_u32.to_le_bytes());
        assert!(matches!(
            FormulaMetadata::parse_bytes(&payload),
            Err(Error::Format(_))
        ));
    }
    for size in [0, 1, base().len() as u32, u32::MAX] {
        let mut data = stroke_list(0xff000000);
        data[4..8].copy_from_slice(&size.to_le_bytes());
        assert!(FormulaMetadata::parse_bytes(&formula(1 << 7, &data)).is_err());
    }
    let graph = label_graphs();
    for offset in [4, 48, 97, 105] {
        let mut data = graph.clone();
        data[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(
            matches!(
                FormulaMetadata::parse_bytes(&formula(1 << 14, &data)),
                Err(Error::LimitExceeded {
                    resource: "math entries",
                    ..
                })
            ),
            "offset {offset}"
        );
    }
}

#[test]
fn label_and_latex_utf8_lengths_use_utf16_units_for_text_limits() {
    let limits = ParseLimits {
        max_text_characters: 2,
        ..Default::default()
    };
    for bit in [0, 4, 15] {
        let payload = formula(1 << bit, &latex_list(&["😀"]));
        FormulaMetadata::parse_bytes_with_limits(&payload, &limits).unwrap();
        let restricted = ParseLimits {
            max_text_characters: 1,
            ..limits
        };
        assert!(matches!(
            FormulaMetadata::parse_bytes_with_limits(&payload, &restricted),
            Err(Error::LimitExceeded {
                resource: "text characters",
                actual: 2,
                ..
            })
        ));
        let payload = formula(1 << bit, &[1, 0, 0, 0, 1, 0, 255]);
        assert!(FormulaMetadata::parse_bytes(&payload).is_err());
    }
    let payload = formula(1 << 14, &label_graphs());
    FormulaMetadata::parse_bytes_with_limits(&payload, &limits).unwrap();
    let restricted = ParseLimits {
        max_text_characters: 1,
        ..limits
    };
    assert!(matches!(
        FormulaMetadata::parse_bytes_with_limits(&payload, &restricted),
        Err(Error::LimitExceeded {
            resource: "text characters",
            actual: 2,
            ..
        })
    ));
    let mut data = label_graphs();
    data[12] = 255;
    assert!(FormulaMetadata::parse_bytes(&formula(1 << 14, &data)).is_err());
    data[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(FormulaMetadata::parse_bytes(&formula(1 << 14, &data)).is_err());
}

#[test]
fn invalid_formula_rectangles_and_embedded_stroke_types_are_rejected() {
    for bit in [1, 11, 12] {
        assert!(
            FormulaMetadata::parse_bytes(&formula(1 << bit, &bbox([0.0, f64::NAN, 1.0, 2.0])))
                .is_err()
        );
    }
    let mut data = label_graphs();
    data[16..24].copy_from_slice(&f64::INFINITY.to_le_bytes());
    assert!(FormulaMetadata::parse_bytes(&formula(1 << 14, &data)).is_err());
    let mut data = stroke_list(0xff000000);
    let kind_offset = 8 + base().len() + 4;
    data[kind_offset..kind_offset + 2].copy_from_slice(&11_i16.to_le_bytes());
    assert!(FormulaMetadata::parse_bytes(&formula(1 << 7, &data)).is_err());
}

#[test]
fn formula_inspection_checks_types_payload_bounds_and_direct_input_limits() {
    let payload = formula(0, &[]);
    for length in 0..payload.len() {
        assert!(
            FormulaMetadata::parse_bytes(&payload[..length]).is_err(),
            "length {length}"
        );
    }
    let bytes = page(&[vec![object(11, &payload, &[])]], 0, &[]);
    let stored = sdocx::parse_stored_page_bytes(&bytes).unwrap();
    let mut stored = stored.layers.layers[0].objects[0].clone();
    let limits = ParseLimits {
        max_entry_size: payload.len() as u64 - 1,
        ..Default::default()
    };
    assert!(matches!(
        stored.formula_metadata_with_limits(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "math payload size",
            ..
        })
    ));
    assert!(matches!(
        FormulaMetadata::parse_bytes_with_limits(&payload, &limits),
        Err(Error::LimitExceeded {
            resource: "formula payload size",
            ..
        })
    ));
    stored.object_type = ObjectType::Math;
    assert!(stored.formula_metadata(&bytes).is_err());
    stored.object_type = ObjectType::Formula;
    stored.payload_offset = usize::MAX;
    assert!(stored.formula_metadata(&bytes).is_err());
    for chain in [
        [base(), frame(21, &[], &[], &[], &[])].concat(),
        [frame(99, &[], &[], &[], &[]), frame(11, &[], &[], &[], &[])].concat(),
    ] {
        assert!(FormulaMetadata::parse_bytes(&chain).is_err());
    }
}

#[test]
fn embedded_math_formulas_can_be_inspected_without_changing_envelope_parsing() {
    let payload = all_fields();
    let flexible = [
        1_u32.to_le_bytes().to_vec(),
        (payload.len() as u32).to_le_bytes().to_vec(),
        payload,
    ]
    .concat();
    let math = [base(), frame(21, &[1], &[1], &[], &flexible)].concat();
    let bytes = page(&[vec![object(21, &math, &[])]], 0, &[]);
    let stored = sdocx::parse_stored_page_bytes(&bytes).unwrap();
    let math = stored.layers.layers[0].objects[0]
        .math_metadata(&bytes)
        .unwrap();
    assert_eq!(math.formula_objects.len(), 1);
    let formula = FormulaMetadata::parse_bytes(&math.formula_objects[0]).unwrap();
    assert_eq!(formula.answer.as_deref(), Some("答😀"));
    assert_eq!(formula.strokes.len(), 1);
}

#[test]
fn formula_label_relations_expose_native_names_without_indexing_or_losing_unknown_values() {
    for (raw, expected) in [
        (0_u32, FormulaLabelRelationKind::Unknown),
        (1, FormulaLabelRelationKind::Right),
        (2, FormulaLabelRelationKind::Subscript),
        (3, FormulaLabelRelationKind::Superscript),
        (4, FormulaLabelRelationKind::Inside),
        (5, FormulaLabelRelationKind::Below),
        (6, FormulaLabelRelationKind::Above),
        (7, FormulaLabelRelationKind::Index),
        (8, FormulaLabelRelationKind::Other(8)),
        (u32::MAX, FormulaLabelRelationKind::Other(u32::MAX)),
    ] {
        let mut data = label_graphs();
        data[109..113].copy_from_slice(&u32::MAX.to_le_bytes());
        data[117..121].copy_from_slice(&raw.to_le_bytes());
        let value = FormulaMetadata::parse_bytes(&formula(1 << 14, &data)).unwrap();
        let relation = &value.label_graphs[0].relations[0];
        assert_eq!(relation.from_label, u32::MAX);
        assert_eq!(relation.to_label, 1);
        assert_eq!(relation.kind_raw, raw);
        assert_eq!(relation.kind(), expected);
    }
}

#[test]
fn formula_graph_endpoints_preserve_empty_and_unresolved_graphs() {
    let data = [1_u32, 0, 0, u32::MAX, u32::MAX]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let value = FormulaMetadata::parse_bytes(&formula(1 << 14, &data)).unwrap();
    let graph = &value.label_graphs[0];
    assert!(graph.labels.is_empty());
    assert!(graph.relations.is_empty());
    assert_eq!(graph.start_label, u32::MAX);
    assert_eq!(graph.end_label, u32::MAX);
    let mut data = label_graphs();
    data[121..125].copy_from_slice(&0_u32.to_le_bytes());
    data[125..129].copy_from_slice(&1_u32.to_le_bytes());
    let value = FormulaMetadata::parse_bytes(&formula(1 << 14, &data)).unwrap();
    let graph = &value.label_graphs[0];
    assert_eq!(graph.start_label, 0);
    assert_eq!(graph.end_label, 1);
    assert_eq!(graph.labels[graph.start_label as usize].text, "𝑥");
    assert_eq!(graph.labels[graph.end_label as usize].text, "+");
}
