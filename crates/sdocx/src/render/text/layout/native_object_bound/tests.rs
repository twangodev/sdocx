use super::*;
use crate::fonts::FontBook;
use crate::render::RenderTheme;
use crate::render::embedded::PreparedObject;
use crate::render::text::{PageExclusions, TextContext, TextSettings, VerticalExclusion};
use crate::{
    BoundingBox, Color, Document, DocumentMetadata, Page, RichTextBox, RichTextObjectSpan,
    RichTextParagraph, RichTextParagraphType, RichTextTable,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const CAPTURE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../conformance/table-bodytext-placement.json"
));

#[derive(Deserialize)]
struct Capture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    bounds_bits: [u32; 4],
    texts_utf8: [String; 4],
    font_size_bits: u32,
    margin_bits: Option<[u32; 4]>,
    text_scale_bits: u32,
    document_density_bits: u32,
    supplied_local_split_band_bits: Vec<[u32; 4]>,
    writer: Parent,
}

#[derive(Deserialize)]
struct Parent {
    supplied_layout_constraint: u32,
    supplied_document_size_bits: [u32; 2],
    object_size_updates: Vec<Callback>,
    document_bound_bits: [u32; 4],
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Callback {
    utf16_anchor: usize,
    output_rect_bits: [u32; 4],
    minimum_height_bits: u32,
}

#[derive(Deserialize)]
struct Entry {
    utf16_slot: usize,
    advance_bits: u32,
    font_height_bits: u32,
    font_size_bits: u32,
    position_bits: [u32; 2],
    layout_rect_bits: [u32; 4],
    ink_rect_bits: [u32; 4],
    kind: u8,
}

fn capture() -> Capture {
    assert_eq!(
        format!("{:x}", Sha256::digest(CAPTURE_BYTES)),
        "7a66e259030edb4e019258fc49f07a717d6386bba6db2d11945f1307c0fa594b"
    );
    let capture: Capture = serde_json::from_slice(CAPTURE_BYTES).unwrap();
    assert_eq!(capture.cases.len(), 3);
    capture
}

fn bounds([x_min, y_min, x_max, y_max]: [f32; 4]) -> BoundingBox {
    BoundingBox {
        x_min: f64::from(x_min),
        y_min: f64::from(y_min),
        x_max: f64::from(x_max),
        y_max: f64::from(y_max),
    }
}

fn source_table(case: &Case) -> RichTextTable {
    let [left, top, right, bottom] = case.bounds_bits.map(f32::from_bits);
    let width = (right - left) * 0.5;
    let height = (bottom - top) * 0.5;
    let mut table = crate::render::table::tests::grid(&[height; 2], &[width; 2]);
    table.bbox = bounds([left, top, right, bottom]);
    table.style.content_bbox = Some(table.bbox);
    for (slot, cell) in table
        .rows
        .iter_mut()
        .flat_map(|row| &mut row.cells)
        .enumerate()
    {
        let x = left + width * (slot % 2) as f32;
        let y = top + height * (slot / 2) as f32;
        cell.bbox = bounds([x, y, x + width, y + height]);
        cell.content.bbox = cell.bbox;
        cell.content.text = case.texts_utf8[slot].clone();
        cell.content.font_size = Some(f32::from_bits(case.font_size_bits));
        cell.content.color = Some(Color {
            r: 37,
            g: 37,
            b: 37,
        });
        cell.content.gravity = Some(1);
        cell.content.margins = case.margin_bits.map(|bits| bits.map(f32::from_bits));
        cell.content.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::Alignment,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: 2_u32.to_le_bytes().to_vec(),
        });
    }
    table
}

fn constraint(case: &Case) -> ObjectSpanLayoutConstraint {
    match case.writer.supplied_layout_constraint {
        1 => ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        2 => ObjectSpanLayoutConstraint::OverPages,
        value => panic!("uncaptured constraint {value}"),
    }
}

fn source(case: &Case) -> RichTextBox {
    let [width, height] = case.writer.supplied_document_size_bits.map(f32::from_bits);
    RichTextBox {
        text_area_type: None,
        bbox: bounds([0.0, 0.0, width, height]),
        rotation_degrees: None,
        text: "\u{fffc}".into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: None,
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: vec![RichTextObjectSpan {
            object_type: crate::ObjectType::Table,
            object_data: Vec::new(),
            content: Some(RichTextObjectContent::Table(Box::new(source_table(case)))),
            text_index_utf16: 0,
            layout_option: ObjectSpanLayoutOption::Block,
            layout_constraint: constraint(case),
        }],
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn renderer<'a>(case: &Case, fonts: &'a FontBook) -> TextRenderer<'a> {
    TextRenderer::new(
        TextSettings {
            scale: f32::from_bits(case.text_scale_bits)
                * f32::from_bits(case.document_density_bits),
            font_size_delta: 0.0,
            ..Default::default()
        },
        fonts,
    )
}

