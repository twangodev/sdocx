//! In-memory native Fountain V17 geometry, framebuffer and PDF regressions.
//! The external capture-receipt audit and artifact exporter have been retired;
//! the compact fixture retains the original producer and Mesa receipt identities.

use super::super::vector::{ColorValue, Rectangle, Styled, Svg, ViewBox};
use super::{Group, Paint, Scene, Transform, V16Plan, render};
use crate::{Point, Stroke};
use base64::Engine as _;
use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::io::Read;

const WIDTH: u32 = 128;
const HEIGHT: u32 = 96;
const RECORD_BYTES: usize = 40;
const PRODUCER_SHA256: &str = "223a7fb07ca15b1d4be44f2d8c3e15ca7a34008fe5bf4b9d5659b91463fab9eb";
const PRODUCER_RECEIPT_SHA256: &str =
    "387e473f18671a3c33536bf3dfaa12dedcc2e910caa45246aafea2280d5772f3";
const MESA_SOURCE_RECEIPT_SHA256: &str =
    "d52ebbcc856fce66c1b2b78ac308760840effa8c8ccd12c8a83e7da37a5265fe";

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn source_stroke(source_input: &serde_json::Value) -> Stroke {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../../../../conformance/fountain-v16.json"))
            .expect("pinned Fountain V16 base fixture parses");
    let mut stroke: Stroke = serde_json::from_value(reference["stroke"].clone())
        .expect("pinned Fountain V16 source stroke deserializes");
    stroke.points = source_input["points"]
        .as_array()
        .expect("captured source point array")
        .iter()
        .map(|point| Point {
            x: point[0].as_f64().expect("source x"),
            y: point[1].as_f64().expect("source y"),
        })
        .collect();
    stroke.pressures = source_input["pressures"]
        .as_array()
        .expect("captured pressure array")
        .iter()
        .map(|value| value.as_f64().expect("source pressure"))
        .collect();
    stroke.timestamps = source_input["timestamps"]
        .as_array()
        .expect("captured timestamp array")
        .iter()
        .map(|value| value.as_i64().expect("source timestamp"))
        .collect();
    stroke.tilts = vec![0.0; stroke.points.len()];
    stroke.orientations.clear();
    stroke.pen_width = source_input["pen_width"]
        .as_f64()
        .expect("captured pen width") as f32;
    stroke.color = None;

    let rendering = stroke.rendering.as_mut().expect("saved Fountain rendering");
    rendering.advanced_settings = Some("18;0;100;".into());
    rendering.tool_type_raw = 2;
    rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.FountainPen".into());
    rendering.properties.rainbow_effect = true;
    rendering.properties.fixed_width = source_input["fixed_width_enabled"]
        .as_bool()
        .expect("fixed width setting");
    rendering.properties.stylus_channels = true;
    rendering.properties.eraser = false;
    rendering.properties.fixed_opacity = false;
    rendering.properties.alpha_lock = false;
    rendering.properties.replay_only = false;
    rendering.properties.reveal_mode = false;
    rendering.properties.top_layer_pen = false;
    rendering.properties.generated = false;
    rendering.properties.binary_added = true;
    rendering.properties.straighten = false;
    rendering.style.pen_size = Some(stroke.pen_width);
    rendering.style.initial_tolerance = Some(
        source_input["initial_tolerance"]
            .as_f64()
            .expect("captured tolerance") as f32,
    );
    stroke
}

fn palette(source_input: &serde_json::Value) -> Vec<u32> {
    source_input["palette"]
        .as_array()
        .expect("captured palette")
        .iter()
        .map(|color| {
            u32::from_str_radix(color.as_str().expect("ARGB palette string"), 16)
                .expect("valid captured ARGB")
        })
        .collect()
}

fn native_records_from_bytes(
    raw: &[u8],
    expected_bytes: usize,
    expected_sha256: &str,
) -> Vec<[f32; 10]> {
    assert_eq!(raw.len(), expected_bytes);
    assert_eq!(raw.len() % RECORD_BYTES, 0);
    assert_eq!(sha256(raw), expected_sha256);
    raw.as_chunks::<RECORD_BYTES>()
        .0
        .iter()
        .map(|chunk| {
            std::array::from_fn(|lane| {
                f32::from_le_bytes(chunk[lane * 4..lane * 4 + 4].try_into().unwrap())
            })
        })
        .collect()
}

