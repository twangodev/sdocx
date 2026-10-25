//! Opt-in SVG/PDF artifacts for the pinned persisted AS18 Fountain GLV16
//! capture. The saved source arrays pass through the production V16
//! preparation and paint plan; captured native AddPoint upload records and the
//! native Mesa frame remain explicit external evidence.

use super::super::vector::{Group, Scene, Styled, Svg, Transform, ViewBox};
use super::{V16Plan, render};
use crate::{Color, Point, Stroke};
use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const SOURCE_PAYLOAD_SHA256: &str =
    "c01022129e9a3a09ee95dc5935d5e177bd507e03d6d9dbd357a5278a64ab72b4";
const SOURCE_FRAME_SHA256: &str =
    "490fa32ab9d69416e4c48dec88b26ad14e362d7b08eb3e1b6a9ada47ce2980c2";
const NATIVE_VECTOR_SHA256: &str =
    "3829df249606e8a8c360780d91929ef21dda075a0c379a6c38b09f8caa9fa56a";
const NATIVE_FRAME_SHA256: &str =
    "318568f0f140349c7662e87f19bb05ee18757d8e17178764a4157242c2742de7";
const FOUNTAIN_PEN_DSO_SHA256: &str =
    "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec";
const RTV5_QUAD_FILE_OFFSET: usize = 0x3e10c;
const RTV5_QUAD_BYTES: usize = 64;
const RTV5_QUAD_SHA256: &str = "a39b6ddc1dbf16f1c1efb544817dc4ca4de6b575365af723878516f6afb558e4";

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_new(path: PathBuf, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .expect("artifact path must be new");
    file.write_all(bytes).expect("write artifact");
    file.sync_all().expect("sync artifact");
}

fn decode_f32_words(values: &serde_json::Value) -> Vec<f32> {
    values
        .as_array()
        .expect("captured f32 bit array")
        .iter()
        .map(|value| {
            f32::from_bits(
                u32::from_str_radix(value.as_str().expect("f32 word"), 16).expect("valid f32 word"),
            )
        })
        .collect()
}

fn source_stroke(baseline: &serde_json::Value, rainbow: bool) -> Stroke {
    let source = &baseline["cases"][0];
    let arrays = &source["source_arrays"];
    let points = decode_f32_words(&arrays["points_f32_bits"]);
    assert_eq!(points.len(), 204 * 2);
    let count = points.len() / 2;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../../conformance/fountain-v16.json"))
            .expect("V16 reference fixture parses");
    let mut stroke: Stroke =
        serde_json::from_value(fixture["stroke"].clone()).expect("V16 reference stroke");
    stroke.points = points
        .as_chunks::<2>()
        .0
        .iter()
        .map(|xy| Point {
            x: f64::from(xy[0]),
            y: f64::from(xy[1]),
        })
        .collect();
    stroke.pressures = decode_f32_words(&arrays["pressure_f32_bits"])
        .into_iter()
        .map(f64::from)
        .collect();
    stroke.tilts = decode_f32_words(&arrays["tilt_f32_bits"])
        .into_iter()
        .map(f64::from)
        .collect();
    stroke.orientations = decode_f32_words(&arrays["orientation_f32_bits"])
        .into_iter()
        .map(f64::from)
        .collect();
    stroke.timestamps = arrays["timestamps_i32"]
        .as_array()
        .expect("captured timestamp array")
        .iter()
        .map(|value| value.as_i64().expect("timestamp"))
        .collect();
    assert_eq!(stroke.pressures.len(), count);
    assert_eq!(stroke.tilts.len(), count);
    assert_eq!(stroke.orientations.len(), count);
    assert_eq!(stroke.timestamps.len(), count);
    stroke.color = Some(Color { r: 0, g: 0, b: 0 });
    stroke.pen_width = f32::from_bits(0x4111_de6a);
    let rendering = stroke.rendering.as_mut().expect("saved rendering metadata");
    rendering.advanced_settings = Some("18;0;100;".into());
    rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.FountainPen".into());
    rendering.tool_type_raw = 2;
    rendering.properties.rainbow_effect = rainbow;
    rendering.properties.fixed_width = false;
    rendering.properties.stylus_channels = true;
    rendering.properties.eraser = false;
    rendering.properties.fixed_opacity = false;
    rendering.properties.alpha_lock = false;
    rendering.style.color_argb = Some(0xff00_0000);
    rendering.style.color_type_raw = Some(1);
    rendering.style.pen_size = Some(stroke.pen_width);
    rendering.style.rainbow_distance = Some(37);
    rendering.style.rainbow_offset = Some(2.75);
    rendering.style.gradient_colors_argb = Some(vec![0xffff_0000, 0xff00_ff00, 0xff00_00ff]);
    stroke
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).expect("valid byte hex")
        })
        .collect()
}

fn decode_little_endian_word(value: &str) -> u32 {
    u32::from_le_bytes(decode_hex(value).try_into().expect("one f32 word"))
}