fn frame(source: &RichTextBox) -> TextFrame<'_> {
    TextFrame {
        bbox: source.bbox,
        gravity: Some(0),
        exclusions: &[],
    }
}

fn layout(source: &RichTextBox, renderer: &TextRenderer<'_>) -> super::super::TextLayout {
    layout_in_context(source, renderer, LayoutContext::Capture)
}

fn layout_in_context(
    source: &RichTextBox,
    renderer: &TextRenderer<'_>,
    context: LayoutContext,
) -> super::super::TextLayout {
    let styled = StyledText::new(source, TextContext::Flow, renderer.settings);
    match context {
        LayoutContext::Flow => super::super::layout_flow_text(
            &styled,
            frame(source),
            RenderTheme::for_canvas(false),
            renderer,
        ),
        LayoutContext::Capture => super::super::layout_capture_text(
            &styled,
            frame(source),
            RenderTheme::for_canvas(false),
            renderer,
        ),
        LayoutContext::Frame => panic!("parent capture requires body context"),
    }
}

pub(in crate::render) fn caller_capture_source(value: &serde_json::Value) -> RichTextBox {
    source(&serde_json::from_value(value.clone()).unwrap())
}

pub(in crate::render) fn caller_capture_profile(
    value: &serde_json::Value,
) -> (RichTextTable, super::super::TextLayout) {
    let case: Case = serde_json::from_value(value.clone()).unwrap();
    let source = caller_capture_source(value);
    let Some(RichTextObjectContent::Table(table)) = &source.object_spans[0].content else {
        panic!("expected source table");
    };
    let table = table.as_ref().clone();
    let fonts = FontBook::default();
    let renderer = renderer(&case, &fonts);
    let layout = layout_in_context(&source, &renderer, LayoutContext::Flow);
    (table, layout)
}

#[test]
fn caller_source_produces_native_parent_entry_and_table_callback_bounds() {
    let fonts = FontBook::default();
    let values: serde_json::Value = serde_json::from_slice(CAPTURE_BYTES).unwrap();
    for (case, value) in capture()
        .cases
        .into_iter()
        .zip(values["cases"].as_array().unwrap())
    {
        assert!(case.supplied_local_split_band_bits.is_empty());
        let source = source(&case);
        let original = source.clone();
        let renderer = renderer(&case, &fonts);
        for context in [LayoutContext::Flow, LayoutContext::Capture] {
            let layout = if matches!(context, LayoutContext::Flow) {
                let (table, layout) = caller_capture_profile(value);
                assert_eq!(
                    source.object_spans[0].content,
                    Some(RichTextObjectContent::Table(Box::new(table)))
                );
                layout
            } else {
                layout_in_context(&source, &renderer, context)
            };
            let entry = layout.native_object_entry.expect(&case.name);
            let [expected] = case.writer.entries.as_slice() else {
                panic!("expected one object")
            };
            assert_eq!(expected.kind, 5);
            assert_eq!(expected.utf16_slot, 0);
            assert_eq!(
                entry.advance.to_bits(),
                expected.advance_bits,
                "{} advance",
                case.name
            );
            assert_eq!(
                entry.height.to_bits(),
                expected.font_height_bits,
                "{} height",
                case.name
            );
            assert_eq!(
                entry.font_size.to_bits(),
                expected.font_size_bits,
                "{} font",
                case.name
            );
            assert_eq!(
                entry.position.map(f32::to_bits),
                expected.position_bits,
                "{} position",
                case.name
            );
            assert_eq!(
                entry.layout.map(f32::to_bits),
                expected.layout_rect_bits,
                "{} layout",
                case.name
            );
            assert_eq!(
                entry.ink.map(f32::to_bits),
                expected.ink_rect_bits,
                "{} ink",
                case.name
            );
            assert_eq!(
                entry.text_bound().map(f32::to_bits),
                case.writer.document_bound_bits,
                "{} text bound",
                case.name
            );

            let [line] = layout.lines.as_slice() else {
                panic!("expected one line")
            };
            let [object] = line.line.objects.as_slice() else {
                panic!("expected one object")
            };
            let Some(Ok(PreparedObject::Table(prepared))) = &object.prepared else {
                panic!("expected prepared table")
            };
            let [callback] = case.writer.object_size_updates.as_slice() else {
                panic!("expected one callback")
            };
            assert_eq!(callback.utf16_anchor, 0);
            assert_eq!(
                [
                    0.0,
                    0.0,
                    object.object.width as f32,
                    object.object.height as f32
                ]
                .map(f32::to_bits),
                callback.output_rect_bits,
                "{} callback dimensions",
                case.name
            );
            assert_eq!(
                (prepared.minimum_first_page_height().unwrap() as f32).to_bits(),
                callback.minimum_height_bits,
                "{} callback minimum",
                case.name
            );
            assert_eq!(
                entry.advance.to_bits(),
                case.writer.supplied_document_size_bits[0]
            );
            assert!(entry.advance > entry.ink[2] - entry.ink[0]);
            assert_eq!(source, original, "{} source mutation", case.name);
        }
    }
}