fn decoded_zlib_base64(value: &str) -> Vec<u8> {
    let compressed = base64::engine::general_purpose::STANDARD
        .decode(value)
        .expect("zlib base64");
    let mut decoder = ZlibDecoder::new(compressed.as_slice());
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).expect("zlib stream");
    bytes
}

fn compare_prepared_to_native<'a>(
    stroke: &'a Stroke,
    source_input: &serde_json::Value,
    native: &[[f32; 10]],
) -> crate::ink::PreparedStroke<'a> {
    let colors = palette(source_input);
    let period = source_input["period"].as_i64().unwrap() as i32;
    let offset = source_input["offset"].as_f64().unwrap() as f32;
    let prepared = crate::ink::prepare_stroke_for_context_with_fountain_v17_rainbow(
        stroke,
        crate::ink::InkRenderContext::CanonicalPage,
        &colors,
        period,
        offset,
        Some(1.0),
    );
    assert_eq!(prepared.points.len(), native.len(), "native stamp count");
    let plan = V16Plan::prepare(&prepared).expect("bounded Fountain V17 paint plan");
    assert_eq!(plan.stamps.len(), native.len());
    let colors = prepared
        .fountain_v17_rgb
        .as_ref()
        .expect("explicit Fountain V17 per-stamp colors");
    assert_eq!(colors.len(), native.len());
    for (index, (stamp, row)) in plan.stamps.iter().zip(native).enumerate() {
        let [red, green, blue] = stamp.gradient_key.rgb.expect("rainbow color key");
        let actual = [
            stamp.x,
            stamp.y,
            stamp.extent_x,
            stamp.extent_y,
            1.0_f32,
            0.0_f32,
            f32::from_bits(stamp.gradient_key.alpha),
            f32::from_bits(red),
            f32::from_bits(green),
            f32::from_bits(blue),
        ];
        assert_eq!(
            actual.map(f32::to_bits),
            row.map(f32::to_bits),
            "exact Rust/native Update record lane bits at stamp {index}"
        );
    }
    prepared
}

fn transformed_scene(prepared: &crate::ink::PreparedStroke<'_>) -> crate::render::RenderedScene {
    let mut scene = Scene::new(
        Svg::new()
            .width(WIDTH)
            .height(HEIGHT)
            .view_box(ViewBox::new(
                0.0,
                0.0,
                f64::from(WIDTH),
                f64::from(HEIGHT),
                1,
            )),
    );
    // Use the SDK's retained-scene PDF path. Partial-coverage stamps remain
    // radial SVG gradients, without manufacturing native-ink markers.
    scene.retain_text();
    // Native target clear color is premultiplied [.04,.08,.12,.4]; render the
    // same straight RGB/opacity as an SVG page background.
    scene.push(
        Rectangle::new()
            .x(0)
            .y(0)
            .width(WIDTH)
            .height(HEIGHT)
            .fill(Paint::Solid(
                ColorValue::rgb_components(0.1, 0.2, 0.3).unwrap(),
            ))
            .fill_opacity(0.4),
    );
    // Recorded native matrix is identity with translation (32,32) in GL-up
    // coordinates. Reverse the framebuffer rows once to canonical SVG top-down
    // coordinates, yielding [1,0,0,-1,32,64].
    let projection = Transform::matrix_unrounded([1.0, 0.0, 0.0, -1.0, 32.0, 64.0]);
    scene.scope(Group::new().transformed(projection), |svg| {
        assert!(render(svg, prepared, false, None));
    });
    let text = scene.take_native_text();
    let ink = scene.take_native_ink();
    let text_error = scene.take_native_text_error();
    assert!(
        text_error.is_none(),
        "retained Fountain scene text state: {text_error:?}"
    );
    assert!(
        ink.is_empty(),
        "partial-alpha Fountain stamps should remain SVG gradients"
    );
    crate::render::RenderedScene {
        page: crate::RenderedPage::new(0, WIDTH, HEIGHT, scene.finish()),
        text,
        ink,
        text_error,
    }
}

fn raster_svg(svg: &str) -> Vec<u8> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .expect("Rust SVG parses for bounded raster comparison");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(WIDTH, HEIGHT).expect("bounded pixmap");
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.data().to_vec()
}