fn native_rtv5_quad_source(baseline: &serde_json::Value) -> serde_json::Value {
    // Pinned FountainPen DSO: RTV5::Init at RVA 0x9eaa4 loads this static
    // 4xvec4 table at 0x3e10c and passes its stack copy to GraphicsObject's
    // SetBuffer slot with first=0,count=4. The matching 64-byte GL upload
    // digest is retained in the native V16 receipt.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.join("../..");
    let dso = repo.join("scratch/apk-analysis-native/arm64-v8a/libSPenFountainPen.so");
    let dso_bytes = fs::read(&dso).expect("pinned FountainPen DSO for quad source");
    assert_eq!(sha256(&dso_bytes), FOUNTAIN_PEN_DSO_SHA256);
    let table = dso_bytes
        .get(RTV5_QUAD_FILE_OFFSET..RTV5_QUAD_FILE_OFFSET + RTV5_QUAD_BYTES)
        .expect("quad table lies in pinned DSO");
    assert_eq!(sha256(table), RTV5_QUAD_SHA256);
    let rows = table
        .as_chunks::<16>()
        .0
        .iter()
        .map(|row| {
            row.as_chunks::<4>()
                .0
                .iter()
                .map(|word| f32::from_le_bytes(*word))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        vec![
            vec![-1.0, -1.0, 0.0, 1.0],
            vec![-1.0, 1.0, 0.0, 0.0],
            vec![1.0, -1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0, 0.0],
        ]
    );
    let matching_uploads = baseline["cases"][0]["renderer_gl_calls"]
        .as_array()
        .expect("native V16 GLES call rows")
        .iter()
        .filter(|event| {
            event["call"] == "glBufferData"
                && event["target"].as_u64() == Some(0x8892)
                && event["size"].as_u64() == Some(RTV5_QUAD_BYTES as u64)
                && event["dataSha256"].as_str() == Some(RTV5_QUAD_SHA256)
        })
        .count();
    assert_eq!(
        matching_uploads, 1,
        "one captured upload matches RTV5's static quad"
    );
    let source_calls = baseline["cases"][0]["renderer_shader_source_calls"]
        .as_array()
        .expect("captured native shader source rows");
    let vertex_shader = source_calls
        .iter()
        .find(|event| {
            event["source"].as_array().is_some_and(|sources| {
                sources.iter().any(|source| {
                    source.as_str().is_some_and(|source| {
                        source.contains("aRectPoint.xy * aPointInfo.zw")
                            && source.contains("fragCoord = vec2(aRectPoint.z, aRectPoint.w)")
                    })
                })
            })
        })
        .expect("captured V16 quad vertex shader source");
    let shader_id = vertex_shader["shader"]
        .as_u64()
        .expect("captured shader ID");
    let program6_attached_source = baseline["cases"][0]["renderer_shader_link_events"]
        .as_array()
        .expect("captured shader lifecycle rows")
        .iter()
        .any(|event| {
            event["call"] == "glAttachShader"
                && event["program"].as_u64() == Some(6)
                && event["shader"].as_u64() == Some(shader_id)
        });
    assert!(
        program6_attached_source,
        "quad shader is attached to native draw program 6"
    );
    assert!(
        baseline["cases"][0]["renderer_gl_draw_events"]
            .as_array()
            .expect("native GL draw events")
            .iter()
            .any(|event| event["state"]["program"].as_u64() == Some(6))
    );
    serde_json::json!({
        "dso": "scratch/apk-analysis-native/arm64-v8a/libSPenFountainPen.so",
        "dso_sha256": FOUNTAIN_PEN_DSO_SHA256,
        "init_rva": "0x9eaa4",
        "table_rva": "0x3e10c",
        "table_file_offset": RTV5_QUAD_FILE_OFFSET,
        "bytes": RTV5_QUAD_BYTES,
        "sha256": RTV5_QUAD_SHA256,
        "vertices_f32le": rows,
        "attribute_mapping": {
            "aRectPoint_xy": "native quad corners",
            "aRectPoint_zw": "normalized radial-field coordinates",
            "source": "pinned V16 vertex shader: gl_Position uses aRectPoint.xy; fragCoord uses aRectPoint.zw",
        },
        "native_capture_upload_matches_size_and_sha256": true,
        "captured_v16_shader": {
            "shader_id": shader_id,
            "attached_to_program": 6,
            "draw_program": 6,
            "position_expression": "aPointInfo.xy + aRectPoint.xy * aPointInfo.zw",
            "radial_coordinate_expression": "vec2(aRectPoint.z, aRectPoint.w)",
        },
        "graphics_object_set_buffer_call": {
            "callsite_rva": "0x9ebd4",
            "dispatch": "GraphicsObject vtable slot +16 (BLR x8)",
            "buffer_index": 0,
            "vertex_count": 4,
        },
    })
}

fn quad_geometry_svg(svg: &str, expected_circles: usize) -> String {
    // Rewrite only serialized unit-circle geometry. Keep every nongeometry
    // attribute (including the same user-space radial fill) and all surrounding
    // group transforms/order byte-for-byte. The native table is a [-1,1]^2
    // quad, so this rectangle has the same local bounds as the unit circle.
    let mut output = String::with_capacity(svg.len() + expected_circles * 32);
    let mut cursor = 0;
    let mut converted = 0;
    while let Some(relative) = svg[cursor..].find("<circle") {
        let start = cursor + relative;
        output.push_str(&svg[cursor..start]);
        let end = svg[start..]
            .find('>')
            .map(|offset| start + offset + 1)
            .expect("serialized circle start tag terminates");
        let tag = &svg[start..end];
        assert!(tag.ends_with("/>"), "only empty circle nodes are rewritten");
        let attrs = &tag[7..tag.len() - 2];
        let attributes = parse_svg_attributes(attrs);
        assert_eq!(
            attributes.iter().filter(|(name, _)| name == "r").count(),
            1,
            "each V16 stamp has one unit radius"
        );
        assert!(
            attributes
                .iter()
                .any(|(name, value)| name == "r" && value == "1")
        );
        assert!(
            attributes
                .iter()
                .all(|(name, value)| { !matches!(name.as_str(), "cx" | "cy") || value == "0" }),
            "V16 unit circles are centered at their local origin"
        );
        assert!(attributes.iter().any(|(name, _)| name == "fill"));
        output.push_str("<rect x=\"-1\" y=\"-1\" width=\"2\" height=\"2\"");
        for (name, value) in attributes {
            if matches!(name.as_str(), "cx" | "cy" | "r") {
                continue;
            }
            output.push(' ');
            output.push_str(&name);
            output.push_str("=\"");
            output.push_str(&value);
            output.push('"');
        }
        output.push_str("/>");
        cursor = end;
        converted += 1;
    }
    output.push_str(&svg[cursor..]);
    assert_eq!(
        converted, expected_circles,
        "all and only V16 stamp circles rewritten"
    );
    assert!(!output.contains("<circle"));
    output
}

fn circle_geometry_svg(svg: &str, expected_rectangles: usize) -> String {
    // Reconstruct the prior SVG circle serialization from the production
    // native-quad SVG. Only the exact local [-1,1]^2 rectangles are changed;
    // gradient attributes, stamp transforms, and order remain untouched.
    let mut output = String::with_capacity(svg.len());
    let mut cursor = 0;
    let mut converted = 0;
    while let Some(relative) = svg[cursor..].find("<rect") {
        let start = cursor + relative;
        output.push_str(&svg[cursor..start]);
        let end = svg[start..]
            .find('>')
            .map(|offset| start + offset + 1)
            .expect("serialized rectangle start tag terminates");
        let tag = &svg[start..end];
        assert!(
            tag.ends_with("/>"),
            "only empty rectangle nodes are inspected"
        );
        let attributes = parse_svg_attributes(&tag[5..tag.len() - 2]);
        let geometry = [("x", "-1"), ("y", "-1"), ("width", "2"), ("height", "2")];
        let is_native_quad = geometry.iter().all(|(name, expected)| {
            attributes
                .iter()
                .filter(|(attribute, _)| attribute.as_str() == *name)
                .count()
                == 1
                && attributes.iter().any(|(attribute, value)| {
                    attribute.as_str() == *name && value.as_str() == *expected
                })
        });
        if !is_native_quad {
            output.push_str(tag);
            cursor = end;
            continue;
        }
        assert!(attributes.iter().any(|(name, _)| name == "fill"));
        output.push_str("<circle");
        for (name, value) in attributes {
            if geometry
                .iter()
                .any(|(geometry_name, _)| name == *geometry_name)
            {
                continue;
            }
            output.push(' ');
            output.push_str(&name);
            output.push_str("=\"");
            output.push_str(&value);
            output.push('"');
        }
        output.push_str(" r=\"1\"/>");
        cursor = end;
        converted += 1;
    }
    output.push_str(&svg[cursor..]);
    assert_eq!(
        converted, expected_rectangles,
        "all production quad stamps converted"
    );
    output
}

fn parse_svg_attributes(input: &str) -> Vec<(String, String)> {
    let bytes = input.as_bytes();
    let mut cursor = 0;
    let mut attributes = Vec::new();
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == bytes.len() {
            break;
        }
        let name_start = cursor;
        while cursor < bytes.len()
            && (bytes[cursor].is_ascii_alphanumeric()
                || matches!(bytes[cursor], b'_' | b'-' | b':'))
        {
            cursor += 1;
        }
        assert!(cursor > name_start, "serialized SVG attribute name");
        let name = std::str::from_utf8(&bytes[name_start..cursor]).unwrap();
        assert_eq!(
            bytes.get(cursor),
            Some(&b'='),
            "serialized SVG attribute equals"
        );
        cursor += 1;
        assert_eq!(
            bytes.get(cursor),
            Some(&b'\"'),
            "serialized SVG quoted attribute"
        );
        cursor += 1;
        let value_start = cursor;
        while cursor < bytes.len() && bytes[cursor] != b'\"' {
            cursor += 1;
        }
        assert!(
            cursor < bytes.len(),
            "serialized SVG attribute quote closes"
        );
        let value = std::str::from_utf8(&bytes[value_start..cursor]).unwrap();
        cursor += 1;
        attributes.push((name.to_owned(), value.to_owned()));
    }
    attributes
}

