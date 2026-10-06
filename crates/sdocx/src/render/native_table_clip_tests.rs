use super::*;
use crate::fonts::FontBook;
use sha2::{Digest, Sha256};

fn captured_rect(value: &serde_json::Value) -> BoundingBox {
    let values = value.as_array().unwrap();
    let coordinates = std::array::from_fn::<_, 4, _>(|axis| {
        f64::from(f32::from_bits(values[axis].as_u64().unwrap() as u32))
    });
    BoundingBox {
        x_min: coordinates[0],
        y_min: coordinates[1],
        x_max: coordinates[2],
        y_max: coordinates[3],
    }
}

fn capture() -> serde_json::Value {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-bodytext-placement.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "7a66e259030edb4e019258fc49f07a717d6386bba6db2d11945f1307c0fa594b"
    );
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn parsed_consecutive_newlines_preserve_certified_cell_positions() {
    let bytes = include_bytes!("../../tests/fixtures/native_cell_consecutive.sdocx");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "6fd7a46b894e5640c321af751f1d63e52cb7964fd6f23e6ff3c33a0cd12d1e8d"
    );
    let document = crate::parse_bytes(bytes).unwrap();
    let original = document.metadata.note_text.clone();
    let source = document.metadata.note_text.as_ref().unwrap();
    assert_eq!(
        source.bbox,
        BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 80.0,
            y_max: 1000.0
        }
    );
    assert_eq!(source.text_sections.len(), 1);
    assert_eq!(
        source.object_spans[0].layout_option,
        crate::ObjectSpanLayoutOption::Inline
    );
    assert_eq!(
        source.object_spans[0].layout_constraint,
        crate::ObjectSpanLayoutConstraint::Normal
    );
    let capture_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-text-cell-emission.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(capture_bytes)),
        "0098cfcd93274b210892fd0653677fd4cef3a35ebc4f0419b4b661857cbfa4ce"
    );
    let capture: serde_json::Value = serde_json::from_slice(capture_bytes).unwrap();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "consecutive-newlines")
        .unwrap();
    let rendered = render_document_svg(&document, &Default::default());
    let xml = roxmltree::Document::parse(&rendered[0].svg).unwrap();
    let groups = xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect::<Vec<_>>();
    let expected = case["emitted_runs"]["runs"].as_array().unwrap();
    assert_eq!(groups.len(), expected.len());
    let coordinate =
        |value: &serde_json::Value| f64::from(f32::from_bits(value.as_u64().unwrap() as u32));
    for (group, expected) in groups.iter().zip(expected) {
        let spans = group
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .collect::<Vec<_>>();
        let positions = spans
            .iter()
            .flat_map(|node| node.attribute("x").unwrap().split_whitespace())
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            positions,
            expected["position_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|position| coordinate(position) + 4.0)
                .collect::<Vec<_>>()
        );
        for span in spans {
            assert_eq!(
                span.attribute("y").unwrap().parse::<f64>().unwrap(),
                coordinate(&expected["origin_bits"][1]) + 10.0
            );
        }
    }
    assert!(!rendered[0].svg.contains("<image"));
    #[cfg(feature = "pdf")]
    {
        let bytes = crate::render_document_pdf(&document, &Default::default(), &Default::default())
            .unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let page = pdf.get_pages().keys().copied().next().unwrap();
        let text = pdf.extract_text(&[page]).unwrap();
        assert!(text.contains("AV"));
        assert!(text.contains("To"));
    }
    assert_eq!(document.metadata.note_text, original);
}