fn reverse_native_rows(raw: &[u8]) -> Vec<u8> {
    assert_eq!(raw.len(), (WIDTH * HEIGHT * 4) as usize);
    let row_bytes = (WIDTH * 4) as usize;
    let mut top_down = vec![0; raw.len()];
    for y in 0..HEIGHT as usize {
        let src = (HEIGHT as usize - 1 - y) * row_bytes;
        let dst = y * row_bytes;
        top_down[dst..dst + row_bytes].copy_from_slice(&raw[src..src + row_bytes]);
    }
    top_down
}

fn frame_metrics(native: &[u8], rust: &[u8]) -> serde_json::Value {
    assert_eq!(native.len(), rust.len());
    let background = [10_u8, 20, 31, 102];
    let mut absolute_error_sum = 0_u64;
    let mut max_absolute_error = 0_u8;
    let mut changed_native = 0_usize;
    let mut changed_rust = 0_usize;
    let mut union_pixels = 0_usize;
    let mut native_bbox: Option<[u32; 4]> = None;
    let mut rust_bbox: Option<[u32; 4]> = None;
    let mut changed_bbox: Option<[u32; 4]> = None;
    for pixel in 0..(WIDTH * HEIGHT) as usize {
        let at = pixel * 4;
        let n = &native[at..at + 4];
        let r = &rust[at..at + 4];
        let n_changed = n != background;
        let r_changed = r != background;
        changed_native += usize::from(n_changed);
        changed_rust += usize::from(r_changed);
        let x = (pixel as u32) % WIDTH;
        let y = (pixel as u32) / WIDTH;
        if n_changed {
            native_bbox = Some(match native_bbox {
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
                None => [x, y, x, y],
            });
        }
        if r_changed {
            rust_bbox = Some(match rust_bbox {
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
                None => [x, y, x, y],
            });
        }
        if n_changed || r_changed {
            union_pixels += 1;
            changed_bbox = Some(match changed_bbox {
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
                None => [x, y, x, y],
            });
        }
        for lane in 0..4 {
            let error = n[lane].abs_diff(r[lane]);
            absolute_error_sum += u64::from(error);
            max_absolute_error = max_absolute_error.max(error);
        }
    }
    serde_json::json!({
        "comparison": "full 128x96 RGBA; native GL rows reversed once to top-down; no crop or phase fit",
        "background_expected_premultiplied_rgba": background,
        "native_changed_pixels_vs_background": changed_native,
        "rust_changed_pixels_vs_background": changed_rust,
        "native_bbox_topdown_xyxy": native_bbox,
        "rust_bbox_topdown_xyxy": rust_bbox,
        "union_bbox_topdown_xyxy": changed_bbox,
        "mean_absolute_channel_error_full_frame": absolute_error_sum as f64 / native.len() as f64,
        "mean_absolute_channel_error_over_union": absolute_error_sum as f64 / (union_pixels.max(1) * 4) as f64,
        "max_absolute_channel_error": max_absolute_error,
        "pixels_in_error_union": union_pixels,
    })
}

fn vector_pdf(retained: &crate::render::RenderedScene) -> (Vec<u8>, usize, usize) {
    let bytes =
        crate::pdf::render_native_scene_pdf_for_test(retained, &crate::PdfOptions::default())
            .expect("SDK retained-scene PDF writer exports Fountain SVG");
    let pdf = lopdf::Document::load_mem(&bytes).expect("exported PDF parses");
    assert_eq!(pdf.get_pages().len(), 1);
    let mut image_count = 0;
    let mut shading_count = 0;
    for object in pdf.objects.values() {
        if let Ok(stream) = object.as_stream() {
            image_count += usize::from(
                stream
                    .dict
                    .get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .ok()
                    == Some(b"Image".as_slice()),
            );
        }
        if let Ok(dict) = object.as_dict() {
            shading_count += usize::from(
                dict.get(b"ShadingType")
                    .and_then(lopdf::Object::as_i64)
                    .ok()
                    == Some(3),
            );
        }
    }
    assert_eq!(
        image_count, 0,
        "captured partial-coverage output remains vector"
    );
    assert!(shading_count > 0, "PDF retains radial vector shadings");
    (bytes, image_count, shading_count)
}

