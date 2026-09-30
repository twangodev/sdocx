#![cfg(all(feature = "render", feature = "serde"))]

#[path = "support/shape_text.rs"]
mod shape_text;

use sdocx::{
    NativeShape, RichTextParagraph, RichTextParagraphType, RichTextSpan, RichTextSpanType,
    TextDiagnosticKind,
};
use shape_text::{assert_lines, bounds, document, render, shape, text};

#[derive(Clone, Copy)]
enum Command {
    Move([f64; 2]),
    Line([f64; 2]),
    Cubic([[f64; 2]; 3]),
    Close,
}

fn path(commands: &[Command]) -> Vec<u8> {
    let mut bytes = u32::try_from(commands.len())
        .unwrap()
        .to_le_bytes()
        .to_vec();
    for command in commands {
        let (verb, coordinates): (u8, &[f64]) = match command {
            Command::Move(point) => (1, point),
            Command::Line(point) => (2, point),
            Command::Cubic(points) => (4, points.as_flattened()),
            Command::Close => (6, &[]),
        };
        bytes.push(verb);
        for coordinate in coordinates {
            bytes.extend(coordinate.to_le_bytes());
        }
    }
    bytes
}

fn polygon(points: &[[f64; 2]]) -> Vec<u8> {
    let mut commands = vec![Command::Move(points[0])];
    commands.extend(points[1..].iter().copied().map(Command::Line));
    commands.push(Command::Close);
    path(&commands)
}

fn triangle() -> NativeShape {
    let mut shape = shape(2, text("A"));
    shape.path_data = polygon(&[[100.0, 0.0], [200.0, 100.0], [0.0, 100.0]]);
    shape
}

fn rounded(width: f64, source: &str) -> NativeShape {
    let mut shape = shape(5, text(source));
    shape.geometry_bbox = bounds(0.0, 0.0, width, 100.0);
    shape.path_data = path(&[
        Command::Move([10.0, 0.0]),
        Command::Line([width - 10.0, 0.0]),
        Command::Cubic([[width - 5.0, 0.0], [width, 5.0], [width, 10.0]]),
        Command::Line([width, 90.0]),
        Command::Cubic([[width, 95.0], [width - 5.0, 100.0], [width - 10.0, 100.0]]),
        Command::Line([10.0, 100.0]),
        Command::Cubic([[5.0, 100.0], [0.0, 95.0], [0.0, 90.0]]),
        Command::Line([0.0, 10.0]),
        Command::Cubic([[0.0, 5.0], [5.0, 0.0], [10.0, 0.0]]),
        Command::Close,
    ]);
    shape
}

fn assert_shape(shape: NativeShape, expected: &[(&str, f64, f64)]) {
    let doc = document(shape, 1);
    let original = serde_json::to_value(&doc).unwrap();
    for replay in [false, true] {
        let page = render(&doc, replay);
        assert_lines(&page, expected);
        assert!(
            page.text_diagnostics.is_empty(),
            "{:?}",
            page.text_diagnostics
        );
        assert_eq!(render(&doc, replay), page);
    }
    assert_eq!(serde_json::to_value(&doc).unwrap(), original);
}

#[test]
fn triangle_uses_saved_apex_ratio_and_actual_vertical_flip() {
    assert_shape(triangle(), &[("A", 50.0, 60.0)]);
    let mut asymmetric = triangle();
    asymmetric.path_data = polygon(&[[50.0, 0.0], [200.0, 100.0], [0.0, 100.0]]);
    asymmetric.metadata.flip_enabled = true;
    assert_shape(asymmetric.clone(), &[("A", 25.0, 60.0)]);
    asymmetric.vertical_flip = true;
    asymmetric.metadata.flip_enabled = false;
    assert_shape(asymmetric, &[("A", 25.0, 10.0)]);
}

#[test]
fn restored_triangle_controls_override_the_path_in_stored_order() {
    let mut shape = triangle();
    shape.rotation_degrees = 90.0;
    shape.path_data = polygon(&[[150.0, 50.0], [50.0, 150.0], [50.0, -50.0]]);
    shape.control_points = vec![[150.0, 150.0], [150.0, 0.0]];
    assert_shape(shape.clone(), &[("A", 25.0, 60.0)]);
    let page = render(&document(shape, 1), false);
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    assert!(
        xml.descendants()
            .any(|node| node.attribute("transform") == Some("rotate(90.00 100.00 50.00)"))
    );
}