#[test]
fn native_page_line_selection_limits_paint_and_diagnostics_without_an_ink_viewport() {
    use text::native_page_index::{NativePageLineRange, NativePageSection};

    let capture = capture();
    let mut source = text::native_object_capture_source(&capture["cases"][0]);
    source.text = "hidden\t\nvisible\nhidden\t".into();
    source.object_spans.clear();
    source.spans.push(crate::RichTextSpan {
        kind: crate::RichTextSpanType::BackgroundColor,
        start_utf16: 0,
        end_utf16: source.text.len() as u32,
        interval_type: crate::SpanIntervalType::from(0),
        payload: 0xffff0000_u32.to_le_bytes().to_vec(),
    });
    let fonts = FontBook::default();
    let settings = TextSettings::resolved();
    let theme = RenderTheme::for_canvas(false);
    let renderer = TextRenderer::new(settings, &fonts);
    let styled = StyledText::new(&source, TextContext::Flow, settings);
    let layout = text::layout_capture_text(
        &styled,
        text::TextFrame {
            bbox: BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 240.0,
                y_max: 0.0,
            },
            gravity: None,
            exclusions: &[],
        },
        theme,
        &renderer,
    );
    assert_eq!(layout.lines.len(), 3);
    let lines = NativePageLineRange::new(NativePageSection { start: 1, count: 1 }, 3).unwrap();
    for (range, expected) in [(lines, "visible"), (NativePageLineRange::EMPTY, "")] {
        let renderer = TextRenderer::new(settings, &fonts);
        let mut scene = Scene::new(Svg::new());
        paint_text_layout_with_target(
            &mut scene,
            &styled,
            &layout,
            &[],
            theme,
            &renderer,
            text::NativePaintTarget::new(false, None).with_native_lines(range),
        );
        let svg = scene.finish();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let emitted = xml
            .descendants()
            .filter(|node| {
                node.is_text()
                    && node
                        .parent()
                        .is_some_and(|parent| parent.has_tag_name("tspan"))
            })
            .filter_map(|node| node.text())
            .collect::<String>();
        assert_eq!(emitted, expected);
        assert!(
            renderer.diagnostics().is_empty(),
            "{:?}",
            renderer.diagnostics()
        );
        assert!(renderer.object_diagnostics().is_empty());
        let backgrounds = xml
            .descendants()
            .filter(|node| node.has_tag_name("rect"))
            .collect::<Vec<_>>();
        assert_eq!(backgrounds.is_empty(), expected.is_empty());
        for rectangle in backgrounds {
            let top = rectangle.attribute("y").unwrap().parse::<f64>().unwrap();
            assert!(top > layout.lines[0].background_top);
            assert!(top < layout.lines[2].background_top);
        }
    }
    let renderer = TextRenderer::new(settings, &fonts);
    let mut scene = Scene::new(Svg::new());
    paint_text_layout(&mut scene, &styled, &layout, &[], theme, &renderer);
    assert!(
        renderer
            .diagnostics()
            .iter()
            .any(|issue| { issue.kind == TextDiagnosticKind::UnsupportedTabMeasurement })
    );
}

