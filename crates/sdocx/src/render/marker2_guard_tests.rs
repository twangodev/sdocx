use crate::{
    BoundingBox, Color, Document, Page, Point, RenderOptions, Stroke, StrokeProperties,
    StrokeRendering, StrokeStyle,
};

fn stroke() -> Stroke {
    Stroke {
        rendering: Some(StrokeRendering {
            metadata: None,
            pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Marker2".into()),
            advanced_settings: Some(String::new()),
            tool_type_raw: 2,
            properties: StrokeProperties {
                compressed: true,
                replay_only: false,
                stylus_channels: false,
                eraser: false,
                fixed_width: false,
                millisecond_timestamps: false,
                top_layer_pen: false,
                alpha_lock: false,
                binary_added: true,
                generated: false,
                fixed_opacity: false,
                rainbow_effect: false,
                straighten: false,
                reveal_mode: false,
            },
            style: StrokeStyle {
                color_argb: Some(0x80994422),
                pen_size: Some(2.0),
                stroke_type_raw: Some(0),
                ..Default::default()
            },
        }),
        bbox: BoundingBox::default(),
        points: vec![
            Point { x: 20., y: 20. },
            Point { x: 30., y: 20. },
            Point { x: 40., y: 30. },
            Point { x: 50., y: 30. },
        ],
        pressures: vec![1.; 4],
        timestamps: vec![],
        tilts: vec![],
        orientations: vec![],
        color: Some(Color {
            r: 0x99,
            g: 0x44,
            b: 0x22,
        }),
        pen_width: 2.,
    }
}
fn document(strokes: Vec<Stroke>) -> Document {
    Document {
        pages: vec![Page {
            uuid: "marker2-guard".into(),
            width: 256,
            height: 256,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: strokes.into_iter().map(Into::into).collect(),
        }],
        metadata: Default::default(),
    }
}
fn opts(scale: Option<u32>) -> RenderOptions {
    RenderOptions {
        marker2_output_scale: scale,
        ..Default::default()
    }
}
fn svg(doc: &Document, scale: Option<u32>) -> String {
    crate::render_document_svg(doc, &opts(scale))[0].svg.clone()
}