fn native_upload_records(diagnostic: &serde_json::Value) -> Vec<[u32; 7]> {
    let upload = &diagnostic["state"]["current"]["native_upload_checkpoint"];
    let raw = decode_hex(
        upload["vector_hex"]
            .as_str()
            .expect("retained upload bytes"),
    );
    assert_eq!(upload["vector_bytes"].as_u64(), Some(2128));
    assert_eq!(sha256(&raw), NATIVE_VECTOR_SHA256);
    assert_eq!(raw.len(), 76 * 7 * 4);
    raw.as_chunks::<28>()
        .0
        .iter()
        .map(|record| {
            std::array::from_fn(|lane| {
                u32::from_le_bytes(record[lane * 4..lane * 4 + 4].try_into().unwrap())
            })
        })
        .collect()
}

fn compare_plan_to_native(plan: &V16Plan, diagnostic: &serde_json::Value) {
    let native = native_upload_records(diagnostic);
    let arguments = diagnostic["state"]["current"]["rtv_addpoint_arguments"]
        .as_array()
        .expect("captured RTV5 AddPoint inputs");
    assert_eq!(plan.stamps.len(), 76);
    assert_eq!(native.len(), plan.stamps.len());
    assert_eq!(arguments.len(), plan.stamps.len());
    for (index, ((stamp, record), args)) in
        plan.stamps.iter().zip(&native).zip(arguments).enumerate()
    {
        let actual = [
            stamp.x.to_bits(),
            stamp.y.to_bits(),
            stamp.extent_x.to_bits(),
            stamp.extent_y.to_bits(),
            1.0_f32.to_bits(),
            stamp.gradient_key.inner,
            stamp.gradient_key.alpha,
        ];
        assert_eq!(actual, *record, "V16Plan/native upload record {index}");
        let abi = &args["native_addpoint_abi_arguments"];
        assert_eq!(abi["float_argument_f32_bits"], "0000803f");
        assert_eq!(
            stamp.x.to_bits(),
            decode_little_endian_word(abi["vector3_argument_f32_bits"][0].as_str().unwrap(),),
            "AddPoint x {index}"
        );
        assert_eq!(
            stamp.y.to_bits(),
            decode_little_endian_word(abi["vector3_argument_f32_bits"][1].as_str().unwrap(),),
            "AddPoint y {index}"
        );
    }
}