#[test]
fn captured_parent_placement_activates_only_the_native_per_run_text_clips() {
    let capture = capture();
    let fonts = FontBook::default();
    let theme = RenderTheme::for_canvas(false);
    let settings = TextSettings::resolved();
    let mut admitted_profiles = 0;
    for case in capture["cases"].as_array().unwrap() {
        if case["writer"]["supplied_page_origin_bits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_u64() != Some(0))
        {
            continue;
        }
        admitted_profiles += 1;
        let (_, layout) = text::native_object_capture_profile(case);
        let source = text::native_object_capture_source(case);
        let [width, height] = [source.bbox.x_max as u32, source.bbox.y_max as u32];
        let original = source.clone();
        let entry = layout.native_object_entry.unwrap();
        let [x_min, y_min, x_max, y_max] = entry.text_bound().map(f64::from);
        let target = BoundingBox {
            x_min,
            y_min,
            x_max,
            y_max,
        };
        assert_eq!(
            target,
            captured_rect(&case["writer"]["document_bound_bits"])
        );
        for retain_text in [false, true] {
            let renderer = TextRenderer::new(settings, &fonts);
            let captured_cells = case["writer"]["writer_after"]["cells"].as_array().unwrap();
            let expected = captured_cells
                .iter()
                .flat_map(|cell| {
                    let slot = cell["slot"].as_u64().unwrap() as usize;
                    cell["runs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(move |run| (slot, run))
                })
                .collect::<Vec<_>>();
            let mut scene = Scene::new(Svg::new().width(width).height(height));
            #[cfg(feature = "pdf")]
            if retain_text {
                scene.retain_text();
            }
            assert_eq!(scene.retains_text(), retain_text && cfg!(feature = "pdf"));
            let styled = StyledText::new(&source, TextContext::Flow, renderer.settings);
            paint_text_layout(&mut scene, &styled, &layout, &[], theme, &renderer);
            #[cfg(feature = "pdf")]
            let registry = retain_text.then(|| scene.take_native_text());
            let svg = scene.finish();
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let groups = xml
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .collect::<Vec<_>>();
            assert_eq!(groups.len(), expected.len(), "{}", case["name"]);
            for (group, (slot, run)) in groups.iter().zip(&expected) {
                let range = run["range_inclusive"].as_array().unwrap();
                let source = case["texts_utf8"][*slot]
                    .as_str()
                    .unwrap()
                    .chars()
                    .skip(range[0].as_u64().unwrap() as usize)
                    .take((range[1].as_u64().unwrap() - range[0].as_u64().unwrap() + 1) as usize)
                    .collect::<String>();
                assert_eq!(
                    group
                        .descendants()
                        .filter(|node| node.is_text())
                        .filter_map(|node| node.text())
                        .collect::<String>(),
                    source
                );
                let clips = group
                    .ancestors()
                    .filter_map(|node| node.attribute("clip-path"))
                    .collect::<Vec<_>>();
                assert_eq!(
                    clips.len(),
                    usize::from(run["clip_selected"].as_bool().unwrap()),
                    "{} slot{slot}",
                    case["name"]
                );
                if let Some(clip) = clips.first() {
                    let id = clip
                        .strip_prefix("url(#")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap();
                    let rectangle = xml
                        .descendants()
                        .find(|node| node.attribute("id") == Some(id))
                        .unwrap()
                        .descendants()
                        .find(|node| node.has_tag_name("rect"))
                        .unwrap();
                    let bounds = captured_rect(&run["world_clip_bits"]);
                    for (attribute, expected) in [
                        ("x", bounds.x_min),
                        ("y", bounds.y_min),
                        ("width", bounds.x_max - bounds.x_min),
                        ("height", bounds.y_max - bounds.y_min),
                    ] {
                        assert_eq!(
                            rectangle
                                .attribute(attribute)
                                .unwrap()
                                .parse::<f64>()
                                .unwrap(),
                            expected,
                            "{} slot{slot} {attribute}",
                            case["name"]
                        );
                    }
                }
            }
            assert!(!svg.contains("<image"));
            assert!(
                renderer.object_diagnostics().is_empty(),
                "{:?}",
                renderer.object_diagnostics()
            );
            assert!(
                renderer.diagnostics().is_empty(),
                "{:?}",
                renderer.diagnostics()
            );
            #[cfg(feature = "pdf")]
            if retain_text {
                let registry = registry.unwrap();
                assert_eq!(registry.iter().count(), expected.len());
                let scene = RenderedScene {
                    page: RenderedPage {
                        source_page_index: 0,
                        width,
                        height,
                        svg,
                        text_diagnostics: Vec::new(),
                        object_diagnostics: Vec::new(),
                        geometry_diagnostics: Vec::new(),
                        paint_diagnostics: Vec::new(),
                    },
                    text: registry,
                    text_error: None,
                };
                let pdf = crate::pdf::render_native_scene_pdf_for_test(
                    &scene,
                    &crate::pdf::PdfOptions::from_font_book(&fonts),
                )
                .unwrap();
                let pdf = lopdf::Document::load_mem(&pdf).unwrap();
                assert!(!pdf.objects.values().any(|object| {
                    object.as_stream().is_ok_and(|stream| {
                        stream
                            .dict
                            .get(b"Subtype")
                            .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
                    })
                }));
                let page = pdf.get_pages().keys().copied().next().unwrap();
                let text = pdf.extract_text(&[page]).unwrap();
                for source in ["AV", "To", "last", "A\u{301}B"] {
                    assert!(text.contains(source), "{text:?}");
                }
            }
        }
        assert_eq!(source, original);
    }
    assert_eq!(admitted_profiles, 1);
}

#[test]
fn uncertified_fit_and_target_controls_keep_conservative_selectable_cell_text() {
    let capture = capture();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "overpages-zero-origin")
        .unwrap();
    let (table, layout) = text::native_object_capture_profile(case);
    let entry = layout.native_object_entry.unwrap();
    let Some(Ok(embedded::PreparedObject::Table(prepared))) =
        &layout.lines[0].line.objects[0].prepared
    else {
        panic!("captured parent callback table required");
    };
    let fonts = FontBook::default();
    let theme = RenderTheme::for_canvas(false);
    for control in [
        "horizontal-fit",
        "width-cap",
        "height-cap",
        "translated-target",
    ] {
        let mut source = table.clone();
        let [x_min, y_min, x_max, y_max] = entry.text_bound().map(f64::from);
        let mut target = BoundingBox {
            x_min,
            y_min,
            x_max,
            y_max,
        };
        match control {
            "horizontal-fit" => source.style.auto_fit = Some(crate::TableAutoFit::Horizontal),
            "width-cap" => source.style.max_width = Some(200.0),
            "height-cap" => source.style.max_height = Some(200.0),
            "translated-target" => {
                target.x_min += 1.0;
                target.x_max += 1.0;
            }
            _ => unreachable!(),
        }
        let before = source.clone();
        let renderer = TextRenderer::new(TextSettings::resolved(), &fonts);
        let drawing = table::prepare_table_clone_drawing_with_native_entry(
            &source,
            crate::ObjectSpanLayoutConstraint::OverPages,
            target,
            theme,
            &renderer,
            Some(table::NativeTableDocumentSource::from_entry(
                entry, prepared,
            )),
        )
        .unwrap()
        .unwrap();
        assert!(!has_native_table_clipping(&drawing.rows), "{control}");
        assert!(
            drawing.rows.iter().flat_map(|row| &row.cells).all(|cell| {
                cell.native_text_placement().unwrap().provenance()
                    == table::native_placement::NativeCellTextClipProvenance::Unknown
            }),
            "{control}"
        );
        let mut scene = Scene::new(Svg::new());
        render_table(
            &mut scene,
            &source,
            0,
            Some((&drawing).into()),
            0.0,
            &[],
            theme,
            &renderer,
            None,
        );
        let svg = scene.finish();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let groups = xml
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .collect::<Vec<_>>();
        assert!(!groups.is_empty());
        let clip_ids = groups
            .iter()
            .map(|group| {
                let clips = group
                    .ancestors()
                    .filter_map(|node| node.attribute("clip-path"))
                    .collect::<Vec<_>>();
                assert_eq!(clips.len(), 1, "{control}");
                clips[0]
            })
            .collect::<Vec<_>>();
        assert!(clip_ids.iter().all(|id| *id == clip_ids[0]), "{control}");
        let text = groups
            .iter()
            .flat_map(|group| group.descendants())
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<String>();
        assert_eq!(text, "AVTolastToA\u{301}Blast", "{control}");
        assert!(!svg.contains("<image"));
        assert_eq!(
            renderer.object_diagnostics(),
            [ObjectDiagnostic {
                anchor_utf16: 0,
                kind: ObjectDiagnosticKind::UnsupportedCellClipping
            }],
            "{control}"
        );
        assert_eq!(source, before);
    }
}

#[test]
fn later_page_context_withholds_local_parent_clip_provenance() {
    let capture = capture();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "overpages-zero-origin")
        .unwrap();
    let source = text::native_object_capture_source(case);
    let (_, layout) = text::native_object_capture_profile(case);
    assert!(layout.native_object_entry.is_some());
    let page = crate::Page {
        uuid: "translated-parent-clip".into(),
        width: 300,
        height: 600,
        content_bbox: BoundingBox::default(),
        background_color: None,
        template: None,
        background: Default::default(),
        objects: Vec::new(),
    };
    let page = table::TableExportPage::for_page(&page, &Default::default()).unwrap();
    assert!(page.has_zero_origin());
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(TextSettings::resolved(), &fonts)
        .with_table_export_page(Some(page))
        .translated_paint(0.0, 50.0);
    assert!(!renderer.table_export_page.unwrap().has_zero_origin());
    let styled = StyledText::new(&source, TextContext::Flow, renderer.settings);
    let mut scene = Scene::new(Svg::new());
    scene.scope(
        Group::new().transformed(Transform::translate(0.0, -50.0, 5)),
        |scene| {
            paint_text_layout(
                scene,
                &styled,
                &layout,
                &[],
                RenderTheme::for_canvas(false),
                &renderer,
            );
        },
    );
    let svg = scene.finish();
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let groups = xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), 6);
    let clips = groups
        .iter()
        .map(|group| {
            let clips = group
                .ancestors()
                .filter_map(|node| node.attribute("clip-path"))
                .collect::<Vec<_>>();
            assert_eq!(clips.len(), 1);
            clips[0]
        })
        .collect::<Vec<_>>();
    assert!(clips.iter().all(|id| *id == clips[0]));
    assert_eq!(
        renderer.object_diagnostics(),
        [ObjectDiagnostic {
            anchor_utf16: 0,
            kind: ObjectDiagnosticKind::UnsupportedCellClipping,
        }]
    );
    assert!(!svg.contains("<image"));
}