#[test]
fn right_triangle_uses_vertex_orientation_and_ignores_saved_controls() {
    for (points, expected) in [
        ([[0.0, 0.0], [200.0, 100.0], [0.0, 100.0]], (17.5, 68.75)),
        ([[200.0, 100.0], [0.0, 0.0], [200.0, 0.0]], (82.5, 18.75)),
    ] {
        let mut shape = shape(3, text("A"));
        shape.path_data = polygon(&points);
        shape.control_points = vec![[200.0, 0.0], [-100.0, 900.0]];
        assert_shape(shape, &[("A", expected.0, expected.1)]);
    }
}

#[test]
fn finite_extreme_triangle_controls_keep_native_endpoint_clamping() {
    for (control, x) in [(f32::MAX, 100.0), (-f32::MAX, 0.0)] {
        let mut shape = triangle();
        shape.control_points = vec![[f64::from(control), 0.0]];
        assert_shape(shape, &[("A", x, 60.0)]);
    }
}

#[test]
fn right_triangle_horizontal_reload_obeys_the_saved_flip_and_rotation() {
    for (flip, x) in [(false, 82.5), (true, 17.5)] {
        let mut shape = shape(3, text("A"));
        shape.rotation_degrees = 90.0;
        shape.horizontal_flip = flip;
        shape.path_data = polygon(&[[150.0, -50.0], [150.0, 150.0], [50.0, 150.0]]);
        assert_shape(shape, &[("A", x, 18.75)]);
    }
}

#[test]
fn pentagon_source_vertices_select_the_apex_side_and_ignore_controls() {
    // Native LoadPath 0x21b2e0 and Update 0x21b444 retain the vertex cut.
    let points = [
        [100.0, 0.0],
        [200.0, 37.5],
        [162.5, 100.0],
        [37.5, 100.0],
        [0.0, 37.5],
    ];
    let mut shape = shape(11, text("A"));
    shape.path_data = polygon(&points);
    shape.control_points = vec![[1000.0, -900.0]];
    assert_shape(shape.clone(), &[("A", 37.5, 35.0)]);
    shape.path_data = polygon(&points.map(|[x, y]| [200.0 - x, 100.0 - y]));
    assert_shape(shape, &[("A", 37.5, 10.0)]);
}

#[test]
fn hexagon_source_cut_and_ordered_controls_retain_native_flip_reconstruction() {
    // Native margin producer 0x2174f0: side cut 30 gives insets 29.5/14.75.
    // Controls 0x2169b8 rebuild vertices after each point using both saved flips.
    let mut shape = shape(6, text("A"));
    shape.path_data = polygon(&[
        [30.0, 0.0],
        [170.0, 0.0],
        [200.0, 50.0],
        [170.0, 100.0],
        [30.0, 100.0],
        [0.0, 50.0],
    ]);
    assert_shape(shape.clone(), &[("A", 29.5, 24.75)]);
    shape.horizontal_flip = true;
    shape.vertical_flip = true;
    shape.control_points = vec![[40.0, -900.0], [150.0, 900.0]];
    assert_shape(shape, &[("A", 37.5, 28.75)]);
}

#[test]
fn rounded_source_radius_and_replayed_flip_controls_determine_the_frame() {
    // Native F32 radius margin: R10 + (R10 * -sqrt(2)) * .5 = 2.9289321899414062.
    assert_shape(
        rounded(200.0, "A"),
        &[("A", 2.9289321899414062, 12.928_932_189_941_406)],
    );
    let mut shape = rounded(200.0, "A");
    shape.horizontal_flip = true;
    shape.vertical_flip = true;
    shape.control_points = vec![[12.0, 0.0], [180.0, -100.0]];
    assert_shape(shape, &[("A", 5.8578643798828125, 15.857_864_379_882_813)]);
}

#[test]
fn signed_polygon_insets_expand_the_retained_frame_without_fallback() {
    for (kind, points, expected) in [
        (
            11,
            vec![
                [100.0, 0.0],
                [200.0, 37.5],
                [162.5, 100.0],
                [-5.0, 100.0],
                [0.0, 37.5],
            ],
            ("A", -5.0, 35.0),
        ),
        (
            6,
            vec![
                [-50.0, 0.0],
                [170.0, 0.0],
                [200.0, 50.0],
                [170.0, 100.0],
                [30.0, 100.0],
                [0.0, 50.0],
            ],
            ("A", -2.5, 8.75),
        ),
    ] {
        let mut shape = shape(kind, text("A"));
        shape.path_data = polygon(&points);
        assert_shape(shape, &[expected]);
    }
}