fn svg_matrix_from_native(matrix: &[f64], viewport: &[u32]) -> [f64; 6] {
    assert_eq!(matrix.len(), 16);
    assert_eq!(viewport.len(), 4);
    let width = f64::from(viewport[2]);
    let height = f64::from(viewport[3]);
    let scale_x = matrix[0] * width / 2.0;
    let scale_y = matrix[5] * height / 2.0;
    let translate_x = f64::from(viewport[0]) + (matrix[12] + 1.0) * width / 2.0;
    let translate_y_gl = f64::from(viewport[1]) + (matrix[13] + 1.0) * height / 2.0;
    [
        scale_x,
        0.0,
        0.0,
        -scale_y,
        translate_x,
        height - translate_y_gl,
    ]
}

fn retained_scene(
    prepared: &crate::PreparedStroke<'_>,
    svg_transform: [f64; 6],
    expect_circle_sidecar: bool,
    preserve_legacy_circle_geometry: bool,
) -> crate::render::RenderedScene {
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
    scene.retain_text();
    // Matrix/viewport evidence maps source pixels directly to GL framebuffer
    // pixels. glReadPixels begins at lower-left, so SVG applies one Y reversal.
    scene.scope(
        Group::new().transformed(Transform::matrix_unrounded(svg_transform)),
        |svg| {
            assert!(render(svg, prepared, false, None));
        },
    );
    let text = scene.take_native_text();
    let mut ink = scene.take_native_ink();
    let text_error = scene.take_native_text_error();
    assert!(
        text_error.is_none(),
        "retained V16 scene text state: {text_error:?}"
    );
    if expect_circle_sidecar {
        let entries = ink.iter().collect::<Vec<_>>();
        assert_eq!(entries.len(), 1, "one experimental circle sidecar");
        let crate::render::NativeInkPayload::RadialCircles {
            records,
            saved_alpha,
        } = &entries[0].1.payload
        else {
            panic!("experimental PDF must use the existing radial-circle payload");
        };
        assert_eq!(records.len(), 76);
        assert_eq!(*saved_alpha, 1.0);
        assert!(records.iter().all(|record| {
            record[4] == 0.0 && record[5] == 1.0 && record[6..9] == [0.0, 0.0, 0.0]
        }));
    } else {
        // Preserve the historical radial-shading PDF as a comparator after
        // production admits full-coverage circles for ordinary V16 color.
        ink = Default::default();
    }
    let mut svg = scene.finish();
    if preserve_legacy_circle_geometry {
        svg = circle_geometry_svg(&svg, 76);
    }
    crate::render::RenderedScene {
        page: crate::RenderedPage::new(0, WIDTH, HEIGHT, svg),
        text,
        ink,
        text_error,
    }
}

fn raster_svg(svg: &str) -> Vec<u8> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .expect("captured V16 SVG parses");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(WIDTH, HEIGHT).expect("bounded canvas");
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.data().to_vec()
}

fn rgba_difference(left: &[u8], right: &[u8]) -> serde_json::Value {
    assert_eq!(left.len(), right.len());
    assert_eq!(left.len() % 4, 0);
    let mut changed_pixels = 0usize;
    let mut alpha_abs_delta = 0u64;
    let mut max_channel_delta = 0u8;
    for (left, right) in left.as_chunks::<4>().0.iter().zip(right.as_chunks::<4>().0) {
        if left != right {
            changed_pixels += 1;
        }
        alpha_abs_delta += u64::from(left[3].abs_diff(right[3]));
        max_channel_delta = max_channel_delta.max(
            left.iter()
                .zip(right)
                .map(|(left, right)| left.abs_diff(*right))
                .max()
                .unwrap_or_default(),
        );
    }
    serde_json::json!({
        "changed_pixels": changed_pixels,
        "alpha_absolute_delta_sum": alpha_abs_delta,
        "max_rgba_channel_delta": max_channel_delta,
    })
}

fn topdown_frame(raw: &[u8]) -> Vec<u8> {
    assert_eq!(raw.len(), (WIDTH * HEIGHT * 4) as usize);
    let row = (WIDTH * 4) as usize;
    let mut result = vec![0; raw.len()];
    for y in 0..HEIGHT as usize {
        let source = (HEIGHT as usize - 1 - y) * row;
        let destination = y * row;
        result[destination..destination + row].copy_from_slice(&raw[source..source + row]);
    }
    result
}

fn vector_pdf(retained: &crate::render::RenderedScene) -> (Vec<u8>, usize, usize) {
    let bytes =
        crate::pdf::render_native_scene_pdf_for_test(retained, &crate::PdfOptions::default())
            .expect("retained V16 scene exports to PDF");
    let pdf = lopdf::Document::load_mem(&bytes).expect("PDF parses");
    assert_eq!(pdf.get_pages().len(), 1);
    let mut images = 0;
    let mut shadings = 0;
    for object in pdf.objects.values() {
        if let Ok(stream) = object.as_stream() {
            images += usize::from(
                stream
                    .dict
                    .get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .ok()
                    == Some(b"Image".as_slice()),
            );
        }
        if let Ok(dict) = object.as_dict() {
            shadings += usize::from(
                dict.get(b"ShadingType")
                    .and_then(lopdf::Object::as_i64)
                    .ok()
                    == Some(3),
            );
        }
    }
    assert_eq!(images, 0, "candidate PDF remains vector");
    assert!(shadings > 0, "V16 radial coverage stays as PDF shading");
    (bytes, images, shadings)
}