#[test]
fn captured_fountain_v17_partial_alpha_matches_mesa_goldens() {
    const FIXTURE: &str =
        include_str!("../../../../conformance/fountain-v17-partial-alpha-native-mesa.json");

    assert_eq!(
        sha256(FIXTURE.as_bytes()),
        "6a0b7d502388d762ad5f27772ed9591cb255f760a3d859fe39509e2483700bfe"
    );
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture["schema"],
        "sdocx-fountain-v17-partial-alpha-native-mesa-v1"
    );
    let provenance = &fixture["provenance"];
    assert_eq!(
        provenance["producer_receipt_sha256"],
        PRODUCER_RECEIPT_SHA256
    );
    assert_eq!(
        provenance["mesa_receipt_sha256"],
        MESA_SOURCE_RECEIPT_SHA256
    );
    assert_eq!(provenance["producer_script_sha256"], PRODUCER_SHA256);
    assert_eq!(
        provenance["fragment_shader_sha256"],
        "db4dcc936a046b8479afaab3824f8b1cbe80a92bb465120ba7c6f1f05acd3215"
    );
    assert_eq!(
        provenance["producer_native_library_sha256"]["FountainPen"],
        "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec"
    );
    assert_eq!(
        provenance["mesa_native_library_sha256"]["Renderer"],
        "f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed"
    );
    assert_eq!(
        fixture["frame"]["size_px"],
        serde_json::json!([WIDTH, HEIGHT])
    );
    assert_eq!(
        fixture["frame"]["inverse_scale_xy"],
        serde_json::json!([1.0, 1.0])
    );
    assert_eq!(fixture["frame"]["enhanced_aa"], false);
    assert_eq!(
        fixture["frame"]["background_rgba8"],
        serde_json::json!([10, 20, 31, 102])
    );
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 2);

    for (case, expected_name) in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(["forward", "reverse"])
    {
        assert_eq!(case["name"], expected_name);
        assert_eq!(case["native_record_count"], 117);
        assert_eq!(case["native_record_bytes"], 117 * RECORD_BYTES);
        assert_eq!(
            case["native_records_sha256"], case["mesa_source_update_records_sha256"],
            "producer and Mesa consumer record streams"
        );
        let source = &case["source"];
        assert_eq!(source["points"].as_array().unwrap().len(), 20);
        assert_eq!(source["pressures"].as_array().unwrap().len(), 20);
        assert_eq!(source["timestamps"].as_array().unwrap().len(), 20);

        let native_record_bytes =
            decoded_zlib_base64(case["native_records_zlib_base64"].as_str().unwrap());
        let native = native_records_from_bytes(
            &native_record_bytes,
            case["native_record_bytes"].as_u64().unwrap() as usize,
            case["native_records_sha256"].as_str().unwrap(),
        );
        assert_eq!(native.len(), 117);
        let stroke = source_stroke(source);
        let prepared = compare_prepared_to_native(&stroke, source, &native);
        let retained = transformed_scene(&prepared);
        let rust_frame = raster_svg(&retained.page.svg);

        let native_raw = decoded_zlib_base64(case["mesa_frame_zlib_base64"].as_str().unwrap());
        assert_eq!(native_raw.len(), (WIDTH * HEIGHT * 4) as usize);
        assert_eq!(sha256(&native_raw), case["mesa_frame_sha256"]);
        let native_top_down = reverse_native_rows(&native_raw);
        let metrics = frame_metrics(&native_top_down, &rust_frame);
        assert_eq!(metrics["native_changed_pixels_vs_background"], 130);
        assert_eq!(metrics["rust_changed_pixels_vs_background"], 130);
        for (pixel, (native, rendered)) in native_top_down
            .as_chunks::<4>()
            .0
            .iter()
            .zip(rust_frame.as_chunks::<4>().0)
            .enumerate()
        {
            assert_eq!(
                *native != [10, 20, 31, 102],
                *rendered != [10, 20, 31, 102],
                "{expected_name} changed support at pixel {pixel}"
            );
        }
        assert_eq!(
            metrics["native_bbox_topdown_xyxy"],
            serde_json::json!([41, 0, 42, 64])
        );
        assert_eq!(&native_top_down[..4], &[10, 20, 31, 102]);
        assert_eq!(&rust_frame[..4], &[10, 20, 31, 102]);
        assert!(
            metrics["max_absolute_channel_error"].as_u64().unwrap() <= 8,
            "{} full-frame RGBA max error: {}",
            expected_name,
            metrics["max_absolute_channel_error"]
        );

        let (_, image_count, shading_count) = vector_pdf(&retained);
        assert_eq!(image_count, 0);
        assert!(shading_count > 0);
    }
}