#[test]
fn rounded_gravity_alignment_and_wrap_use_native_ceiled_inset_dimensions() {
    // R10 gives inset width 194.1421/height 94.1421; native measurement is 195x95.
    for (gravity, y) in [(1, 53.678_932_189_941_406), (2, 94.428_932_189_941_4)] {
        let mut shape = rounded(200.0, "A");
        shape.text.as_mut().unwrap().gravity = Some(gravity);
        assert_shape(shape, &[("A", 2.9289321899414062, y)]);
    }
    for (alignment, x) in [(2_u32, 90.800_025_939_941_4), (1, 178.671_119_689_941_4)] {
        let mut shape = rounded(200.0, "ABC");
        shape
            .text
            .as_mut()
            .unwrap()
            .paragraphs
            .push(RichTextParagraph {
                kind: RichTextParagraphType::Alignment,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: alignment.to_le_bytes().to_vec(),
            });
        assert_shape(shape, &[("ABC", x, 12.928_932_189_941_406)]);
    }
    assert_shape(
        rounded(25.0, "ABC"),
        &[("ABC", 2.9289321899414062, 12.928_932_189_941_406)],
    );
    assert_shape(
        rounded(24.0, "ABC"),
        &[
            ("AB", 2.9289321899414062, 12.928_932_189_941_406),
            ("C", 2.9289321899414062, 26.428_932_189_941_406),
        ],
    );
}

#[test]
fn path_dependent_shape_background_clips_to_original_geometry() {
    let mut shape = triangle();
    shape.text.as_mut().unwrap().spans.push(RichTextSpan {
        kind: RichTextSpanType::BackgroundColor,
        start_utf16: 0,
        end_utf16: 1,
        expand: true,
        payload: 0xffff0011_u32.to_le_bytes().to_vec(),
    });
    let doc = document(shape, 1);
    for replay in [false, true] {
        let page = render(&doc, replay);
        assert_lines(&page, &[("A", 50.0, 60.0)]);
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let rectangle = xml
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#ff0011"))
            .unwrap();
        let clip = rectangle
            .ancestors()
            .find_map(|node| node.attribute("clip-path"))
            .unwrap();
        let id = clip
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let clip = xml
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap();
        let bounds = clip
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert_eq!(
            ["x", "y", "width", "height"].map(|attribute| bounds
                .attribute(attribute)
                .unwrap()
                .parse::<f64>()
                .unwrap()),
            [0.0, 0.0, 200.0, 100.0]
        );
        assert!(page.text_diagnostics.is_empty());
    }
}

#[test]
fn missing_noncanonical_and_unknown_paths_report_explicit_saved_frame_recovery() {
    let unsupported = [
        Vec::new(),
        polygon(&[[100.0, 0.0], [0.0, 100.0]]),
        vec![1, 0, 0, 0, 0xff],
    ];
    for kind in [2, 3, 5, 6, 11] {
        for path in &unsupported {
            let mut shape = shape(kind, text("A"));
            shape.path_data = path.clone();
            for replay in [false, true] {
                let page = render(&document(shape.clone(), 1), replay);
                assert_lines(&page, &[("A", 71.0, 83.0)]);
                assert_eq!(page.text_diagnostics.len(), 1);
                assert_eq!(
                    page.text_diagnostics[0].kind,
                    TextDiagnosticKind::UnsupportedTextFrame
                );
            }
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn native_path_frames_keep_unicode_selectable_in_vector_pdf() {
    let mut triangle = triangle();
    triangle.text.as_mut().unwrap().text = "Aé".into();
    for shape in [triangle, rounded(200.0, "Aé")] {
        let doc = document(shape, 1);
        for replay in [false, true] {
            let page = render(&doc, replay);
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("tspan"))
                    .filter_map(|node| node.text())
                    .collect::<String>(),
                "Aé"
            );
            let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            assert_eq!(
                pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
                "Aé"
            );
            assert!(
                !pdf.objects
                    .values()
                    .any(|object| object
                        .as_stream()
                        .is_ok_and(|stream| stream.dict.get(b"Subtype").is_ok_and(|value| value
                            .as_name()
                            .is_ok_and(|name| name == b"Image"))))
            );
            assert!(pdf.objects.values().any(|object| {
                object
                    .as_dict()
                    .is_ok_and(|dict| dict.has(b"FontFile2") || dict.has(b"FontFile3"))
            }));
        }
    }
}