#[cfg(feature = "serde")]
#[test]
fn marker2_scale_serialization_preserves_opt_in_and_legacy_defaults() {
    assert_eq!(RenderOptions::default().marker2_output_scale, None);
    let mut legacy = serde_json::to_value(RenderOptions::default()).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("marker2_output_scale");
    assert_eq!(
        serde_json::from_value::<RenderOptions>(legacy)
            .unwrap()
            .marker2_output_scale,
        None
    );
    for scale in [None, Some(2), Some(3)] {
        let serialized = serde_json::to_value(opts(scale)).unwrap();
        assert_eq!(serialized["marker2_output_scale"], serde_json::json!(scale));
        let decoded: RenderOptions = serde_json::from_value(serialized).unwrap();
        assert_eq!(decoded.marker2_output_scale, scale);
    }
}
#[test]
fn marker2_guard_replay_slice_and_invalid() {
    let original = stroke();
    let doc = document(vec![original.clone()]);
    assert_ne!(
        svg(&doc, None),
        svg(&doc, Some(2)),
        "positive control must select candidate"
    );
    for scale in [Some(0), Some(1), Some(3), Some(u32::MAX)] {
        assert_eq!(svg(&doc, None), svg(&doc, scale), "scale {scale:?}");
    }
    for count in [2, 3, 5, 6, 7, 16] {
        let mut s = original.clone();
        s.points.truncate(count);
        while s.points.len() < count {
            s.points.push(*s.points.last().unwrap());
        }
        let d = document(vec![s]);
        assert_ne!(svg(&d, None), svg(&d, Some(2)), "source count {count}");
    }
    let layout = crate::layout_document(&doc);
    let replay = |scale| {
        crate::render_layout_page_replay_svg(&doc, &layout, 0, &opts(scale))
            .unwrap()
            .svg
    };
    assert_eq!(replay(None), replay(Some(2)), "replay fallback");
    let request = super::PageSceneRequest::sliced(&layout.pages[0], 0, 96, false).unwrap();
    let slice = |scale| {
        let mut cache = super::DocumentTextCache::default();
        cache
            .render_layout_page_scene_requests(
                &doc,
                &[request],
                &opts(scale),
                &super::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page
            .svg
    };
    assert_eq!(slice(None), slice(Some(2)), "slice fallback");
    let mut cases = Vec::new();
    let mut single = original.clone();
    single.points.truncate(1);
    cases.push(single);
    for setting in [None, Some("2;"), Some("invalid")] {
        let mut s = original.clone();
        s.rendering.as_mut().unwrap().advanced_settings = setting.map(str::to_owned);
        cases.push(s);
    }
    for tool in [0, 1, 3] {
        let mut s = original.clone();
        s.rendering.as_mut().unwrap().tool_type_raw = tool;
        cases.push(s);
    }
    let mut s = original.clone();
    s.rendering.as_mut().unwrap().properties.compressed = false;
    cases.push(s);
    let mut s = original.clone();
    s.rendering.as_mut().unwrap().style.stroke_type_raw = Some(1);
    cases.push(s);
    let mut s = original.clone();
    s.rendering.as_mut().unwrap().style.pen_size = Some(4.);
    s.pen_width = 4.;
    cases.push(s);
    let mut s = original.clone();
    s.points[0].x = 300.;
    cases.push(s);
    for (i, s) in cases.into_iter().enumerate() {
        let d = document(vec![s]);
        assert_eq!(svg(&d, None), svg(&d, Some(2)), "guard case {i}");
        assert_eq!(
            crate::render_document_pdf(&d, &opts(None), &Default::default()).unwrap(),
            crate::render_document_pdf(&d, &opts(Some(2)), &Default::default()).unwrap(),
            "PDF guard case {i}"
        );
    }
}
fn svg_fragment(doc: &Document, scale: Option<u32>) -> String {
    let empty = svg(&document(vec![]), scale);
    let rendered = svg(doc, scale);
    let closing = "</svg>";
    let prefix = empty.strip_suffix(closing).expect("empty SVG closing tag");
    rendered
        .strip_prefix(prefix)
        .and_then(|s| s.strip_suffix(closing))
        .expect("stroke SVG must share the empty page wrapper")
        .to_owned()
}
#[test]
fn marker2_multistroke_preserves_unsupported_neighbor() {
    let supported = stroke();
    let mut unsupported = stroke();
    unsupported.rendering.as_mut().unwrap().advanced_settings = Some("2;".into());
    for p in &mut unsupported.points {
        p.y += 40.;
    }
    let supported_doc = document(vec![supported.clone()]);
    let unsupported_doc = document(vec![unsupported.clone()]);
    let baseline_supported = svg_fragment(&supported_doc, None);
    let candidate_supported = svg_fragment(&supported_doc, Some(2));
    let baseline_unsupported = svg_fragment(&unsupported_doc, None);
    assert!(!baseline_supported.is_empty() && !baseline_unsupported.is_empty());
    assert_ne!(
        baseline_supported, candidate_supported,
        "positive control must select candidate"
    );
    assert_eq!(
        baseline_unsupported,
        svg_fragment(&unsupported_doc, Some(2)),
        "unsupported standalone fallback"
    );
    for (strokes, expected_baseline, expected_candidate) in [
        (
            vec![supported.clone(), unsupported.clone()],
            format!("{baseline_supported}{baseline_unsupported}"),
            format!("{candidate_supported}{baseline_unsupported}"),
        ),
        (
            vec![unsupported, supported],
            format!("{baseline_unsupported}{baseline_supported}"),
            format!("{baseline_unsupported}{candidate_supported}"),
        ),
    ] {
        let combined = document(strokes);
        assert_eq!(
            svg_fragment(&combined, None),
            expected_baseline,
            "baseline source order"
        );
        assert_eq!(
            svg_fragment(&combined, Some(2)),
            expected_candidate,
            "candidate and unsupported fallback in source order"
        );
    }
}

#[test]
fn marker2_page_budget_atomic_fallback_and_reset() {
    for width in [2., 3.] {
        let mut s = stroke();
        s.pen_width = width;
        s.rendering.as_mut().unwrap().style.pen_size = Some(width);
        marker2_page_budget_for_stroke(s);
    }
}

fn marker2_page_budget_for_stroke(s: Stroke) {
    let single = document(vec![s.clone()]);
    let candidate = svg_fragment(&single, Some(2));
    let fallback = svg_fragment(&single, None);
    assert_ne!(candidate, fallback);
    let subpaths = candidate.matches('M').count();
    assert_eq!(subpaths % 4, 0);
    let cost = subpaths / 4 * 4 * 128;
    assert!(cost > 0 && cost < 65_536);
    let admitted = 65_536 / cost;
    assert!(admitted < 100);
    let mut small = s.clone();
    for (i, p) in small.points.iter_mut().enumerate() {
        p.x = 20.0 + i as f64 * 0.5;
        p.y = 20.0;
    }
    let small_candidate = svg_fragment(&document(vec![small.clone()]), Some(2));
    let small_fallback = svg_fragment(&document(vec![small.clone()]), None);
    assert_ne!(small_candidate, small_fallback);
    let small_cost = small_candidate.matches('M').count() * 128;
    assert!(small_cost <= 65_536 - admitted * cost);
    assert_eq!(65_536 % small_cost, 0);
    let exact = 65_536 / small_cost;
    let boundary = document(vec![small.clone(); exact + 1]);
    assert_eq!(
        svg_fragment(&boundary, Some(2)),
        format!("{}{}", small_candidate.repeat(exact), small_fallback)
    );
    fn pdf_fragment(doc: &Document, scale: Option<u32>) -> (Vec<u8>, lopdf::Dictionary) {
        let bytes = crate::render_document_pdf(doc, &opts(scale), &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let page = *pdf.get_pages().values().next().unwrap();
        let content = pdf.get_page_content(page).unwrap();
        let marker = b"\nf\nQ\n";
        let start = content
            .windows(marker.len())
            .position(|w| w == marker)
            .unwrap()
            + marker.len();
        let (direct, inherited) = pdf.get_page_resources(page).unwrap();
        let resources = direct
            .or_else(|| inherited.iter().find_map(|id| pdf.get_dictionary(*id).ok()))
            .unwrap();
        let states = pdf
            .dereference(resources.get(b"ExtGState").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        let mut resolved = lopdf::Dictionary::new();
        for (name, state) in states.iter() {
            resolved.set(name.clone(), pdf.dereference(state).unwrap().1.clone());
        }
        (
            content[start..]
                .strip_suffix(b"\nQ\n")
                .or_else(|| content[start..].strip_suffix(b"\nQ"))
                .unwrap()
                .to_vec(),
            resolved,
        )
    }
    let (pdf_candidate, candidate_states) = pdf_fragment(&document(vec![small.clone()]), Some(2));
    let (pdf_fallback, fallback_states) = pdf_fragment(&document(vec![small.clone()]), None);
    let (pdf_boundary, boundary_states) = pdf_fragment(&boundary, Some(2));
    let mut expected_pdf = Vec::new();
    for _ in 0..exact {
        if !expected_pdf.is_empty() {
            expected_pdf.push(b'\n');
        }
        expected_pdf.extend_from_slice(&pdf_candidate);
    }
    expected_pdf.push(b'\n');
    expected_pdf.extend_from_slice(&pdf_fallback);
    assert_eq!(
        boundary_states.get(b"g0").unwrap(),
        candidate_states.get(b"g0").unwrap()
    );
    assert_eq!(
        boundary_states.get(b"g1").unwrap(),
        fallback_states.get(b"g0").unwrap()
    );
    let normalized = String::from_utf8(pdf_boundary)
        .unwrap()
        .replace("/g1 gs", "/g0 gs");
    assert_eq!(
        normalized.as_bytes(),
        expected_pdf,
        "PDF ordered candidate and fallback operators (graphics state names normalized)"
    );
    let mut mixed = vec![s.clone(); admitted + 1];
    mixed.push(small);
    assert_eq!(
        svg_fragment(&document(mixed), Some(2)),
        format!(
            "{}{}{}",
            candidate.repeat(admitted),
            fallback,
            small_candidate
        )
    );
    let page = document(vec![s.clone(); admitted + 2]);
    let expected = format!("{}{}", candidate.repeat(admitted), fallback.repeat(2));
    assert_eq!(svg_fragment(&page, Some(2)), expected);
    let mut two_pages = page.clone();
    two_pages.pages.push(page.pages[0].clone());
    let rendered = crate::render_document_svg(&two_pages, &opts(Some(2)));
    assert_eq!(rendered[0].svg, rendered[1].svg);
    let pdf = crate::render_document_pdf(&two_pages, &opts(Some(2)), &Default::default()).unwrap();
    let baseline_pdf =
        crate::render_document_pdf(&two_pages, &opts(None), &Default::default()).unwrap();
    let baseline = lopdf::Document::load_mem(&baseline_pdf).unwrap();
    let baseline_first = *baseline.get_pages().values().next().unwrap();
    let parsed = lopdf::Document::load_mem(&pdf).unwrap();
    let pages = parsed.get_pages();
    let content: Vec<_> = pages
        .values()
        .map(|id| parsed.get_page_content(*id).unwrap())
        .collect();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0], content[1]);
    assert_ne!(
        content[0],
        baseline.get_page_content(baseline_first).unwrap(),
        "PDF must select candidate"
    );
}

// Retained synthetic host-Mesa controls, not device or saved-page parity.
// This checks SVG pixels; historical MuPDF/PDFium measurements are not rerun here.
#[test]
fn marker2_both_widths_improve_native_overlap_rgb() {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/marker2-scale-investigation");
    let read_pinned = |path: &str, expected: &str| {
        let bytes = std::fs::read(root.join(path)).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), expected, "{path}");
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()
    };
    let decode = |value: &serde_json::Value| {
        let hex = value.as_str().unwrap();
        assert_eq!(hex.len(), 8);
        f32::from_le_bytes(std::array::from_fn(|i| {
            u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()
        })) as f64
    };
    for (source_path, source_hash, native_dir, receipt_hash) in [
        (
            "v61/bend/w2-translated-a255.json",
            "8700cfda6f47d1e5c9f06a40106f1ea73aa50e803138a9a02a37f85946e81047",
            "expanded/independent-lifecycle",
            "2897535340da643d019f42590a25b0a2e7b2e850ca1ebd5369ad3debb5ec4a88",
        ),
        (
            "fringe-sweep/bend/w3-translated-a255.json",
            "5e5162b1a1263ec1dd42969cf2ce634c264612b1830e2604e6be4f9d4d2682f1",
            "width3-controls",
            "2b44034dd9cbf29677c8ebdca86f24851addbd37fa4ffce723bc0136f3baca21",
        ),
    ] {
        let source = read_pinned(source_path, source_hash);
        for (name, channels, color_type) in [
            ("composite", 4usize, png::ColorType::Rgba),
            ("mask", 1usize, png::ColorType::Grayscale),
        ] {
            let image = &source[name];
            let encoded = image["embedded_png_base64"].as_str().unwrap();
            assert!(encoded.len() <= 16_384);
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                image["png_sha256"].as_str().unwrap()
            );
            let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
                .read_info()
                .unwrap();
            assert_eq!((reader.info().width, reader.info().height), (256, 256));
            assert_eq!(reader.info().color_type, color_type);
            assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
            assert_eq!(reader.output_buffer_size().unwrap(), 256 * 256 * channels);
            let mut pixels = vec![0; 256 * 256 * channels];
            let frame = reader.next_frame(&mut pixels).unwrap();
            assert_eq!(frame.buffer_size(), pixels.len());
            let mut digest = Sha256::new();
            for row in pixels.chunks_exact(256 * channels).rev() {
                digest.update(row);
            }
            assert_eq!(
                format!("{:x}", digest.finalize()),
                image["raw_gl_sha256"].as_str().unwrap()
            );
        }
        let receipt = read_pinned(&format!("{native_dir}/receipt.json"), receipt_hash);
        assert!(receipt["nonzero_gl_errors"].as_array().unwrap().is_empty());
        for field in ["object_strokes", "drawings", "pens", "renderers"] {
            assert_eq!(receipt[format!("independent_{field}")], true);
        }
        assert_eq!(
            receipt["native_vector_sha256"],
            receipt["second_native_vector_sha256"]
        );
        assert_eq!(
            source["source"]["source_callback_vector_sha256"],
            receipt["native_vector_sha256"]
        );
        let mut native = Vec::new();
        for frame in ["first", "second"] {
            let compressed =
                std::fs::read(root.join(format!("{native_dir}/{frame}.rgba.zlib"))).unwrap();
            native.clear();
            flate2::read::ZlibDecoder::new(compressed.as_slice())
                .read_to_end(&mut native)
                .unwrap();
            assert_eq!(native.len(), 256 * 256 * 4);
            assert_eq!(
                format!("{:x}", Sha256::digest(&native)),
                receipt[format!("{frame}_sha256")].as_str().unwrap()
            );
        }
        let input = &source["native_input"]["source_case"];
        let translation = &source["native_input"]["translation_f32"];
        for i in 0..2 {
            assert_eq!(translation[i], receipt["matrices"][0][12 + i]);
        }
        let dx = translation[0].as_f64().unwrap();
        let dy = translation[1].as_f64().unwrap();
        let mut s = stroke();
        let rendering = s.rendering.as_mut().unwrap();
        assert_eq!(input["pen_name"].as_str(), rendering.pen_name.as_deref());
        assert_eq!(input["advanced_setting"], "");
        assert_eq!(input["tool_type"], 2);
        assert_eq!(input["curve_enabled"], true);
        assert_eq!(input["stroke_type"], 0);
        s.pen_width = decode(&input["pen_size_f32_bits"]) as f32;
        rendering.style.pen_size = Some(s.pen_width);
        s.points = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| Point {
                x: ((decode(&p[0]) + dx / 2.) as f32) as f64,
                y: ((128. - dy / 2. - decode(&p[1])) as f32) as f64,
            })
            .collect();
        let channel = |key: &str| input[key].as_array().unwrap().iter().map(&decode).collect();
        s.pressures = channel("pressure_f32_bits");
        s.tilts = channel("tilt_f32_bits");
        s.orientations = channel("orientation_f32_bits");
        s.timestamps = input["timestamps_i32"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect();
        s.bbox = BoundingBox {
            x_min: 10.,
            y_min: 20.,
            x_max: 34.,
            y_max: 25.,
        };
        let mut second = s.clone();
        for point in &mut second.points {
            point.x += 2.;
            point.y -= 1.5;
        }
        let mut doc = document(vec![s, second]);
        doc.metadata.page_dimensions = Some((256, 256));
        doc.pages[0].content_bbox = BoundingBox {
            x_min: 0.,
            y_min: 0.,
            x_max: 256.,
            y_max: 256.,
        };
        let error = |scale| {
            let tree = resvg::usvg::Tree::from_str(&svg(&doc, scale), &Default::default()).unwrap();
            let mut pixels = resvg::tiny_skia::Pixmap::new(512, 512).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(2., 2.),
                &mut pixels.as_mut(),
            );
            assert_eq!(&pixels.data()[..4], &[252, 252, 252, 255]);
            let mut sum = 0u64;
            for y in 0..256 {
                for x in 0..256 {
                    let actual = &pixels.data()[(y * 512 + x) * 4..][..4];
                    assert_eq!(actual[3], 255);
                    let expected = &native[((255 - y) * 256 + x) * 4..][..4];
                    // Integer rounding matches NumPy rint here: denominator 255
                    // is odd, so this fraction can never be exactly half-integral.
                    let background = (252u32 * (255 - expected[3] as u32) + 127) / 255;
                    for channel in 0..3 {
                        sum +=
                            (actual[channel] as i32 - expected[channel] as i32 - background as i32)
                                .unsigned_abs() as u64;
                    }
                }
            }
            sum
        };
        let baseline = error(None);
        let candidate = error(Some(2));
        assert!(
            candidate < baseline,
            "{native_dir}: candidate {candidate}, baseline {baseline}"
        );
    }
}