#[test]
fn public_document_render_activates_the_captured_parent_clips() {
    let capture = capture();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "overpages-zero-origin")
        .unwrap();
    let source = text::native_object_capture_source(case);
    let width = source.bbox.x_max as u32;
    let height = source.bbox.y_max as u32;
    let document = Document {
        pages: vec![crate::Page {
            uuid: "public-native-parent-clip".into(),
            width,
            height,
            content_bbox: source.bbox,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: Vec::new(),
        }],
        metadata: crate::DocumentMetadata {
            note_text: Some(source),
            default_page_dimensions: Some((360, height)),
            flow_page_padding: Some((0, 0)),
            ..Default::default()
        },
    };
    assert_public_parent_clips(&document, case);
}

#[test]
fn parsed_document_exports_preserve_the_captured_parent_clips() {
    let bytes = include_bytes!("../../tests/fixtures/native_body_table.sdocx");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "68d94a73f18d44e28ecc41de819171f01fbca2d497808680797a2ecc654e735a"
    );
    let document = crate::parse_bytes(bytes).unwrap();
    let source = document.metadata.note_text.as_ref().unwrap();
    assert_eq!(source.text, "\u{fffc}");
    assert_eq!(source.font_size, None);
    assert!(source.spans.is_empty());
    assert!(source.text_sections.is_empty());
    let Some(crate::RichTextObjectContent::Table(table)) = source.object_spans[0].content.as_ref()
    else {
        panic!("captured source must contain the declared table");
    };
    assert_eq!(table.style.content_bbox, Some(table.bbox));
    assert_eq!(document.metadata.flow_page_padding, Some((0, 0)));
    assert_eq!(document.metadata.default_page_dimensions, Some((360, 600)));
    assert_eq!(document.metadata.page_mode, Some(0));
    assert_eq!(document.metadata.document_density(), 1.0);
    let exclusions = text::PageExclusions::for_document(
        &document,
        TextSettings::from_document(&document.metadata),
    )
    .unwrap();
    assert_eq!(
        exclusions
            .line_bands()
            .iter()
            .map(|band| [band.top, band.bottom])
            .collect::<Vec<_>>(),
        [[-10.0, 10.0], [590.0, 610.0]]
    );
    assert_eq!(document.pages[0].width, 240);
    assert_eq!(document.pages[0].height, 600);
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-bodytext-one-page-obstacles.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "0a919c37edba851e952c112a8d0ac4f1c4a58f6856b5d118fd26ad75098d5da5"
    );
    let capture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "ordinary-one-page-native-page-padding")
        .unwrap();
    assert_public_parent_clips(&document, case);
}