#[test]
fn native_zero_height_parent_retains_full_kind5_geometry_without_a_vertical_cap() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-bodytext-one-page-placement.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "cc0810421dceddffaa3454e8010ece57d7afddcce1f67dba1b662eeea4acd15a"
    );
    let capture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let value = &capture["cases"][0];
    assert_eq!(value["name"], "ordinary-one-page-zero-widget-height");
    let model = &value["writer"]["document_model_source"];
    assert_eq!(model["text_utf8"], "\u{fffc}");
    assert_eq!(model["font_size_spans"], serde_json::json!([]));
    assert_eq!(model["foreground_spans"], serde_json::json!([]));
    assert_eq!(
        model["font_size_at_utf16_including_end_bits"],
        serde_json::json!([17.0_f32.to_bits(), 17.0_f32.to_bits()])
    );
    let measurement = &value["writer"]["native_page_measurement"];
    assert_eq!(measurement["is_infinite_scroll"], false);
    assert_eq!(measurement["height_without_last_page"], 0);
    assert_eq!(measurement["height_including_last_page"], 600);
    assert_eq!(measurement["widget_layout_height_bits"], 0);
    let page_record: [i32; 5] =
        serde_json::from_value(measurement["supplied_page_record"].clone()).unwrap();
    assert_eq!(page_record, [0, 0, 0, 240, 600]);
    let case: Case = serde_json::from_value(value.clone()).unwrap();
    let source = source(&case);
    let original = source.clone();
    assert_eq!(source.font_size, None);
    assert!(source.spans.is_empty());
    assert_eq!(source.bbox.y_max, 600.0);
    let fonts = FontBook::default();
    let renderer = renderer(&case, &fonts);
    let styled = StyledText::new(&source, TextContext::Flow, renderer.settings);
    for context in [LayoutContext::Flow, LayoutContext::Capture] {
        let mut frame = frame(&source);
        frame.bbox.y_max = f64::from(page_record[0]);
        assert_eq!(
            (frame.bbox.y_max as f32).to_bits(),
            measurement["widget_layout_height_bits"].as_u64().unwrap() as u32
        );
        let outer_height = frame.bbox.y_max as f32;
        let layout = match context {
            LayoutContext::Flow => super::super::layout_flow_text(
                &styled,
                frame,
                RenderTheme::for_canvas(false),
                &renderer,
            ),
            LayoutContext::Capture => super::super::layout_capture_text(
                &styled,
                frame,
                RenderTheme::for_canvas(false),
                &renderer,
            ),
            LayoutContext::Frame => unreachable!(),
        };
        let entry = layout.native_object_entry.unwrap();
        let [expected] = case.writer.entries.as_slice() else {
            panic!("expected one object")
        };
        assert_eq!(entry.advance.to_bits(), expected.advance_bits);
        assert_eq!(entry.height.to_bits(), expected.font_height_bits);
        assert_eq!(entry.font_size.to_bits(), expected.font_size_bits);
        assert_eq!(entry.position.map(f32::to_bits), expected.position_bits);
        assert_eq!(entry.layout.map(f32::to_bits), expected.layout_rect_bits);
        assert_eq!(entry.ink.map(f32::to_bits), expected.ink_rect_bits);
        assert_eq!(
            entry.text_bound().map(f32::to_bits),
            case.writer.document_bound_bits
        );
        let [object] = layout.lines[0].line.objects.as_slice() else {
            panic!("expected one object")
        };
        let [callback] = case.writer.object_size_updates.as_slice() else {
            panic!("expected one callback")
        };
        assert_eq!(callback.utf16_anchor, 0);
        assert_eq!(
            [
                0.0,
                0.0,
                object.object.width as f32,
                object.object.height as f32
            ]
            .map(f32::to_bits),
            callback.output_rect_bits
        );
        assert!(entry.layout[3] > outer_height);
        assert_eq!(source, original);
    }
}