fn vector_circle_pdf(retained: &crate::render::RenderedScene) -> (Vec<u8>, usize, usize, usize) {
    let bytes =
        crate::pdf::render_native_scene_pdf_for_test(retained, &crate::PdfOptions::default())
            .expect("experimental uniform-black circle scene exports to PDF");
    let pdf = lopdf::Document::load_mem(&bytes).expect("circle PDF parses");
    assert_eq!(pdf.get_pages().len(), 1);
    let mut images = 0;
    let mut circle_forms = 0;
    let mut circle_segments = 0;
    for object in pdf.objects.values() {
        let Ok(stream) = object.as_stream() else {
            continue;
        };
        images += usize::from(
            stream
                .dict
                .get(b"Subtype")
                .and_then(lopdf::Object::as_name)
                .ok()
                == Some(b"Image".as_slice()),
        );
        if stream
            .dict
            .get(b"Subtype")
            .and_then(lopdf::Object::as_name)
            .ok()
            != Some(b"Form".as_slice())
        {
            continue;
        }
        let Ok(group) = stream.dict.get(b"Group").and_then(lopdf::Object::as_dict) else {
            continue;
        };
        if group.get(b"S").and_then(lopdf::Object::as_name).ok() != Some(b"Transparency".as_slice())
            || !matches!(group.get(b"I"), Ok(lopdf::Object::Boolean(true)))
            || !matches!(group.get(b"K"), Ok(lopdf::Object::Boolean(false)))
        {
            continue;
        }
        let content = stream
            .decompressed_content()
            .expect("decompress native-circle form");
        let operations = lopdf::content::Content::decode(&content).expect("circle form operators");
        let black_fill = operations.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && operation
                    .operands
                    .iter()
                    .all(|value| value.as_float().ok() == Some(0.0))
        });
        let cubics = operations
            .operations
            .iter()
            .filter(|operation| operation.operator == "c")
            .count();
        if black_fill && cubics > 0 {
            circle_forms += 1;
            circle_segments += cubics;
            assert!(
                operations
                    .operations
                    .iter()
                    .all(|operation| operation.operator != "re"),
                "native circle form contains only vector paths"
            );
        }
    }
    assert_eq!(images, 0, "experimental PDF remains raster-free");
    assert_eq!(circle_forms, 1, "one native radial-circle Form");
    assert_eq!(
        circle_segments,
        76 * 4,
        "four cubic segments per native stamp"
    );
    (bytes, images, circle_forms, circle_segments)
}

fn uniform_black_circle_prepared<'a>(
    prepared: &crate::PreparedStroke<'a>,
    diagnostic: &serde_json::Value,
) -> (crate::PreparedStroke<'a>, serde_json::Value) {
    let candidate = prepared.clone();
    let plan = V16Plan::prepare(&candidate).expect("ordinary uniform-color V16 circle plan");
    let records = plan
        .radial_circle_records()
        .expect("native full-coverage equal-circle admission");
    assert_eq!(records.len(), 76);
    let args = diagnostic["state"]["current"]["rtv_addpoint_arguments"]
        .as_array()
        .expect("captured native AddPoint inputs");
    assert_eq!(args.len(), records.len());
    let upload = native_upload_records(diagnostic);
    assert_eq!(upload.len(), records.len());
    let mut radius_mismatch_indices = Vec::new();
    let mut max_radius_ulp_distance = 0_u32;
    for (index, (record, argument)) in records.iter().zip(args).enumerate() {
        assert_eq!(record[4], 0.0, "radial edge lane {index}");
        assert_eq!(record[5], 1.0, "full coverage lane {index}");
        assert_eq!(
            &record[6..9],
            &[0.0, 0.0, 0.0],
            "native uniform black {index}"
        );
        // The existing V16 circle helper derives its radius from the exact
        // uploaded outer extent. Keep x/y/extents bit-exact with that upload;
        // report, but do not gate on, round-trip differences from AddPoint's
        // original radius lane (f32 extent +/- fringe is not invertible).
        assert_eq!(record[0].to_bits(), upload[index][0], "native x {index}");
        assert_eq!(record[1].to_bits(), upload[index][1], "native y {index}");
        assert_eq!(
            circle_plan_extent_x_bits(&plan, index),
            upload[index][2],
            "native uploaded extent x {index}"
        );
        assert_eq!(
            circle_plan_extent_y_bits(&plan, index),
            upload[index][3],
            "native uploaded extent y {index}"
        );
        let native_radius = decode_little_endian_word(
            argument["native_addpoint_abi_arguments"]["vector3_argument_f32_bits"][2]
                .as_str()
                .unwrap(),
        );
        let distance = record[2].to_bits().abs_diff(native_radius);
        if distance != 0 {
            radius_mismatch_indices.push(index);
        }
        max_radius_ulp_distance = max_radius_ulp_distance.max(distance);
        assert_eq!(
            argument["native_addpoint_abi_arguments"]["float_argument_f32_bits"], "0000803f",
            "native unit transparency {index}"
        );
    }
    (
        candidate,
        serde_json::json!({
            "circle_radius_source": "existing V16Plan radial_circle_records: uploaded outer extent minus half inverse-scale fringe",
            "native_addpoint_radius_lane_exact_match_required": false,
            "bitwise_circle_radius_mismatch_count": radius_mismatch_indices.len(),
            "max_positive_f32_ulp_distance": max_radius_ulp_distance,
            "mismatch_stamp_indices": radius_mismatch_indices,
            "uploaded_center_and_outer_extent_bits_remain_exact": true,
        }),
    )
}

fn circle_plan_extent_x_bits(plan: &V16Plan, index: usize) -> u32 {
    plan.stamps[index].extent_x.to_bits()
}

fn circle_plan_extent_y_bits(plan: &V16Plan, index: usize) -> u32 {
    plan.stamps[index].extent_y.to_bits()
}