#[test]
fn parsed_explicit_parent_font_preserves_geometry_without_certifying_its_style() {
    let bytes = include_bytes!("../../tests/fixtures/native_body_table_explicit_font.sdocx");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "5db6e8396fdf313badce5770b387c37626d223dfeed52c59cc54d8b368b29345"
    );
    let explicit = crate::parse_bytes(bytes).unwrap();
    let default = crate::parse_bytes(include_bytes!(
        "../../tests/fixtures/native_body_table.sdocx"
    ))
    .unwrap();
    let fonts = FontBook::default();
    let mut layouts = Vec::new();
    for (document, certified) in [(&default, true), (&explicit, false)] {
        let source = document.metadata.note_text.as_ref().unwrap();
        let original = source.clone();
        assert_eq!(source.font_size, (!certified).then_some(17.0));
        assert_eq!(source.spans.len(), usize::from(!certified));
        let settings = TextSettings::from_document(&document.metadata);
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(source, TextContext::Flow, settings);
        let layout = text::layout_flow_text(
            &styled,
            text::TextFrame {
                bbox: source.bbox,
                gravity: Some(0),
                exclusions: &[],
            },
            RenderTheme::for_canvas(false),
            &renderer,
        );
        assert_eq!(layout.native_object_entry.is_some(), certified);
        assert_eq!(source, &original);
        layouts.push(layout);
    }
    let [default, explicit] = layouts.as_slice() else {
        unreachable!()
    };
    assert_eq!(default.lines.len(), 1);
    assert_eq!(explicit.lines.len(), 1);
    assert_eq!(default.lines[0].baseline, explicit.lines[0].baseline);
    assert_eq!(default.lines[0].bottom, explicit.lines[0].bottom);
    assert_eq!(
        default.lines[0].line.advance,
        explicit.lines[0].line.advance
    );
}