#[test]
fn unchanged_prepared_geometry_cannot_certify_unsupported_parent_controls() {
    let fonts = FontBook::default();
    let case = capture().cases.remove(0);
    let source = source(&case);
    let renderer = renderer(&case, &fonts);
    let layout = layout(&source, &renderer);
    let certified = layout.native_object_entry.unwrap();
    type SourceChange = (&'static str, fn(&mut RichTextBox));
    let variants: [SourceChange; 8] = [
        ("inline", |source| {
            source.object_spans[0].layout_option = ObjectSpanLayoutOption::Inline
        }),
        ("font", |source| source.font_size = Some(18.0)),
        ("margins", |source| {
            source.margins = Some([1.0, 0.0, 0.0, 0.0])
        }),
        ("gravity", |source| source.gravity = Some(1)),
        ("foreground", |source| {
            source.color = Some(Color { r: 1, g: 2, b: 3 })
        }),
        ("highlight", |source| {
            source.highlight_color = Some(Color { r: 1, g: 2, b: 3 })
        }),
        ("underline", |source| source.underline = true),
        ("paragraph", |source| {
            source.paragraphs.push(RichTextParagraph {
                kind: RichTextParagraphType::Alignment,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: 2_u32.to_le_bytes().to_vec(),
            })
        }),
    ];
    for (name, change) in variants {
        let mut changed = source.clone();
        change(&mut changed);
        assert_eq!(
            changed.object_spans[0].content,
            source.object_spans[0].content
        );
        let styled = StyledText::new(&changed, TextContext::Flow, renderer.settings);
        assert_eq!(
            native_object_entry_bounds(
                &styled,
                &frame(&source),
                &layout.lines,
                &renderer,
                LayoutContext::Capture
            ),
            None,
            "{name}"
        );
    }
    let styled = StyledText::new(&source, TextContext::Flow, renderer.settings);
    let mut moved = frame(&source);
    moved.bbox.x_min = 0.5;
    assert_eq!(
        native_object_entry_bounds(
            &styled,
            &moved,
            &layout.lines,
            &renderer,
            LayoutContext::Capture
        ),
        None
    );
    let exclusions = [VerticalExclusion::obstacle(2000.0, 2010.0)];
    let mut blocked = frame(&source);
    blocked.exclusions = &exclusions;
    assert_eq!(
        native_object_entry_bounds(
            &styled,
            &blocked,
            &layout.lines,
            &renderer,
            LayoutContext::Capture
        ),
        None
    );
    assert_eq!(layout.native_object_entry, Some(certified));
    for height in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1.0,
        50.0,
        1000.0000001,
    ] {
        let mut unsupported = frame(&source);
        unsupported.bbox.y_max = height;
        assert_eq!(
            native_object_entry_bounds(
                &styled,
                &unsupported,
                &layout.lines,
                &renderer,
                LayoutContext::Capture,
            ),
            None,
            "height {height}",
        );
    }
}

#[test]
fn saved_negative_table_top_preserves_flow_continuation_and_withdraws_certificate() {
    let fonts = FontBook::default();
    let case = capture().cases.remove(0);
    let mut source = source(&case);
    let Some(RichTextObjectContent::Table(table)) = &mut source.object_spans[0].content else {
        panic!("expected source table");
    };
    table.bbox.y_min -= 10.0;
    table.bbox.y_max -= 10.0;
    table.style.content_bbox = Some(table.bbox);
    for cell in table.rows.iter_mut().flat_map(|row| &mut row.cells) {
        cell.bbox.y_min -= 10.0;
        cell.bbox.y_max -= 10.0;
        cell.content.bbox = cell.bbox;
    }
    let original = source.clone();
    let renderer = renderer(&case, &fonts);
    let layout = layout_in_context(&source, &renderer, LayoutContext::Flow);
    assert_eq!(layout.native_object_entry, None);
    let [line] = layout.lines.as_slice() else {
        panic!("expected one line")
    };
    let [object] = line.line.objects.as_slice() else {
        panic!("expected one object")
    };
    assert!(object.object.bounds.y_min < 0.0);
    assert_eq!(
        line.baseline,
        object.object.bounds.y_min + object.object.height
    );
    assert_eq!(source, original);
}