fn captured_frame(capture_dir: &Path, diagnostic: &serde_json::Value, index: usize) -> Vec<u8> {
    let artifact = diagnostic["rgba_artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["case_index"].as_u64() == Some(index as u64))
        .expect("paired native frame artifact");
    let path = capture_dir.join(
        Path::new(artifact["path"].as_str().unwrap())
            .file_name()
            .unwrap(),
    );
    let compressed = fs::read(path).expect("compressed native Mesa frame");
    assert_eq!(sha256(&compressed), artifact["compressed_sha256"]);
    let mut decoder = ZlibDecoder::new(compressed.as_slice());
    let mut raw = Vec::new();
    decoder.read_to_end(&mut raw).expect("decode native frame");
    assert_eq!(raw.len(), artifact["raw_bytes"].as_u64().unwrap() as usize);
    assert_eq!(sha256(&raw), NATIVE_FRAME_SHA256);
    raw
}

#[test]
#[ignore = "writes paired captured AS18 GLV16 SVG/PDF artifacts when capture/output directories are set"]
fn write_as18_glv16_capture_svg_pdf_pairs() {
    let capture_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_204_CAPTURE_DIR")
            .expect("set SDOCX_FOUNTAIN_V16_204_CAPTURE_DIR to the V14 capture directory"),
    );
    let baseline_path = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_204_BASELINE_RECEIPT")
            .expect("set SDOCX_FOUNTAIN_V16_204_BASELINE_RECEIPT to the pinned HF02 receipt"),
    );
    let output_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_204_ARTIFACT_DIR")
            .expect("set SDOCX_FOUNTAIN_V16_204_ARTIFACT_DIR to a fresh output directory"),
    );
    fs::create_dir(&output_dir).expect("artifact output directory must be new");
    let diagnostic_bytes = fs::read(capture_dir.join("as18-v16-204-incomplete-diagnostic.json"))
        .expect("V14 incomplete diagnostic receipt");
    let diagnostic: serde_json::Value = serde_json::from_slice(&diagnostic_bytes).unwrap();
    let baseline_bytes = fs::read(&baseline_path).expect("pinned visible baseline receipt");
    assert_eq!(
        sha256(&baseline_bytes),
        "a631d4389c60b5188fc87119c0d196962bb4674848e315082206899055f408b7"
    );
    let baseline: serde_json::Value = serde_json::from_slice(&baseline_bytes).unwrap();
    assert_eq!(
        baseline["source_input"]["payload_sha256"],
        SOURCE_PAYLOAD_SHA256
    );
    assert_eq!(
        baseline["source_input"]["frame_sha256"],
        SOURCE_FRAME_SHA256
    );
    assert_eq!(baseline["source_input"]["point_count"], 204);
    assert_eq!(baseline["cases"][0]["selected_drawable_class"], "GLV16");
    assert_eq!(baseline["cases"][0]["hf02_visible"], 1);
    assert_eq!(
        baseline["cases"][0]["renderer_readback"]["rgba_sha256"],
        NATIVE_FRAME_SHA256
    );
    assert_eq!(
        baseline["cases"][0]["controlled_preclear_upload"]["vector_sha256"],
        NATIVE_VECTOR_SHA256
    );
    let native_quad = native_rtv5_quad_source(&baseline);
    assert_eq!(
        diagnostic["state"]["source_payload_sha256"],
        SOURCE_PAYLOAD_SHA256
    );
    assert_eq!(
        diagnostic["state"]["source_frame_sha256"],
        SOURCE_FRAME_SHA256
    );
    assert_eq!(diagnostic["state"]["source_point_count"], 204);
    assert_eq!(diagnostic["state"]["current"]["case"], "rainbow_on");
    assert_eq!(
        diagnostic["state"]["current"]["redraw_entries"][0]["entry"],
        "SPen::FountainPenStrokeDrawableGLV16::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)"
    );
    assert_eq!(
        diagnostic["state"]["current"]["viewport_at_readback"],
        serde_json::json!([0, 0, WIDTH, HEIGHT])
    );
    assert_eq!(
        diagnostic["state"]["current"]["renderer_readback"]["rgba_sha256"],
        NATIVE_FRAME_SHA256
    );

    let gl = diagnostic["state"]["current"]["renderer_gl_events"]
        .as_array()
        .unwrap();
    assert!(gl.iter().any(|event| event["call"] == "glViewport"
        && event["values"] == serde_json::json!([0, 0, WIDTH, HEIGHT])));
    let matrix = gl
        .iter()
        .find(|event| event["call"] == "glUniformMatrix4fv" && event["location"] == 0)
        .unwrap();
    assert_eq!(matrix["program"], 6);
    // glUniformMatrix4fv consumes f32 values. Recover that recorded precision
    // before comparing or deriving the mapping; JSON's f64 parser can differ
    // by one f64 ULP without changing any native uniform bits.
    let matrix_values = matrix["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| f64::from(value.as_f64().unwrap() as f32))
        .collect::<Vec<_>>();
    let matrix_words = matrix_values
        .iter()
        .map(|value| (*value as f32).to_bits())
        .collect::<Vec<_>>();
    assert_eq!(
        matrix_words,
        [
            0x3a88_8889,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x3af2_b9d6,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0xb800_0100,
            0x0000_0000,
            0xbf80_0000,
            0xbf80_0000,
            0x0000_0000,
            0x3f80_0000,
        ]
    );
    let color = gl
        .iter()
        .find(|event| event["call"] == "glUniform4fv" && event["location"] == 1)
        .unwrap();
    assert_eq!(color["program"], 6);
    assert_eq!(color["values"], serde_json::json!([0.0, 0.0, 0.0, 1.0]));
    let draw = gl
        .iter()
        .find(|event| event["call"] == "glDrawArraysInstanced")
        .unwrap();
    assert_eq!(draw["state"]["program"], 6);
    assert_eq!(draw["values"], serde_json::json!([5, 0, 4, 76]));
    assert_eq!(
        diagnostic["state"]["current"]["native_upload_checkpoint"]["vector_sha256"],
        NATIVE_VECTOR_SHA256
    );
    let viewport = diagnostic["state"]["current"]["viewport_at_readback"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as u32)
        .collect::<Vec<_>>();
    let svg_transform = svg_matrix_from_native(&matrix_values, &viewport);

    // Both captured native controls use the same saved 204-point arrays and
    // yielded byte-identical Mesa frames. Off is the pinned baseline; on is
    // the V14 incomplete receipt whose only failure is later cache release.
    let native_off = topdown_frame(&captured_frame(&capture_dir, &diagnostic, 0));
    let native_on = topdown_frame(&captured_frame(&capture_dir, &diagnostic, 1));
    assert_eq!(native_off, native_on);
    let mut manifest_cases = Vec::new();
    let mut shared_rust_raster: Option<Vec<u8>> = None;
    let mut shared_svg: Option<String> = None;
    let mut shared_quad_svg: Option<String> = None;
    let mut shared_quad_raster: Option<Vec<u8>> = None;
    for (name, rainbow, native) in [
        ("rainbow_off", false, &native_off),
        ("rainbow_on", true, &native_on),
    ] {
        let stroke = source_stroke(&baseline, rainbow);
        let prepared = if rainbow {
            crate::ink::prepare_stroke_for_context_with_fountain_v16_rainbow_ignored(
                &stroke,
                crate::ink::InkRenderContext::CanonicalPage,
            )
        } else {
            crate::ink::prepare_stroke_for_context(
                &stroke,
                false,
                crate::ink::InkRenderContext::CanonicalPage,
            )
        };
        let plan = V16Plan::prepare(&prepared).expect("production GLV16 V16Plan");
        assert_eq!(plan.stamps.len(), 76);
        if rainbow {
            compare_plan_to_native(&plan, &diagnostic);
        }
        let production = retained_scene(&prepared, svg_transform, true, false);
        let production_quad_svg = production.page.svg.clone();
        let production_quad_raster = raster_svg(&production_quad_svg);
        let (production_quad_pdf, production_images, production_forms, production_segments) =
            vector_circle_pdf(&production);

        // Preserve the original SVG-circle and PDF-shading output as the
        // comparator. The separate production output is generated by the
        // current V16Plan quad admission, not by a fitted or post-raster shape.
        let legacy_circle = retained_scene(&prepared, svg_transform, false, true);
        let legacy_circle_svg = legacy_circle.page.svg.clone();
        let legacy_circle_raster = raster_svg(&legacy_circle_svg);
        let (legacy_circle_pdf, image_count, shading_count) = vector_pdf(&legacy_circle);
        let quad_svg_from_legacy = quad_geometry_svg(&legacy_circle_svg, plan.stamps.len());
        // SVG attribute order is serializer-specific. Compare the complete
        // element tree and attribute values while retaining exact node order.
        let signature = |document: &roxmltree::Document<'_>| {
            document
                .descendants()
                .filter(|node| node.is_element())
                .map(|node| {
                    let mut attributes = node
                        .attributes()
                        .map(|attribute| {
                            (attribute.name().to_owned(), attribute.value().to_owned())
                        })
                        .collect::<Vec<_>>();
                    attributes.sort();
                    (
                        node.ancestors().count(),
                        node.tag_name().name().to_owned(),
                        attributes,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            signature(&roxmltree::Document::parse(&quad_svg_from_legacy).unwrap()),
            signature(&roxmltree::Document::parse(&production_quad_svg).unwrap()),
            "production quad differs only by the source-pinned native geometry"
        );
        assert_eq!(raster_svg(&quad_svg_from_legacy), production_quad_raster);
        let quad_vs_circle_raster = rgba_difference(&legacy_circle_raster, &production_quad_raster);

        // Compare the legacy radial-shading PDF with ordinary uniform-color
        // V16 full-coverage circle admission. Both use the same prepared stroke;
        // only the legacy PDF omits the registered sidecar during conversion.
        let (circle_prepared, radius_round_trip) =
            uniform_black_circle_prepared(&prepared, &diagnostic);
        let circle_plan =
            V16Plan::prepare(&circle_prepared).expect("experimental uniform-black V16Plan");
        assert_eq!(circle_plan.stamps.len(), plan.stamps.len());
        let circle_scene = retained_scene(&circle_prepared, svg_transform, true, false);
        let circle_svg_raster = raster_svg(&circle_scene.page.svg);
        assert_eq!(
            circle_svg_raster, production_quad_raster,
            "circle sidecar keeps current production SVG appearance"
        );
        let (circle_pdf, circle_images, circle_forms, circle_segments) =
            vector_circle_pdf(&circle_scene);
        assert_eq!(
            production_quad_pdf, circle_pdf,
            "production PDF keeps the native circle sidecar"
        );
        if let Some(previous) = &shared_rust_raster {
            assert_eq!(
                *previous, legacy_circle_raster,
                "legacy circle comparator is equal across paired cases"
            );
        } else {
            shared_rust_raster = Some(legacy_circle_raster.clone());
        }
        if let Some(previous) = &shared_svg {
            assert_eq!(*previous, legacy_circle_svg, "paired legacy circle SVG");
        } else {
            shared_svg = Some(legacy_circle_svg.clone());
        }
        if let Some(previous) = &shared_quad_svg {
            assert_eq!(
                *previous, production_quad_svg,
                "paired production native-quad SVG"
            );
        } else {
            shared_quad_svg = Some(production_quad_svg.clone());
        }
        if let Some(previous) = &shared_quad_raster {
            assert_eq!(
                *previous, production_quad_raster,
                "paired production native-quad raster"
            );
        } else {
            shared_quad_raster = Some(production_quad_raster.clone());
        }
        let prefix = name.to_owned();
        write_new(
            output_dir.join(format!("{prefix}.svg")),
            legacy_circle_svg.as_bytes(),
        );
        write_new(output_dir.join(format!("{prefix}.pdf")), &legacy_circle_pdf);
        write_new(
            output_dir.join(format!("{prefix}.production-quad.svg")),
            production_quad_svg.as_bytes(),
        );
        write_new(
            output_dir.join(format!("{prefix}.production-quad.pdf")),
            &production_quad_pdf,
        );
        write_new(
            output_dir.join(format!("{prefix}.full-coverage-circles-experiment.pdf")),
            &circle_pdf,
        );
        if name == "rainbow_off" {
            write_new(output_dir.join("rust-topdown.rgba"), &legacy_circle_raster);
            write_new(
                output_dir.join("quad-topdown.rgba"),
                &production_quad_raster,
            );
        }
        let native_changed = native
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| **pixel != [0, 0, 0, 0])
            .count();
        let rust_changed = legacy_circle_raster
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| **pixel != [0, 0, 0, 0])
            .count();
        manifest_cases.push(serde_json::json!({
            "case": name,
            "captured_native_frame_sha256_topdown": sha256(native),
            "captured_native_nontransparent_pixels": native_changed,
            "rust_svg_raster_sha256_topdown": sha256(&legacy_circle_raster),
            "rust_svg_nontransparent_pixels": rust_changed,
            "production_quad": {
                "svg_path": format!("{prefix}.production-quad.svg"),
                "pdf_path": format!("{prefix}.production-quad.pdf"),
                "geometry": "production V16Plan quad gate emits rect x=-1,y=-1,width=2,height=2 per stamp; same source-derived gradient, transform, and draw order",
                "stamp_count": plan.stamps.len(),
                "svg_sha256": sha256(production_quad_svg.as_bytes()),
                "resvg_premultiplied_rgba_sha256_topdown": sha256(&production_quad_raster),
                "resvg_nontransparent_pixels": production_quad_raster
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|pixel| **pixel != [0, 0, 0, 0])
                    .count(),
                "against_circle_svg_resvg": quad_vs_circle_raster,
                "pixel_output_equality_required": false,
                "native_quad_source": native_quad.clone(),
                "production_svg_matches_independently_rewritten_native_quad":
                    true,
                "pdf_images": production_images,
                "pdf_radial_shadings": 0,
                "pdf_native_circle_forms": production_forms,
                "pdf_native_circle_segments": production_segments,
                "pdf_sha256": sha256(&production_quad_pdf),
            },
            "native_vs_rust_pixel_equality_evaluated": false,
            "source_to_plan_records": 76,
            "native_upload_sha256": NATIVE_VECTOR_SHA256,
            "native_addpoint_input_vs_production_plan_bitwise_compared": rainbow,
            "pdf_pages": 1,
            "pdf_image_objects": image_count,
            "pdf_radial_shadings": shading_count,
            "svg_sha256": sha256(legacy_circle_svg.as_bytes()),
            "pdf_sha256": sha256(&legacy_circle_pdf),
            "experimental_pdf": {
                "path": format!("{prefix}.full-coverage-circles-experiment.pdf"),
                "representation": "ordinary uniform-color V16 full-coverage radial-circle admission and existing PDF converter",
                "native_uniform_argb": "0xff000000",
                "native_gl_input_color": [0.0, 0.0, 0.0, 1.0],
                "record_source": "same production V16Plan stamps; existing radial_circle_records admission supplies radii",
                "positions_or_radii_fitted": false,
                "radius_round_trip_diagnostic": radius_round_trip,
                "native_full_coverage_records": circle_plan.stamps.len(),
                "registered_radial_circle_forms": circle_forms,
                "cubic_segments": circle_segments,
                "pdf_image_objects": circle_images,
                "pdf_vector_only": circle_images == 0 && circle_forms == 1,
                "svg_scene_raster_matches_production_quad":
                    circle_svg_raster == production_quad_raster,
                "pdf_sha256": sha256(&circle_pdf),
            },
        }));
    }
    let manifest = serde_json::json!({
        "schema": "sdocx.fountain.as18-glv16.capture-svg-pdf-artifacts.v1",
        "scope": "source arrays from pinned 204-point saved HF02 frame through the production GLV16 V16Plan; explicit legacy circle SVG/PDF comparator, separately emitted production quad SVG/PDF, one captured GLV16/RTV5 native upload, and paired host Mesa frames",
        "primitive_boundary": "class-level V16 stamp export; this does not prove Samsung device/PDF parity or independent native GPU implementation",
        "source_payload_sha256": SOURCE_PAYLOAD_SHA256,
        "source_frame_sha256": SOURCE_FRAME_SHA256,
        "source_point_count": 204,
        "native_upload_sha256": NATIVE_VECTOR_SHA256,
        "native_rtv5_static_quad": native_quad,
        "native_rgba_sha256": NATIVE_FRAME_SHA256,
        "native_saved_argb": "0xff000000",
        "native_canvas_px": [WIDTH, HEIGHT],
        "native_viewport": [0, 0, WIDTH, HEIGHT],
        "native_gl_uniform_matrix4fv_program6_location0_f32": matrix_values,
        "svg_transform_from_native_matrix_and_gl_read_origin": svg_transform,
        "native_gl_input_color": color["values"],
        "native_draw": draw["values"],
        "native_rows_flipped_once_for_topdown_artifacts": true,
        "off_on_native_frames_bitwise_equal": true,
        "native_execution": false,
        "artifact_render_executed": true,
        "capture_replay": false,
        "cases": manifest_cases,
    });
    write_new(
        output_dir.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).unwrap(),
    );
}