fn assert_public_parent_clips(document: &Document, case: &serde_json::Value) {
    let original = document.metadata.note_text.clone();
    let layout = crate::layout_document(document);
    let page = render_layout_page_svg(document, &layout, 0, &Default::default()).unwrap();
    let fonts = FontBook::default();
    let mut cache = DocumentTextCache::default();
    let cached = cache
        .render_layout_page_svg(document, &layout, 0, &Default::default(), &fonts)
        .unwrap();
    assert_eq!(page.svg, cached.svg);
    assert_eq!(page.object_diagnostics, cached.object_diagnostics);
    assert_eq!(cache.plans.len(), 1);
    let parent = cache.plans[0]
        .1
        .layout
        .native_object_entry
        .expect("public source must retain the native parent geometry");
    let expected_parent = &case["writer"]["entries"][0];
    for (actual, expected) in [
        (
            parent.position.as_slice(),
            &expected_parent["position_bits"],
        ),
        (
            parent.layout.as_slice(),
            &expected_parent["layout_rect_bits"],
        ),
        (parent.ink.as_slice(), &expected_parent["ink_rect_bits"]),
    ] {
        assert_eq!(
            actual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            expected
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect::<Vec<_>>()
        );
    }
    let [line] = cache.plans[0].1.layout.lines.as_slice() else {
        panic!("captured parent has one measured line");
    };
    let [object] = line.line.objects.as_slice() else {
        panic!("captured parent has one table callback");
    };
    let Some(Ok(embedded::PreparedObject::Table(prepared))) = object.prepared.as_ref() else {
        panic!("captured callback must preserve prepared table geometry");
    };
    let callback = case["writer"]["object_size_updates"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(
        prepared.measured_bbox,
        captured_rect(&callback["output_rect_bits"])
    );
    for (cell, expected) in prepared.rows.iter().flat_map(|row| &row.cells).zip(
        case["states"].as_array().unwrap().last().unwrap()["cells"]
            .as_array()
            .unwrap(),
    ) {
        assert_eq!(cell.frame, captured_rect(&expected["frame_bits"]));
    }
    assert!(
        page.object_diagnostics.is_empty(),
        "{:?}",
        page.object_diagnostics
    );
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let groups = xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), 6);
    let captured = case["writer"]["writer_after"]["cells"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|cell| {
            cell["runs"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |run| (cell["slot"].as_u64().unwrap() as usize, run))
        })
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), captured.len());
    for (group, (slot, expected)) in groups.iter().zip(captured) {
        let local =
            case["states"].as_array().unwrap().last().unwrap()["cells"][slot]["emitted"]["runs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|run| run["range_inclusive"] == expected["range_inclusive"])
                .unwrap();
        let coordinate =
            |value: &serde_json::Value| f64::from(f32::from_bits(value.as_u64().unwrap() as u32));
        let caller = &expected["caller_origin_bits"];
        for (axis, attribute) in [(0, "x"), (1, "y")] {
            assert_eq!(
                group.attribute(attribute).unwrap().parse::<f64>().unwrap(),
                coordinate(&local["origin_bits"][axis]) + coordinate(&caller[axis])
            );
        }
        let positions = group
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .flat_map(|node| node.attribute("x").unwrap().split_whitespace())
            .map(|position| position.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        let expected_positions = local["position_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|position| coordinate(position) + coordinate(&caller[0]))
            .collect::<Vec<_>>();
        assert_eq!(positions, expected_positions);
        let clips = group
            .ancestors()
            .filter_map(|node| node.attribute("clip-path"))
            .collect::<Vec<_>>();
        let expected_count = 1 + usize::from(expected["clip_selected"].as_bool().unwrap());
        assert_eq!(clips.len(), expected_count);
        if expected_count == 2 {
            let id = clips[0]
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let rectangle = xml
                .descendants()
                .find(|node| node.attribute("id") == Some(id))
                .unwrap()
                .descendants()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            let bounds = captured_rect(&expected["world_clip_bits"]);
            for (attribute, expected) in [
                ("x", bounds.x_min),
                ("y", bounds.y_min),
                ("width", bounds.x_max - bounds.x_min),
                ("height", bounds.y_max - bounds.y_min),
            ] {
                assert_eq!(
                    rectangle
                        .attribute(attribute)
                        .unwrap()
                        .parse::<f64>()
                        .unwrap(),
                    expected
                );
            }
        }
    }
    assert!(!page.svg.contains("<image"));
    #[cfg(feature = "pdf")]
    {
        let pdf =
            crate::render_document_pdf(document, &Default::default(), &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&pdf).unwrap();
        let page = pdf.get_pages().keys().copied().next().unwrap();
        let source = pdf.extract_text(&[page]).unwrap();
        for text in ["AV", "To", "last", "A\u{301}B"] {
            assert!(source.contains(text), "{source:?}");
        }
    }
    assert_eq!(document.metadata.note_text, original);
}