#[test]
fn valid_prepared_tables_with_uncertified_cell_sources_withdraw_parent_bounds() {
    let fonts = FontBook::default();
    let case = capture().cases.remove(0);
    for empty in [false, true] {
        let mut source = source(&case);
        let Some(RichTextObjectContent::Table(table)) = &mut source.object_spans[0].content else {
            panic!("expected source table");
        };
        let cell = &mut table.rows[0].cells[0];
        if empty {
            cell.content.text.clear();
        } else {
            cell.content.font_size = Some(10.0);
        }
        let original = source.clone();
        let renderer = renderer(&case, &fonts);
        for context in [LayoutContext::Flow, LayoutContext::Capture] {
            let layout = layout_in_context(&source, &renderer, context);
            assert_eq!(layout.native_object_entry, None, "empty cell {empty}");
            let [line] = layout.lines.as_slice() else {
                panic!("expected one parent line");
            };
            let [object] = line.line.objects.as_slice() else {
                panic!("expected one table object");
            };
            let Some(Ok(PreparedObject::Table(prepared))) = &object.prepared else {
                panic!("uncertified cell must retain valid table preparation");
            };
            assert!(prepared.measured_bbox.x_max > prepared.measured_bbox.x_min);
            assert!(prepared.measured_bbox.y_max > prepared.measured_bbox.y_min);
            let child = &prepared.rows[0].cells[0].layout;
            if empty {
                assert!(
                    child.lines.is_empty()
                        || child.lines.iter().any(|line| line.line.source.is_empty())
                );
            } else {
                assert!(!child.lines.is_empty());
                assert!(child.lines.iter().all(|line| line.line.font_size == 10.0));
                assert!(
                    child
                        .lines
                        .iter()
                        .any(|line| !line.line.placements.is_empty())
                );
                for placement in child.lines.iter().flat_map(|line| &line.line.placements) {
                    let entries = placement.cluster.run.native_entries.as_ref().unwrap();
                    let owner = entries.geometry().source_range_utf16().start;
                    assert!(entries.entry_facts_at_utf16(owner).is_none());
                }
            }
            assert_eq!(source, original);
        }
    }
}

#[test]
fn tiny_negative_saved_table_top_withdraws_capture_bounds() {
    let fonts = FontBook::default();
    let case = capture().cases.remove(0);
    let mut source = source(&case);
    let Some(RichTextObjectContent::Table(table)) = &mut source.object_spans[0].content else {
        panic!("expected source table");
    };
    table.bbox.y_min = -f64::from(f32::EPSILON);
    let original = source.clone();
    let renderer = renderer(&case, &fonts);
    let layout = layout(&source, &renderer);
    assert_eq!(layout.native_object_entry, None);
    let [line] = layout.lines.as_slice() else {
        panic!("expected one parent line");
    };
    let [object] = line.line.objects.as_slice() else {
        panic!("expected one table object");
    };
    assert!(matches!(
        object.prepared,
        Some(Ok(PreparedObject::Table(_)))
    ));
    assert_eq!(source, original);
}

#[test]
fn page_bands_and_nonzero_translation_withdraw_parent_entry_certificate() {
    let fonts = FontBook::default();
    let case = capture().cases.remove(0);
    let source = source(&case);
    let renderer = renderer(&case, &fonts);
    let mut layout = layout(&source, &renderer);
    let certified = layout.native_object_entry.unwrap();
    let page = Page {
        uuid: "parent-bands-control".into(),
        width: 400,
        height: 1000,
        content_bbox: BoundingBox::default(),
        background_color: None,
        template: None,
        background: Default::default(),
        objects: Vec::new(),
    };
    let document = Document {
        pages: vec![page],
        metadata: DocumentMetadata {
            page_mode: Some(0),
            default_page_dimensions: Some((400, 1000)),
            ..Default::default()
        },
    };
    let pages = PageExclusions::for_document(&document, renderer.settings).unwrap();
    let paginated = renderer.clone().with_page_exclusions(Some(pages));
    assert!(
        !paginated
            .table_split_rects(constraint(&case), 0.0)
            .is_empty()
    );
    let styled = StyledText::new(&source, TextContext::Flow, renderer.settings);
    assert_eq!(
        native_object_entry_bounds(
            &styled,
            &frame(&source),
            &layout.lines,
            &paginated,
            LayoutContext::Capture
        ),
        None
    );
    layout.translate(0.0, 0.0);
    assert_eq!(layout.native_object_entry, Some(certified));
    layout.translate(0.25, -0.5);
    assert_eq!(layout.native_object_entry, None);
    assert_eq!(
        layout.lines[0].baseline,
        f64::from(certified.position[1]) - 0.5
    );
}
