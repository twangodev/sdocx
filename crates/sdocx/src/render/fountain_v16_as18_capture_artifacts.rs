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
const SCALE_SWEEP_SHA256: &str = "c4def42b77ae1b7f480507253cf0a696575e52d28dc751295436b289b1ad3866";
const SCALE_SWEEP_VALIDATION_SHA256: &str =
    "985bc501ca6ded8c200745d221224aa5826a7a668f363bc6aca3493fb18aa61b";
const SCALE_SWEEP_RAINBOW_SHA256: &str =
    "b858dec2803116a319b6eb6a11822564792b7404c9215a4d69d4c141045b180a";
const SCALE_SWEEP_RAINBOW_VALIDATION_SHA256: &str =
    "7701867da41cfbe31be4713a06376a00e813a0c18b7cd41cf2da59d0381534b3";
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

fn source_stroke_argb(baseline: &serde_json::Value, rainbow: bool, argb: u32) -> Stroke {
    let mut stroke = source_stroke(baseline, rainbow);
    stroke
        .rendering
        .as_mut()
        .expect("saved rendering metadata")
        .style
        .color_argb = Some(argb);
    stroke
}

fn source_stroke_argb_width(
    baseline: &serde_json::Value,
    rainbow: bool,
    argb: u32,
    width: f32,
) -> Stroke {
    let mut stroke = source_stroke_argb(baseline, rainbow, argb);
    stroke.pen_width = width;
    stroke
        .rendering
        .as_mut()
        .expect("saved rendering metadata")
        .style
        .pen_size = Some(width);
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

type SvgElementSignature = (usize, String, Vec<(String, String)>);

fn svg_element_signature(svg: &str) -> Vec<SvgElementSignature> {
    let document = roxmltree::Document::parse(svg).expect("artifact SVG parses");
    document
        .descendants()
        .filter(|node| node.is_element())
        .map(|node| {
            let mut attributes = node
                .attributes()
                .map(|attribute| (attribute.name().to_owned(), attribute.value().to_owned()))
                .collect::<Vec<_>>();
            attributes.sort();
            (
                node.ancestors().count(),
                node.tag_name().name().to_owned(),
                attributes,
            )
        })
        .collect()
}

fn native_upload_records(diagnostic: &serde_json::Value) -> Vec<[u32; 7]> {
    native_upload_records_with_sha(diagnostic, NATIVE_VECTOR_SHA256)
}

fn native_upload_records_with_sha(
    diagnostic: &serde_json::Value,
    expected_sha256: &str,
) -> Vec<[u32; 7]> {
    let upload = &diagnostic["state"]["current"]["native_upload_checkpoint"];
    let raw = decode_hex(
        upload["vector_hex"]
            .as_str()
            .expect("retained upload bytes"),
    );
    assert_eq!(upload["vector_bytes"].as_u64(), Some(2128));
    assert_eq!(sha256(&raw), expected_sha256);
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
    compare_plan_to_native_records(plan, diagnostic, &native);
}

fn compare_plan_to_native_records(
    plan: &V16Plan,
    diagnostic: &serde_json::Value,
    native: &[[u32; 7]],
) {
    let current = &diagnostic["state"]["current"];
    let arguments = current
        .get("rtv_addpoint_arguments")
        .or_else(|| current.get("rtv_addpoint_entries"))
        .unwrap()
        .as_array()
        .expect("captured RTV5 AddPoint inputs");
    assert_eq!(plan.stamps.len(), 76);
    assert_eq!(native.len(), plan.stamps.len());
    assert_eq!(arguments.len(), plan.stamps.len());
    for (index, ((stamp, record), args)) in
        plan.stamps.iter().zip(native).zip(arguments).enumerate()
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

fn scale_sweep_native_records(case: &serde_json::Value) -> Vec<[u32; 7]> {
    let upload = &case["native_upload_checkpoint"];
    let raw = decode_hex(
        upload["vector_hex"]
            .as_str()
            .expect("retained scale-sweep upload bytes"),
    );
    let record_count = case["rtv_addpoint_entries"]
        .as_array()
        .expect("native AddPoint entries")
        .len();
    assert_eq!(
        upload["vector_bytes"].as_u64(),
        Some((record_count * 28) as u64)
    );
    assert_eq!(raw.len(), record_count * 28);
    assert_eq!(
        sha256(&raw),
        upload["vector_sha256"].as_str().expect("upload digest")
    );
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

fn compare_scale_sweep_plan_to_native(plan: &V16Plan, case: &serde_json::Value) {
    let native = scale_sweep_native_records(case);
    let arguments = case["rtv_addpoint_entries"]
        .as_array()
        .expect("captured RTV5 AddPoint inputs");
    assert_eq!(plan.stamps.len(), native.len());
    assert_eq!(arguments.len(), native.len());
    for (index, (stamp, record)) in plan.stamps.iter().zip(native).enumerate() {
        let actual = [
            stamp.x.to_bits(),
            stamp.y.to_bits(),
            stamp.extent_x.to_bits(),
            stamp.extent_y.to_bits(),
            1.0_f32.to_bits(),
            stamp.gradient_key.inner,
            stamp.gradient_key.alpha,
        ];
        assert_eq!(
            actual, record,
            "scaled V16Plan/native upload record {index}"
        );
        let abi = &arguments[index]["native_addpoint_abi_arguments"];
        assert_eq!(abi["float_argument_f32_bits"], "0000803f");
        assert_eq!(
            stamp.x.to_bits(),
            decode_little_endian_word(abi["vector3_argument_f32_bits"][0].as_str().unwrap()),
            "scaled AddPoint x {index}"
        );
        assert_eq!(
            stamp.y.to_bits(),
            decode_little_endian_word(abi["vector3_argument_f32_bits"][1].as_str().unwrap()),
            "scaled AddPoint y {index}"
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

fn sweep_svg_transform(case: &serde_json::Value) -> [f64; 6] {
    let matrices = case["renderer_gl_calls"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|call| {
            call["call"] == "glUniformMatrix4fv"
                && call["location"].as_u64() == Some(0)
                && call["program"].as_u64() == Some(6)
        })
        .collect::<Vec<_>>();
    assert_eq!(matrices.len(), 1, "one captured V16 projection uniform");
    let matrix = matrices[0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap())
        .collect::<Vec<_>>();
    let viewport = case["renderer_readback"]["viewport_at_readback"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| u32::try_from(value.as_u64().unwrap()).unwrap())
        .collect::<Vec<_>>();
    svg_matrix_from_native(&matrix, &viewport)
}

fn sweep_svg_from_plan(
    prepared: &crate::PreparedStroke<'_>,
    svg_transform: [f64; 6],
    force_native_quad: bool,
) -> String {
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
    scene.scope(
        Group::new().transformed(Transform::matrix_unrounded(svg_transform)),
        |svg| {
            if force_native_quad {
                let mut plan = V16Plan::prepare(prepared).expect("production V16Plan");
                // This is a test-only candidate control. It does not alter the
                // production admission gate or any source-derived field values.
                plan.native_quad_geometry = true;
                plan.emit(svg, prepared, false);
            } else {
                assert!(
                    render(svg, prepared, false, None),
                    "production V16 SVG route"
                );
            }
        },
    );
    scene.finish()
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

fn retained_scene_with_default_ink(
    prepared: &crate::PreparedStroke<'_>,
    svg_transform: [f64; 6],
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
    scene.scope(
        Group::new().transformed(Transform::matrix_unrounded(svg_transform)),
        |svg| assert!(render(svg, prepared, false, None)),
    );
    let text = scene.take_native_text();
    let ink = scene.take_native_ink();
    let text_error = scene.take_native_text_error();
    assert!(
        text_error.is_none(),
        "retained V16 scene text state: {text_error:?}"
    );
    crate::render::RenderedScene {
        page: crate::RenderedPage::new(0, WIDTH, HEIGHT, scene.finish()),
        text,
        ink,
        text_error,
    }
}

fn retained_scene_with_direct_type4_ink(
    prepared: &crate::PreparedStroke<'_>,
    svg_transform: [f64; 6],
    records: &[[f32; 9]],
    bounds: crate::BoundingBox,
    saved_alpha: f32,
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
    let id = scene
        .native_ink()
        .register_radial(records, bounds, saved_alpha, false)
        .expect("source-joined thin records pass existing Type 4 validation");
    scene.scope(
        Group::new()
            .native_ink_id(id)
            .transformed(Transform::matrix_unrounded(svg_transform)),
        |svg| assert!(render(svg, prepared, false, None)),
    );
    let text = scene.take_native_text();
    let ink = scene.take_native_ink();
    let text_error = scene.take_native_text_error();
    assert!(
        text_error.is_none(),
        "Type 4 experiment text state: {text_error:?}"
    );
    assert_eq!(ink.len(), 1, "exactly one experimental Type 4 field");
    crate::render::RenderedScene {
        page: crate::RenderedPage::new(0, WIDTH, HEIGHT, scene.finish()),
        text,
        ink,
        text_error,
    }
}

fn type4_function_programs(pdf_bytes: &[u8]) -> Vec<Vec<u8>> {
    let pdf = lopdf::Document::load_mem(pdf_bytes).expect("Type 4 experiment PDF parses");
    pdf.objects
        .values()
        .filter_map(|object| object.as_stream().ok())
        .filter(|stream| {
            stream
                .dict
                .get(b"FunctionType")
                .ok()
                .and_then(|value| value.as_i64().ok())
                == Some(4)
        })
        .map(|stream| {
            stream
                .decompressed_content()
                .expect("decompress Type 4 program")
        })
        .collect()
}

fn type4_vector_pdf_summary(retained: &crate::render::RenderedScene) -> (Vec<u8>, usize, usize) {
    let bytes =
        crate::pdf::render_native_scene_pdf_for_test(retained, &crate::PdfOptions::default())
            .expect("experimental DirectSourceOver scene exports to PDF");
    let pdf = lopdf::Document::load_mem(&bytes).expect("Type 4 experiment PDF parses");
    assert_eq!(pdf.get_pages().len(), 1);
    let mut images = 0;
    let mut type1_shadings = 0;
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
            type1_shadings += usize::from(
                dict.get(b"ShadingType")
                    .and_then(lopdf::Object::as_i64)
                    .ok()
                    == Some(1),
            );
        }
    }
    assert_eq!(images, 0, "Type 4 candidate remains vector");
    assert!(
        type1_shadings > 0,
        "Type 4 mask/RGB use FunctionType 4 shadings"
    );
    (bytes, images, type1_shadings)
}

fn public_svg_with_default_ink(
    prepared: &crate::PreparedStroke<'_>,
    svg_transform: [f64; 6],
) -> String {
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
    scene.scope(
        Group::new().transformed(Transform::matrix_unrounded(svg_transform)),
        |svg| assert!(render(svg, prepared, false, None)),
    );
    scene.finish()
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

fn alpha_support_metrics(native: &[u8], rendered: &[u8]) -> serde_json::Value {
    assert_eq!(native.len(), rendered.len());
    let mut union = 0usize;
    let mut intersection = 0usize;
    let mut native_support = 0usize;
    let mut rendered_support = 0usize;
    let mut absolute_error = 0u64;
    let mut maximum_error = 0u8;
    for (native, rendered) in native
        .as_chunks::<4>()
        .0
        .iter()
        .zip(rendered.as_chunks::<4>().0)
    {
        let (a, b) = (native[3], rendered[3]);
        native_support += usize::from(a != 0);
        rendered_support += usize::from(b != 0);
        union += usize::from(a != 0 || b != 0);
        intersection += usize::from(a != 0 && b != 0);
        let error = a.abs_diff(b);
        absolute_error += u64::from(error);
        maximum_error = maximum_error.max(error);
    }
    assert!(union > 0, "native scale witness must contain ink");
    serde_json::json!({
        "native_support": native_support,
        "rendered_support": rendered_support,
        "union_support": union,
        "intersection_support": intersection,
        "support_iou": intersection as f64 / union as f64,
        "alpha_absolute_error_sum": absolute_error,
        "alpha_union_mae_bytes": absolute_error as f64 / union as f64,
        "alpha_max_error_bytes": maximum_error,
    })
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

fn vector_pdf_summary(retained: &crate::render::RenderedScene) -> (Vec<u8>, usize, usize) {
    let bytes =
        crate::pdf::render_native_scene_pdf_for_test(retained, &crate::PdfOptions::default())
            .expect("retained partial-alpha V16 scene exports to PDF");
    let pdf = lopdf::Document::load_mem(&bytes).expect("partial-alpha PDF parses");
    assert_eq!(pdf.get_pages().len(), 1);
    let mut images = 0;
    let mut radial_shadings = 0;
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
            radial_shadings += usize::from(
                dict.get(b"ShadingType")
                    .and_then(lopdf::Object::as_i64)
                    .ok()
                    == Some(3),
            );
        }
    }
    assert_eq!(images, 0, "partial-alpha comparison PDF stays vector");
    (bytes, images, radial_shadings)
}

fn circle_sidecar_form_signatures(
    pdf_bytes: &[u8],
    stamp_count: usize,
) -> Vec<(String, u32, usize, usize)> {
    let pdf = lopdf::Document::load_mem(pdf_bytes).expect("experimental circle PDF parses");
    let mut signatures = Vec::new();
    for object in pdf.objects.values() {
        let Ok(stream) = object.as_stream() else {
            continue;
        };
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
        let resources = stream
            .dict
            .get(b"Resources")
            .and_then(lopdf::Object::as_dict)
            .expect("circle form ExtGState resources");
        let states = resources
            .get(b"ExtGState")
            .and_then(lopdf::Object::as_dict)
            .expect("circle form ExtGState table");
        let reference = states
            .get(b"GS0")
            .and_then(lopdf::Object::as_reference)
            .expect("circle form GS0 reference");
        let state = pdf
            .get_object(reference)
            .and_then(lopdf::Object::as_dict)
            .expect("circle form GS0 state");
        let emitted_alpha = state
            .get(b"ca")
            .and_then(lopdf::Object::as_float)
            .expect("circle form nonstroking alpha");
        let content = stream
            .decompressed_content()
            .expect("decompress saved-alpha circle form");
        let operations = lopdf::content::Content::decode(&content).expect("circle form operators");
        assert_eq!(
            operations
                .operations
                .iter()
                .filter(|operation| operation.operator == "gs")
                .count(),
            stamp_count,
            "saved alpha is selected once for each ordered stamp"
        );
        assert_eq!(
            operations
                .operations
                .iter()
                .filter(|operation| operation.operator == "c")
                .count(),
            stamp_count * 4,
            "one four-segment vector circle per stamp"
        );
        signatures.push((
            sha256(&content),
            emitted_alpha.to_bits(),
            operations
                .operations
                .iter()
                .filter(|operation| operation.operator == "gs")
                .count(),
            operations
                .operations
                .iter()
                .filter(|operation| operation.operator == "c")
                .count(),
        ));
    }
    signatures
}

fn assert_circle_sidecar_saved_alpha(pdf_bytes: &[u8], alpha: f32, stamp_count: usize) {
    let signatures = circle_sidecar_form_signatures(pdf_bytes, stamp_count);
    assert_eq!(signatures.len(), 1, "one saved-alpha circle form");
    assert_eq!(signatures[0].1, alpha.to_bits());
}

fn alpha_native_upload_records(case: &serde_json::Value) -> Vec<[u32; 7]> {
    let upload = &case["native_upload_checkpoint"];
    assert_eq!(upload["hit"], true);
    assert_eq!(upload["vector_sha256"], NATIVE_VECTOR_SHA256);
    assert_eq!(upload["vector_bytes"].as_u64(), Some(2128));
    let raw = decode_hex(
        upload["vector_hex"]
            .as_str()
            .expect("V5 native upload bytes"),
    );
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

fn compare_alpha_plan_to_native(plan: &V16Plan, case: &serde_json::Value) {
    let native = alpha_native_upload_records(case);
    let arguments = case["rtv_addpoint_entries"]
        .as_array()
        .expect("V5 captured RTV5 AddPoint inputs");
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
        assert_eq!(
            actual, *record,
            "alpha V16Plan/native upload record {index}"
        );
        let abi = &args["native_addpoint_abi_arguments"];
        assert_eq!(abi["float_argument_f32_bits"], "0000803f");
        assert_eq!(
            stamp.x.to_bits(),
            decode_little_endian_word(abi["vector3_argument_f32_bits"][0].as_str().unwrap()),
            "alpha AddPoint x {index}"
        );
        assert_eq!(
            stamp.y.to_bits(),
            decode_little_endian_word(abi["vector3_argument_f32_bits"][1].as_str().unwrap()),
            "alpha AddPoint y {index}"
        );
    }
}

fn captured_alpha_frame(repo: &Path, receipt: &serde_json::Value, case_name: &str) -> Vec<u8> {
    let artifact = receipt["pixel_artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["key"] == case_name)
        .expect("V5 per-case native frame artifact");
    let path = Path::new(artifact["path"].as_str().unwrap());
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        repo.join(path)
    };
    let compressed = fs::read(path).expect("pinned V5 compressed native frame");
    assert_eq!(sha256(&compressed), artifact["compressed_sha256"]);
    let mut decoder = ZlibDecoder::new(compressed.as_slice());
    let mut raw = Vec::new();
    decoder
        .read_to_end(&mut raw)
        .expect("decode V5 native frame");
    assert_eq!(raw.len(), (WIDTH * HEIGHT * 4) as usize);
    assert_eq!(raw.len() as u64, artifact["raw_bytes"].as_u64().unwrap());
    assert_eq!(sha256(&raw), artifact["raw_sha256"]);
    topdown_frame(&raw)
}

#[test]
#[ignore = "writes captured alpha128/alpha255 V16 SVG/PDF comparisons when V5 capture/output directories are set"]
fn write_as18_glv16_alpha_svg_pdf_pairs() {
    const V5_RECEIPT_SHA256: &str =
        "87766dcceff7cb38408db3d6bbf92a9671c14588c517f03302ac41ccc0979009";
    const V5_PINS_SHA256: &str = "a912304fa05810ee6fe85b655ee3ee0fcd413f7a042a6cabd3d40541f82a0fb5";
    const V5_UPLOAD_SHA256: &str =
        "3829df249606e8a8c360780d91929ef21dda075a0c379a6c38b09f8caa9fa56a";
    const SOURCE_ARRAYS_SHA256: &str =
        "c01022129e9a3a09ee95dc5935d5e177bd507e03d6d9dbd357a5278a64ab72b4";

    let capture_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_204_ALPHA_CAPTURE_DIR")
            .expect("set SDOCX_FOUNTAIN_V16_204_ALPHA_CAPTURE_DIR to the V5 capture directory"),
    );
    let output_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_204_ALPHA_V2_ARTIFACT_DIR")
            .expect("set SDOCX_FOUNTAIN_V16_204_ALPHA_V2_ARTIFACT_DIR to a fresh output directory"),
    );
    fs::create_dir(&output_dir).expect("alpha artifact output directory must be new");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root");
    let receipt_path = capture_dir.join("as18-v16-204-alpha-mesa-pair.json");
    let receipt_bytes = fs::read(&receipt_path).expect("V5 paired alpha capture receipt");
    assert_eq!(sha256(&receipt_bytes), V5_RECEIPT_SHA256);
    assert_eq!(
        sha256(&fs::read(capture_dir.join("candidate-pins.json")).expect("V5 source pins")),
        V5_PINS_SHA256
    );
    let receipt: serde_json::Value = serde_json::from_slice(&receipt_bytes).unwrap();
    assert_eq!(
        receipt["schema"],
        "sdocx-fountain-saved-renderer-experiment-v1"
    );
    assert_eq!(
        receipt["source_input"]["payload_sha256"],
        SOURCE_ARRAYS_SHA256
    );
    assert_eq!(receipt["source_input"]["frame_sha256"], SOURCE_FRAME_SHA256);
    assert_eq!(receipt["source_input"]["point_count"], 204);
    assert_eq!(receipt["profile"], serde_json::json!([WIDTH, HEIGHT]));
    assert_eq!(receipt["pixel_artifacts"].as_array().unwrap().len(), 2);
    let cases = receipt["cases"].as_array().expect("V5 alpha pair cases");
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0]["case"], "alpha128");
    assert_eq!(cases[1]["case"], "alpha255");
    assert_eq!(cases[0]["source_payload_sha256"], SOURCE_ARRAYS_SHA256);
    assert_eq!(cases[1]["source_payload_sha256"], SOURCE_ARRAYS_SHA256);
    assert_eq!(cases[0]["source_arrays"], cases[1]["source_arrays"]);
    assert_eq!(cases[0]["source_point_count"], 204);
    assert_eq!(cases[1]["source_point_count"], 204);
    assert_eq!(cases[0]["selected_drawable_class"], "GLV16");
    assert_eq!(cases[1]["selected_drawable_class"], "GLV16");
    assert_eq!(cases[0]["rainbow_enabled_control"], 0);
    assert_eq!(cases[1]["rainbow_enabled_control"], 0);
    assert_eq!(
        cases[0]["rtv_addpoint_entries"].as_array().unwrap().len(),
        76
    );
    assert_eq!(
        cases[1]["rtv_addpoint_entries"].as_array().unwrap().len(),
        76
    );

    // Decode the same captured arrays through the existing production source
    // adapter. Only saved ARGB alpha differs between the two prepared strokes.
    let source_receipt = serde_json::json!({
        "cases": [{"source_arrays": cases[0]["source_arrays"].clone()}]
    });
    let matrix_for_case = |case: &serde_json::Value| {
        let event = case["renderer_gl_calls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|event| event["call"] == "glUniformMatrix4fv" && event["location"] == 0)
            .expect("V5 native f32 projection uniform");
        assert_eq!(event["program"], 6);
        let values = event["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap() as f32)
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 16);
        values
    };
    let matrix_values = matrix_for_case(&cases[0]);
    let matrix_words = matrix_values
        .iter()
        .map(|value| value.to_bits())
        .collect::<Vec<_>>();
    assert_eq!(
        matrix_words,
        matrix_for_case(&cases[1])
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
    let matrix_values = matrix_values
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let viewport = cases[0]["renderer_draw_context"]["viewport_at_readback"]
        .as_array()
        .expect("V5 readback viewport")
        .iter()
        .map(|value| value.as_u64().unwrap() as u32)
        .collect::<Vec<_>>();
    assert_eq!(viewport, [0, 0, WIDTH, HEIGHT]);
    assert_eq!(
        cases[1]["renderer_draw_context"]["viewport_at_readback"],
        cases[0]["renderer_draw_context"]["viewport_at_readback"]
    );
    let svg_transform = svg_matrix_from_native(&matrix_values, &viewport);

    let mut manifest_cases = Vec::new();
    for (case, name, argb, alpha_byte) in [
        (&cases[0], "alpha128", 0x8000_0000_u32, 128_u8),
        (&cases[1], "alpha255", 0xff00_0000_u32, 255_u8),
    ] {
        assert_eq!(case["saved_color_argb_control"], format!("0x{argb:08x}"));
        assert_eq!(case["object_color_set_get_argb"], format!("0x{argb:08x}"));
        assert_eq!(
            case["effective_selected_pen_state_after_drawing"]["color_argb"],
            serde_json::json!(argb)
        );
        let stroke = source_stroke_argb(&source_receipt, false, argb);
        let prepared = crate::ink::prepare_stroke_for_context(
            &stroke,
            false,
            crate::ink::InkRenderContext::CanonicalPage,
        );
        assert!(
            prepared.fountain_v17_rgb.is_none(),
            "no V17 metadata is synthesized"
        );
        let expected_alpha = f32::from(alpha_byte) / 255.0;
        assert_eq!(
            (prepared.opacity as f32).to_bits(),
            expected_alpha.to_bits(),
            "saved ARGB alpha reaches production V16 opacity"
        );
        let plan = V16Plan::prepare(&prepared).expect("production V16Plan for alpha control");
        assert_eq!(plan.stamps.len(), 76);
        assert!(
            plan.stamps
                .iter()
                .all(|stamp| f32::from_bits(stamp.gradient_key.alpha) == 1.0)
        );
        compare_alpha_plan_to_native(&plan, case);

        let color_uniforms = case["drawable_color_uniforms"]
            .as_array()
            .expect("V5 draw-associated inputColor uniforms");
        let matching_uniforms = color_uniforms
            .iter()
            .filter(|event| {
                event["call"] == "glUniform4fv" && event["program"] == 6 && event["location"] == 1
            })
            .collect::<Vec<_>>();
        assert_eq!(
            matching_uniforms.len(),
            1,
            "one joined program-6 inputColor write per case"
        );
        let color = matching_uniforms[0];
        let native_alpha = color["values"][3].as_f64().unwrap() as f32;
        assert_eq!(native_alpha.to_bits(), expected_alpha.to_bits());
        assert_eq!(
            color["values"],
            serde_json::json!([0.0, 0.0, 0.0, f64::from(expected_alpha)])
        );
        assert_eq!(color["observed_current_program"], 6);
        assert_eq!(color["program_is_live"], true);
        assert_eq!(color["program_link_status"], 1);
        assert!(
            !color["matched_draw_sequences"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            case["renderer_draw_context"]["draw_program_ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|program| program.as_u64() == Some(6))
        );
        assert_eq!(
            case["native_upload_checkpoint"]["vector_sha256"],
            V5_UPLOAD_SHA256
        );
        let native = captured_alpha_frame(&repo, &receipt, name);

        let production = retained_scene_with_default_ink(&prepared, svg_transform);
        let production_svg = production.page.svg.clone();
        let production_rgba = raster_svg(&production_svg);
        let production_plan_records = plan.radial_circle_records();
        let default_sidecar_policy = prepared.fountain_v17_rgb.is_some()
            || prepared.opacity == 1.0
            || (prepared.opacity.is_finite()
                && (0.0..=1.0).contains(&prepared.opacity)
                && plan.inverse_scale == 1.0);
        let default_sidecar_allowed = production_plan_records.is_some() && default_sidecar_policy;
        assert_eq!(
            production.ink.len(),
            usize::from(default_sidecar_allowed),
            "production scene retains only the actual default sidecar admission"
        );
        assert!(
            plan.native_quad_geometry(&prepared, true),
            "the captured unit-scale full-coverage cases use production native-quad geometry"
        );
        if let Some(records) = &production_plan_records
            && default_sidecar_allowed
        {
            let entries = production.ink.iter().collect::<Vec<_>>();
            assert_eq!(entries.len(), 1);
            match &entries[0].1.payload {
                crate::render::NativeInkPayload::RadialCircles {
                    records: registered,
                    saved_alpha,
                } => {
                    assert_eq!(registered, records);
                    assert_eq!(saved_alpha.to_bits(), expected_alpha.to_bits());
                }
                _ => panic!("production admitted an unexpected native ink payload"),
            }
        }
        let (production_pdf, production_images, production_shadings) =
            vector_pdf_summary(&production);
        let native_alpha = f32::from(alpha_byte) / 255.0;
        if default_sidecar_allowed {
            assert_circle_sidecar_saved_alpha(&production_pdf, native_alpha, plan.stamps.len());
        } else {
            assert!(
                circle_sidecar_form_signatures(&production_pdf, plan.stamps.len()).is_empty(),
                "production PDF has no unregistered circle sidecar"
            );
        }

        // The legacy path is reconstructed independently from the same
        // production plan, but its PDF deliberately drops native ink so it
        // preserves the pre-sidecar SVG-gradient circle baseline.
        let legacy_circle = retained_scene(&prepared, svg_transform, false, true);
        assert!(legacy_circle.ink.is_empty());
        let legacy_circle_svg = legacy_circle.page.svg.clone();
        assert_eq!(
            legacy_circle_svg.matches("<circle").count(),
            plan.stamps.len()
        );
        assert_eq!(legacy_circle_svg.matches("<rect").count(), 0);
        let legacy_circle_rgba = raster_svg(&legacy_circle_svg);
        let (legacy_circle_pdf, legacy_pdf_images, legacy_pdf_shadings) =
            vector_pdf_summary(&legacy_circle);
        assert!(circle_sidecar_form_signatures(&legacy_circle_pdf, plan.stamps.len()).is_empty());
        let independent_quad_svg = quad_geometry_svg(&legacy_circle_svg, plan.stamps.len());
        assert_eq!(
            svg_element_signature(&independent_quad_svg),
            svg_element_signature(&production_svg),
            "production quad matches the independent rewrite of the legacy circle SVG"
        );
        let independent_quad_rgba = raster_svg(&independent_quad_svg);
        assert_eq!(
            independent_quad_rgba, production_rgba,
            "independent quad and actual production quad rasterize identically"
        );
        let production_circle_signature =
            circle_sidecar_form_signatures(&production_pdf, plan.stamps.len());
        if default_sidecar_allowed {
            assert_eq!(production_circle_signature.len(), 1);
        }

        write_new(
            output_dir.join(format!("{name}.production.svg")),
            production_svg.as_bytes(),
        );
        write_new(
            output_dir.join(format!("{name}.production.pdf")),
            &production_pdf,
        );
        write_new(
            output_dir.join(format!("{name}.production.rgba")),
            &production_rgba,
        );
        write_new(
            output_dir.join(format!("{name}.native-topdown.rgba")),
            &native,
        );
        write_new(
            output_dir.join(format!("{name}.legacy-circle.svg")),
            legacy_circle_svg.as_bytes(),
        );
        write_new(
            output_dir.join(format!("{name}.legacy-circle.pdf")),
            &legacy_circle_pdf,
        );
        write_new(
            output_dir.join(format!("{name}.legacy-circle.rgba")),
            &legacy_circle_rgba,
        );

        let mut quad_experiment = None;
        let mut partial_circle_pdf = None;
        if alpha_byte == 128 {
            assert!(
                default_sidecar_allowed,
                "unit-scale partial alpha gets actual default circle sidecar"
            );
            let circle_pdf = production_pdf.clone();
            let circle_signature = circle_sidecar_form_signatures(&circle_pdf, plan.stamps.len());
            assert_eq!(circle_signature, production_circle_signature);
            assert_circle_sidecar_saved_alpha(&circle_pdf, expected_alpha, plan.stamps.len());
            write_new(
                output_dir.join("alpha128.quad-experiment.svg"),
                independent_quad_svg.as_bytes(),
            );
            write_new(
                output_dir.join("alpha128.quad-experiment.rgba"),
                &independent_quad_rgba,
            );
            write_new(
                output_dir.join("alpha128.partial-circle-sidecar-confirmation.pdf"),
                &circle_pdf,
            );
            quad_experiment = Some(serde_json::json!({
                "svg_path": "alpha128.quad-experiment.svg",
                "rgba_path": "alpha128.quad-experiment.rgba",
                "geometry": "independent source-pinned [-1,1]^2 rectangle per partial-alpha stamp; existing radial gradient, transform, order, and opacity preserved",
                "stamp_count": plan.stamps.len(),
                "rgba_sha256": sha256(&independent_quad_rgba),
                "against_production_quad_rgba": rgba_difference(&production_rgba, &independent_quad_rgba),
                "parsed_svg_matches_production": true,
                "pixel_equality_required": false,
            }));
            partial_circle_pdf = Some(serde_json::json!({
                "path": "alpha128.partial-circle-sidecar-confirmation.pdf",
                "origin": "byte-identical confirmation copy of the actual production PDF, whose default V16Plan/native ink admission uses the existing radial_circle_records and saved alpha",
                "saved_alpha_applied_once_per_stamp": expected_alpha,
                "stamp_count": plan.stamps.len(),
                "vector_forms": circle_signature.len(),
                "form_signature_matches_production": circle_signature == production_circle_signature,
                "pdf_image_objects": production_images,
                "pdf_sha256": sha256(&circle_pdf),
                "production_output": true,
            }));
        }
        manifest_cases.push(serde_json::json!({
            "case": name,
            "saved_argb": format!("0x{argb:08x}"),
            "native_uniform_alpha": native_alpha,
            "native_inputColor_program": color["program"],
            "native_inputColor_location": color["location"],
            "native_inputColor_matched_draw_sequences": color["matched_draw_sequences"],
            "source_payload_sha256": SOURCE_ARRAYS_SHA256,
            "source_point_count": 204,
            "native_upload_sha256": V5_UPLOAD_SHA256,
            "plan_stamp_count": plan.stamps.len(),
            "native_default_circle_sidecar_admitted": default_sidecar_allowed,
            "production_uses_native_quad_geometry": plan.native_quad_geometry(&prepared, true),
            "production_svg_path": format!("{name}.production.svg"),
            "production_pdf_path": format!("{name}.production.pdf"),
            "production_rgba_path": format!("{name}.production.rgba"),
            "native_rgba_path": format!("{name}.native-topdown.rgba"),
            "legacy_circle_svg_path": format!("{name}.legacy-circle.svg"),
            "legacy_circle_pdf_path": format!("{name}.legacy-circle.pdf"),
            "legacy_circle_rgba_path": format!("{name}.legacy-circle.rgba"),
            "production_svg_sha256": sha256(production_svg.as_bytes()),
            "production_pdf_sha256": sha256(&production_pdf),
            "production_rgba_sha256": sha256(&production_rgba),
            "native_rgba_sha256_topdown": sha256(&native),
            "production_vs_native_rgba": rgba_difference(&production_rgba, &native),
            "production_vs_legacy_circle_rgba": rgba_difference(&production_rgba, &legacy_circle_rgba),
            "legacy_circle_svg_sha256": sha256(legacy_circle_svg.as_bytes()),
            "legacy_circle_pdf_sha256": sha256(&legacy_circle_pdf),
            "legacy_circle_rgba_sha256": sha256(&legacy_circle_rgba),
            "production_pdf_image_objects": production_images,
            "production_pdf_radial_shadings": production_shadings,
            "legacy_circle_pdf_image_objects": legacy_pdf_images,
            "legacy_circle_pdf_radial_shadings": legacy_pdf_shadings,
            "production_circle_sidecar_form_signature": production_circle_signature,
            "partial_alpha_quad_experiment": quad_experiment,
            "partial_alpha_circle_sidecar_confirmation": partial_circle_pdf,
            "native_vs_rust_pixel_equality_required": false,
        }));
    }
    let manifest = serde_json::json!({
        "schema": "sdocx.fountain.as18-glv16.alpha-pair-artifacts.v2",
        "scope": "same pinned persisted 204-point AS18 input and native f32 matrix/upload across V5 saved-alpha 128/255 controls; rainbow stays disabled; retains explicit legacy circle SVG/PDF baselines and records actual bounded production quad SVG/PDF, the actual saved-alpha radial-circle PDF sidecar, and an independently rewritten partial-alpha quad SVG",
        "provenance": "V5 source frame, arrays, RTV5 upload/AddPoint geometry, inputColor uniform, and two Mesa readbacks are hash-checked. This is a class-level prepared-stroke comparison, not Samsung device or PDFium/MuPDF parity.",
        "source_payload_sha256": SOURCE_ARRAYS_SHA256,
        "source_frame_sha256": SOURCE_FRAME_SHA256,
        "native_upload_sha256": V5_UPLOAD_SHA256,
        "v5_receipt_sha256": V5_RECEIPT_SHA256,
        "v5_candidate_pins_sha256": V5_PINS_SHA256,
        "canvas_px": [WIDTH, HEIGHT],
        "native_gl_uniform_matrix4fv_program6_location0_f32": matrix_values,
        "svg_transform_from_native_matrix_and_gl_read_origin": svg_transform,
        "native_execution": false,
        "artifact_render_executed": true,
        "production_gate": "current bounded ordinary V16 admission: source alphaByWidth lanes remain full coverage; unit-scale ordinary strokes use the source-pinned quad SVG and radial-circle PDF sidecar with saved alpha; existing production selection was not changed by this artifact writer",
        "predecessor_v1_artifacts_modified": false,
        "cases": manifest_cases,
    });
    write_new(
        output_dir.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).unwrap(),
    );
}

#[test]
#[ignore = "writes thin-width AS18 GLV16 SVG/PDF artifacts from the complete pinned V8 native pair"]
fn write_as18_glv16_thin_svg_pdf_pairs() {
    const PAIR_SHA256: &str = "0204f19ed9d325b78dce5e050e17082e6957d76d7f64efdaf0663aa42dc7cae8";
    const SOURCE_PAYLOAD_SHA256: &str =
        "c01022129e9a3a09ee95dc5935d5e177bd507e03d6d9dbd357a5278a64ab72b4";
    const SOURCE_FRAME_SHA256: &str =
        "490fa32ab9d69416e4c48dec88b26ad14e362d7b08eb3e1b6a9ada47ce2980c2";
    const NATIVE_UPLOAD_SHA256: &str =
        "621623590e5818b88167e910cce8dca9b0478790dcc7d95b2e55b0e7eb606a36";

    let pair_path = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_204_THIN_V8_PAIR")
            .expect("set SDOCX_FOUNTAIN_V16_204_THIN_V8_PAIR to the pinned V8 pair JSON"),
    );
    let output_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_204_THIN_V8_ARTIFACT_DIR")
            .expect("set SDOCX_FOUNTAIN_V16_204_THIN_V8_ARTIFACT_DIR to a fresh output directory"),
    );
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root");

    let pair_bytes = fs::read(&pair_path).expect("complete V8 pair receipt");
    assert_eq!(sha256(&pair_bytes), PAIR_SHA256);
    let pair: serde_json::Value = serde_json::from_slice(&pair_bytes).unwrap();
    const BLEND_JOIN_SHA256: &str =
        "44d68f251a6d06b5126e89e5702a7128ce3ebfe30e7cb3292639a6812d60b2e8";
    let blend_join_path = repo.join(
        "tmp/vector-parity-plan/fountain-as18-v16-thin-native-quad-blend-joins-v1/native-quad-blend-joins.json",
    );
    let blend_join_bytes = fs::read(&blend_join_path).expect("pinned native blend/source join");
    assert_eq!(sha256(&blend_join_bytes), BLEND_JOIN_SHA256);
    let blend_join: serde_json::Value = serde_json::from_slice(&blend_join_bytes).unwrap();
    assert_eq!(
        blend_join["schema"],
        "sdocx-fountain-v16-thin-native-quad-blend-join-v1"
    );
    assert_eq!(
        blend_join["status"],
        "offline_source_and_retained_trace_join_complete"
    );
    assert_eq!(
        blend_join["source_pins"]["v8_pair_receipt_sha256"],
        PAIR_SHA256
    );
    assert_eq!(
        blend_join["source_pins"]["fountainpen_dso_sha256"],
        FOUNTAIN_PEN_DSO_SHA256
    );
    let blend_cases = blend_join["cases"]
        .as_array()
        .expect("two source-over cases");
    assert_eq!(blend_cases.len(), 2);
    assert_eq!(blend_cases[0]["case"], "alpha128");
    assert_eq!(blend_cases[1]["case"], "alpha255");
    assert_eq!(
        pair["schema"],
        "sdocx-fountain-saved-renderer-experiment-v1"
    );
    assert_eq!(pair["source_input"]["point_count"], 204);
    assert_eq!(
        pair["source_input"]["payload_sha256"],
        SOURCE_PAYLOAD_SHA256
    );
    assert_eq!(pair["source_input"]["frame_sha256"], SOURCE_FRAME_SHA256);
    assert_eq!(
        pair["source_getter_arrays"]["retention"],
        "complete actual Model getters; no diagnostic prefix cap in complete pair receipt"
    );
    assert_eq!(
        pair["source_getter_arrays"]["verified_against_pinned_natural_input"],
        true
    );
    let expected_counts = [
        ("points_f32_bits", 408usize),
        ("pressure_f32_bits", 204),
        ("tilt_f32_bits", 204),
        ("orientation_f32_bits", 204),
        ("timestamps_i32", 204),
    ];
    let cases = pair["cases"].as_array().expect("V8 captured cases");
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0]["case"], "alpha128");
    assert_eq!(cases[1]["case"], "alpha255");
    assert_eq!(cases[0]["source_arrays"], cases[1]["source_arrays"]);
    for case in cases {
        assert_eq!(case["source_point_count"], 204);
        assert_eq!(case["source_model_get_point_count"], 204);
        assert_eq!(case["source_payload_sha256"], SOURCE_PAYLOAD_SHA256);
        assert_eq!(case["source_frame_sha256"], SOURCE_FRAME_SHA256);
        let arrays = case["source_arrays"]
            .as_object()
            .expect("complete getter arrays");
        for (key, count) in expected_counts {
            assert_eq!(
                arrays[key].as_array().expect("complete getter array").len(),
                count,
                "source getter array {key}"
            );
            assert_eq!(
                pair["source_getter_arrays"]["expected_item_counts"][key],
                count
            );
        }
    }
    let checks = pair["capture_diagnostics"]["pairwise_checks"]
        .as_object()
        .expect("V8 pairwise checks");
    assert!(!checks.is_empty());
    assert!(checks.values().all(|value| value == true));
    assert_eq!(
        pair["capture_diagnostics"]["status"],
        "host_mesa_pair_captured_no_device_or_document_parity_claim"
    );
    fs::create_dir(&output_dir).expect("thin V8 artifact output directory must be new");

    let native_frame = |name: &str, expected_raw_sha: &str| {
        let artifact = pair["pixel_artifacts"]
            .as_array()
            .expect("retained readback artifacts")
            .iter()
            .find(|artifact| artifact["key"] == name)
            .expect("case readback artifact");
        assert_eq!(artifact["cases"], serde_json::json!([name]));
        assert_eq!(
            artifact["format"],
            "zlib(RGBA8 glReadPixels rows, lower-left first)"
        );
        let artifact_path = Path::new(artifact["path"].as_str().unwrap());
        let path = if artifact_path.is_absolute() {
            artifact_path.to_owned()
        } else {
            repo.join(artifact_path)
        };
        let compressed = fs::read(path).expect("V8 retained compressed Mesa frame");
        assert_eq!(sha256(&compressed), artifact["compressed_sha256"]);
        let mut decoder = ZlibDecoder::new(compressed.as_slice());
        let mut raw = Vec::new();
        decoder.read_to_end(&mut raw).expect("decode V8 Mesa frame");
        assert_eq!(raw.len() as u64, artifact["raw_bytes"].as_u64().unwrap());
        assert_eq!(raw.len(), (WIDTH * HEIGHT * 4) as usize);
        assert_eq!(sha256(&raw), expected_raw_sha);
        topdown_frame(&raw)
    };

    let mut output_cases = Vec::new();
    let mut native_frames = Vec::new();
    for (case, name, argb, alpha_byte, frame_sha) in [
        (
            &cases[0],
            "alpha128",
            0x8000_0000_u32,
            128_u8,
            "7516b081b2a3b56faa364b0c5a4bbaa6ec550c678cd6469c0f272b492d8927ea",
        ),
        (
            &cases[1],
            "alpha255",
            0xff00_0000_u32,
            255_u8,
            "59dd11d7bea8825aea4974399c0495c93b61a2e44bd81c4d1dd78ddbbb157466",
        ),
    ] {
        assert_eq!(case["saved_color_argb_control"], format!("0x{argb:08x}"));
        assert_eq!(case["object_color_set_get_argb"], format!("0x{argb:08x}"));
        assert_eq!(
            case["effective_selected_pen_state_after_drawing"]["color_argb"],
            argb
        );
        assert_eq!(
            case["effective_selected_pen_state_after_drawing"]["rainbow_enabled"],
            0
        );
        assert_eq!(case["controlled_object_size_input_f32_bits"], "0000803f");
        assert_eq!(case["controlled_object_size_get_f32_bits"], "0000803f");
        assert_eq!(case["native_pen_getsize_f32_bits"], "0000803f");
        let size_calls = case["native_pen_setsize_calls"].as_array().unwrap();
        assert!(
            size_calls
                .iter()
                .any(|call| { call["input_s0"] == "0000803f" && call["return_w0"] == 1 })
        );
        assert_eq!(case["selected_drawable_class"], "GLV16");
        assert_eq!(
            case["native_addpoint_raw_y_range"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(case["rtv_addpoint_entries"].as_array().unwrap().len(), 76);
        assert_eq!(
            case["native_rtv5_s3_f32_bits"].as_array().unwrap().len(),
            76
        );
        assert_eq!(
            case["native_rtv5_upload_owner_join"]["addpoint_entry_count"],
            76
        );
        assert_eq!(case["native_upload_checkpoint"]["vector_bytes"], 2128);
        assert_eq!(
            case["native_upload_checkpoint"]["vector_sha256"],
            NATIVE_UPLOAD_SHA256
        );
        assert_eq!(
            case["native_upload_checkpoint"]["vector_hex_omitted"],
            false
        );
        assert_eq!(
            case["native_upload_checkpoint"]["vector_object"],
            case["native_set_buffer_calls"][0]["vector_one"]["ptr"]
        );
        assert_eq!(
            case["native_upload_checkpoint"]["receiver"],
            case["send_data_to_gpu_calls"][0]["receiver"]
        );
        assert_eq!(
            case["native_upload_checkpoint"]["vector_hex"]
                .as_str()
                .unwrap()
                .len(),
            2128 * 2
        );
        assert_eq!(
            case["native_rtv5_upload_owner_join"]["upload_vector_matches_send_and_task"],
            true
        );
        assert_eq!(
            case["native_rtv5_upload_owner_join"]["addpoint_owner_matches_renderer_receiver"],
            true
        );
        assert_eq!(
            case["native_rtv5_upload_owner_join"]["all_rt_owners_match"],
            true
        );
        assert_eq!(
            case["native_rtv5_upload_owner_join"]["all_setbuffer_owners_match"],
            true
        );
        assert_eq!(case["renderer_gl_draw_events"].as_array().unwrap().len(), 1);
        assert_eq!(
            case["renderer_gl_draw_events"][0]["call"],
            "glDrawArraysInstanced"
        );
        assert_eq!(
            case["renderer_gl_draw_events"][0]["values"],
            serde_json::json!([5, 0, 4, 76])
        );
        assert_eq!(
            case["renderer_gl_draw_events"][0]["observed_current_program"],
            6
        );
        assert_eq!(
            case["renderer_draw_context"]["viewport_at_readback"],
            serde_json::json!([0, 0, WIDTH, HEIGHT])
        );
        assert_eq!(
            case["native_upload_checkpoint"]["vector_sha256"],
            NATIVE_UPLOAD_SHA256
        );

        let uniform_rows = case["drawable_color_uniforms"].as_array().unwrap();
        assert_eq!(uniform_rows.len(), 1);
        let uniform = &uniform_rows[0];
        assert_eq!(uniform["program"], 6);
        assert_eq!(uniform["location"], 1);
        assert_eq!(uniform["observed_current_program"], 6);
        assert_eq!(uniform["program_is_live"], true);
        assert_eq!(uniform["program_link_status"], 1);
        let expected_alpha = f32::from(alpha_byte) / 255.0;
        assert_eq!(uniform["values"][0].as_f64().unwrap() as f32, 0.0);
        assert_eq!(uniform["values"][1].as_f64().unwrap() as f32, 0.0);
        assert_eq!(uniform["values"][2].as_f64().unwrap() as f32, 0.0);
        assert_eq!(
            uniform["values"][3].as_f64().unwrap() as f32,
            expected_alpha
        );
        let native_draw = &case["renderer_gl_draw_events"][0];
        assert_eq!(uniform["egl_context"], native_draw["egl_context"]);
        assert_eq!(uniform["program_create_generation"], 1);
        assert_eq!(uniform["program_link_generation"], 1);
        assert_eq!(
            uniform["matched_draw_sequences"],
            serde_json::json!([native_draw["program_lifecycle_sequence"]])
        );

        let stroke = source_stroke_argb_width(&pair, false, argb, 1.0);
        let prepared = crate::ink::prepare_stroke_for_context(
            &stroke,
            false,
            crate::ink::InkRenderContext::CanonicalPage,
        );
        assert!(prepared.fountain_v17_rgb.is_none());
        assert_eq!(
            (prepared.opacity as f32).to_bits(),
            expected_alpha.to_bits()
        );
        // `PreparedStroke::width` is generic display fallback width, derived
        // from public `Stroke::pen_width` by normalized_stroke_width (/2.5,
        // clamped to 0.4..12). Fountain V16 geometry consumes saved style
        // `pen_size` instead; the following native-upload join checks that
        // actual geometry rather than treating this fallback width as native.
        assert_eq!((prepared.width as f32).to_bits(), 0.4_f32.to_bits());
        let plan = V16Plan::prepare(&prepared).expect("thin-width V16Plan");
        assert_eq!(plan.stamps.len(), 76);
        assert!(plan.native_quad_geometry(&prepared, false));
        assert!(!plan.native_quad_geometry(&prepared, true));
        assert!(plan.radial_circle_records().is_none());
        let plan_min = plan
            .stamps
            .iter()
            .map(|stamp| f32::from_bits(stamp.gradient_key.alpha))
            .fold(f32::INFINITY, f32::min);
        let plan_max = plan
            .stamps
            .iter()
            .map(|stamp| f32::from_bits(stamp.gradient_key.alpha))
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(plan_min.is_finite() && plan_max.is_finite());
        assert!(plan_min >= 0.0 && plan_max <= 1.0 && plan_min < 1.0);

        let case_diagnostic = serde_json::json!({
            "state": { "current": {
                "rtv_addpoint_entries": case["rtv_addpoint_entries"],
                "native_upload_checkpoint": case["native_upload_checkpoint"]
            }}
        });
        let native_upload = native_upload_records_with_sha(&case_diagnostic, NATIVE_UPLOAD_SHA256);
        compare_plan_to_native_records(&plan, &case_diagnostic, &native_upload);

        // This separate Type 4 experiment follows the captured source-over
        // draw contract. Upload lane 4 is native transparency (1.0 here),
        // lane 6 is alphaByWidth, and program-6 inputColor alpha is applied
        // separately once by the DirectSourceOver compiler.
        let blend = &blend_cases[if name == "alpha128" { 0 } else { 1 }];
        assert_eq!(blend["program"], 6);
        assert_eq!(blend["program_generation"][1], 6);
        assert_eq!(blend["program_generation"][2], 1);
        assert_eq!(blend["program_generation"][3], 1);
        assert_eq!(blend["addpoint_count"], 76);
        assert_eq!(blend["draw_values"], serde_json::json!([5, 0, 4, 76]));
        assert_eq!(blend["instance_upload_bytes"], 2128);
        assert_eq!(blend["instance_upload_sha256"], NATIVE_UPLOAD_SHA256);
        assert_eq!(blend["source_over"]["blend_enabled"], true);
        assert_eq!(
            blend["source_over"]["equation_separate"],
            serde_json::json!([32774, 32774])
        );
        assert_eq!(
            blend["source_over"]["function_separate"],
            serde_json::json!([1, 771, 1, 771])
        );
        assert_eq!(
            blend["source_over"]["no_blend_state_change_until_draw"],
            true
        );
        assert_eq!(
            blend["native_owner_vector_join"]["all_rt_owners_match"],
            true
        );
        assert_eq!(
            blend["native_owner_vector_join"]["all_setbuffer_owners_match"],
            true
        );
        assert_eq!(
            blend["native_owner_vector_join"]["upload_vector_matches_send_and_task"],
            true
        );
        assert_eq!(
            blend["input_color_rgba"][3].as_f64().unwrap() as f32,
            expected_alpha
        );
        assert_eq!(blend["input_color_rgba"][0].as_f64(), Some(0.0));
        assert_eq!(blend["input_color_rgba"][1].as_f64(), Some(0.0));
        assert_eq!(blend["input_color_rgba"][2].as_f64(), Some(0.0));

        let mut type4_records = Vec::with_capacity(native_upload.len());
        for (index, lanes) in native_upload.iter().enumerate() {
            let [cx, cy, rx, ry, transparency, inner, width_alpha] = lanes.map(f32::from_bits);
            let observed_s3 = f32::from_bits(decode_little_endian_word(
                case["native_rtv5_s3_f32_bits"][index].as_str().unwrap(),
            ));
            assert_eq!(
                observed_s3.to_bits(),
                transparency.to_bits(),
                "native S3/lane4 {index}"
            );
            assert_eq!(transparency.to_bits(), 1.0_f32.to_bits());
            assert!(cx.is_finite() && cy.is_finite());
            assert!(rx.is_finite() && ry.is_finite() && rx > 0.0 && ry > 0.0);
            assert!(inner.is_finite() && (0.0..0.5).contains(&inner));
            assert!(width_alpha.is_finite() && (0.0..=1.0).contains(&width_alpha));
            let coverage = transparency * width_alpha;
            type4_records.push([cx, cy, rx, ry, inner, coverage, 0.0, 0.0, 0.0]);
        }

        let native = native_frame(name, frame_sha);
        native_frames.push(native.clone());
        let matrix_events = case["renderer_gl_calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| {
                event["call"] == "glUniformMatrix4fv"
                    && event["location"] == 0
                    && event["program"] == 6
            })
            .collect::<Vec<_>>();
        assert_eq!(matrix_events.len(), 1);
        let matrix_values = matrix_events[0]["values"]
            .as_array()
            .expect("captured program-6 native projection")
            .iter()
            .map(|value| value.as_f64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(matrix_values.len(), 16);
        assert!(matrix_values.iter().all(|value| value.is_finite()));
        let viewport = case["renderer_draw_context"]["viewport_at_readback"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect::<Vec<_>>();
        let svg_transform = svg_matrix_from_native(&matrix_values, &viewport);
        let production_svg = public_svg_with_default_ink(&prepared, svg_transform);
        assert_eq!(production_svg.matches("<rect").count(), plan.stamps.len());
        assert_eq!(production_svg.matches("<circle").count(), 0);
        let production_raster = raster_svg(&production_svg);

        // The retained-PDF route keeps thin width-alpha on the legacy circle
        // vector fallback. It must not register the full-coverage native circle
        // sidecar, whose admission requires every per-stamp coverage to be 1.
        let production = retained_scene_with_default_ink(&prepared, svg_transform);
        assert!(
            production.ink.is_empty(),
            "thin width-alpha keeps PDF vector fallback without circle sidecar"
        );
        let retained_pdf_svg = production.page.svg.clone();
        assert_eq!(
            retained_pdf_svg.matches("<circle").count(),
            plan.stamps.len()
        );
        assert_eq!(retained_pdf_svg.matches("<rect").count(), 0);
        let (production_pdf, production_pdf_images, production_pdf_shadings) =
            vector_pdf_summary(&production);
        assert!(circle_sidecar_form_signatures(&production_pdf, plan.stamps.len()).is_empty());

        let mut type4_bounds = crate::BoundingBox {
            x_min: f64::INFINITY,
            y_min: f64::INFINITY,
            x_max: f64::NEG_INFINITY,
            y_max: f64::NEG_INFINITY,
        };
        for record in &type4_records {
            type4_bounds.x_min = type4_bounds
                .x_min
                .min(f64::from(record[0]) - f64::from(record[2]));
            type4_bounds.y_min = type4_bounds
                .y_min
                .min(f64::from(record[1]) - f64::from(record[3]));
            type4_bounds.x_max = type4_bounds
                .x_max
                .max(f64::from(record[0]) + f64::from(record[2]));
            type4_bounds.y_max = type4_bounds
                .y_max
                .max(f64::from(record[1]) + f64::from(record[3]));
        }
        let type4_scene = retained_scene_with_direct_type4_ink(
            &prepared,
            svg_transform,
            &type4_records,
            type4_bounds,
            expected_alpha,
        );
        let type4_entry = type4_scene.ink.iter().next().expect("Type 4 payload");
        assert_eq!(type4_scene.ink.len(), 1);
        assert_eq!(type4_entry.1.opacity, 1.0);
        assert_eq!(type4_entry.1.bounds, type4_bounds);
        match &type4_entry.1.payload {
            crate::render::native_ink::NativeInkPayload::Radial {
                records,
                saved_alpha,
                enhanced_antialias,
            } => {
                assert_eq!(records, &type4_records);
                assert_eq!(saved_alpha.to_bits(), expected_alpha.to_bits());
                assert!(!enhanced_antialias);
            }
            _ => panic!("experimental field must stay the existing radial Type 4 payload"),
        }
        let (type4_pdf, type4_images, type4_type1_shadings) =
            type4_vector_pdf_summary(&type4_scene);
        assert_eq!(type4_images, 0, "experimental Type 4 output is vector-only");
        let type4_programs = type4_function_programs(&type4_pdf);
        assert_eq!(
            type4_programs.len(),
            2,
            "existing compiler emits RGB and mask Type 4 functions"
        );
        assert!(type4_programs.iter().all(|program| !program.is_empty()));
        assert!(
            type4_type1_shadings > 0,
            "Type 4 candidate contains vector shading objects"
        );

        // Keep an explicit legacy-circle public SVG baseline and an
        // independent quad rewrite for comparison with the public route.
        let legacy_circle_svg = circle_geometry_svg(&production_svg, plan.stamps.len());
        assert_eq!(
            legacy_circle_svg.matches("<circle").count(),
            plan.stamps.len()
        );
        assert_eq!(legacy_circle_svg.matches("<rect").count(), 0);
        let legacy_circle_raster = raster_svg(&legacy_circle_svg);
        assert_eq!(raster_svg(&retained_pdf_svg), legacy_circle_raster);
        let legacy_pdf_images = production_pdf_images;
        let legacy_pdf_shadings = production_pdf_shadings;

        // Independently convert the retained circle baseline back to the
        // static RTV5 quad, then compare its raster with the actual renderer.
        let independent_quad_svg = quad_geometry_svg(&legacy_circle_svg, plan.stamps.len());
        let independent_quad_raster = raster_svg(&independent_quad_svg);
        assert_eq!(production_raster, independent_quad_raster);
        write_new(
            output_dir.join(format!("{name}.production.svg")),
            production_svg.as_bytes(),
        );
        write_new(
            output_dir.join(format!("{name}.production.pdf")),
            &production_pdf,
        );
        write_new(
            output_dir.join(format!("{name}.type4-direct-source-over-experiment.pdf")),
            &type4_pdf,
        );
        write_new(
            output_dir.join(format!("{name}.retained-pdf-source.svg")),
            retained_pdf_svg.as_bytes(),
        );
        write_new(
            output_dir.join(format!("{name}.production.rgba")),
            &production_raster,
        );
        write_new(
            output_dir.join(format!("{name}.native-topdown.rgba")),
            &native,
        );
        write_new(
            output_dir.join(format!("{name}.legacy-circle.svg")),
            legacy_circle_svg.as_bytes(),
        );
        write_new(
            output_dir.join(format!("{name}.legacy-circle.pdf")),
            &production_pdf,
        );
        write_new(
            output_dir.join(format!("{name}.legacy-circle.rgba")),
            &legacy_circle_raster,
        );
        write_new(
            output_dir.join(format!("{name}.independent-quad.svg")),
            independent_quad_svg.as_bytes(),
        );
        write_new(
            output_dir.join(format!("{name}.independent-quad.rgba")),
            &independent_quad_raster,
        );
        let type4_record_bytes = type4_records
            .iter()
            .flat_map(|record| record.iter())
            .flat_map(|value| value.to_bits().to_le_bytes())
            .collect::<Vec<_>>();
        let type4_experiment = serde_json::json!({
            "pdf_path": format!("{name}.type4-direct-source-over-experiment.pdf"),
            "source_join_receipt": "tmp/vector-parity-plan/fountain-as18-v16-thin-native-quad-blend-joins-v1/native-quad-blend-joins.json",
            "source_join_receipt_sha256": BLEND_JOIN_SHA256,
            "coverage_mapping": "native upload lane4 transparency * lane6 alphaByWidth; lane4 is source-joined to native RTV5 S3 and exactly 1.0 in these cases; native program-6 inputColor alpha is saved_alpha separately once",
            "record_count": type4_records.len(),
            "record_sha256": sha256(&type4_record_bytes),
            "saved_alpha": expected_alpha,
            "bounds_f64_exact_union_of_f32_records": [type4_bounds.x_min, type4_bounds.y_min, type4_bounds.x_max, type4_bounds.y_max],
            "pdf_image_objects": type4_images,
            "type_1_shading_objects": type4_type1_shadings,
            "function_type_4_programs": type4_programs.len(),
            "is_production_admission": false
        });
        output_cases.push(serde_json::json!({
            "case": name,
            "saved_argb": format!("0x{argb:08x}"),
            "explicit_native_object_and_style_size": 1.0,
            "prepared_generic_width_fallback": 0.4,
            "source_profile": "pinned 204-point archive input with explicit native ObjectStroke size=1.0 override; controlled thin-render fork, not archive-native thin widths",
            "source_getter_arrays": "complete V8 native Model getters, exact expected lengths and pair equality checked",
            "native_saved_alpha": expected_alpha,
            "native_addpoint_count": 76,
            "native_upload_sha256": NATIVE_UPLOAD_SHA256,
            "native_upload_records_compared_bitwise": true,
            "production_geometry": "current V16 unit quad for ordinary unit-scale uniform-color input, including native width-alpha variation",
            "production_pdf_geometry": "retained PDF sink preserves the native width-alpha circle fallback",
            "production_pdf_circle_sidecar_admitted": false,
            "production_svg_path": format!("{name}.production.svg"),
            "production_pdf_path": format!("{name}.production.pdf"),
            "type4_direct_source_over_experiment": type4_experiment,
            "retained_pdf_source_svg_path": format!("{name}.retained-pdf-source.svg"),
            "production_raster_path": format!("{name}.production.rgba"),
            "native_raster_path": format!("{name}.native-topdown.rgba"),
            "legacy_circle_svg_path": format!("{name}.legacy-circle.svg"),
            "legacy_circle_pdf_path": format!("{name}.legacy-circle.pdf"),
            "legacy_circle_pdf_same_as_retained_production_fallback": true,
            "legacy_circle_raster_path": format!("{name}.legacy-circle.rgba"),
            "retained_pdf_source_svg_geometry": "circles",
            "independent_quad_svg_path": format!("{name}.independent-quad.svg"),
            "independent_quad_raster_path": format!("{name}.independent-quad.rgba"),
            "production_quad_matches_independent_rewrite_rgba": true,
            "production_vs_legacy_circle_rgba": rgba_difference(&production_raster, &legacy_circle_raster),
            "production_vs_native_rgba": rgba_difference(&production_raster, &native),
            "production_pdf_image_objects": production_pdf_images,
            "production_pdf_radial_shadings": production_pdf_shadings,
            "legacy_circle_pdf_image_objects": legacy_pdf_images,
            "legacy_circle_pdf_radial_shadings": legacy_pdf_shadings,
            "pixel_equality_required": false
        }));
    }
    assert_ne!(native_frames[0], native_frames[1]);
    let manifest = serde_json::json!({
        "schema": "sdocx.fountain.as18-glv16.thin-width-artifacts.v3",
        "scope": "offline artifact emission from complete V8 native pair: public non-retained SVG uses production quad geometry; retained PDF preserves thin circle fallback without circle sidecar; explicit legacy-circle and independent-quad SVG rasters plus native frames are retained for comparison; no recovered V6 prefix or reconstructed getter tail",
        "source_pair_path": pair_path,
        "source_pair_sha256": PAIR_SHA256,
        "source_payload_sha256": SOURCE_PAYLOAD_SHA256,
        "source_frame_sha256": SOURCE_FRAME_SHA256,
        "native_upload_sha256": NATIVE_UPLOAD_SHA256,
        "native_execution": false,
        "production_admission_modified_by_artifact_writer": false,
        "type4_direct_source_over_experiment": {
            "receipt_path": "tmp/vector-parity-plan/fountain-as18-v16-thin-native-quad-blend-joins-v1/native-quad-blend-joins.json",
            "receipt_sha256": BLEND_JOIN_SHA256,
            "coverage_formula": "native lane4 transparency * lane6 alphaByWidth; saved program-6 ARGB alpha is a separate once-only compiler input",
            "not_production_admission": true,
            "no_appearance_parity_claim": true
        },
        "cases": output_cases
    });
    write_new(
        output_dir.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).unwrap(),
    );
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

#[test]
#[ignore = "compares rainbow-off/on V16 scale paths against pinned six-case native captures"]
fn v16_explicit_scale_matches_native_scale_sweep() {
    let capture_path = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_SCALE_SWEEP")
            .expect("set SDOCX_FOUNTAIN_V16_SCALE_SWEEP to the pinned native sweep JSON"),
    );
    let validation_path = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_SCALE_VALIDATION")
            .expect("set SDOCX_FOUNTAIN_V16_SCALE_VALIDATION to the postcapture receipt"),
    );
    let capture_bytes = fs::read(&capture_path).expect("pinned native scale-sweep capture");
    assert_eq!(sha256(&capture_bytes), SCALE_SWEEP_SHA256);
    let capture: serde_json::Value =
        serde_json::from_slice(&capture_bytes).expect("scale-sweep capture JSON");
    let validation_bytes = fs::read(&validation_path).expect("scale-sweep validation receipt");
    assert_eq!(sha256(&validation_bytes), SCALE_SWEEP_VALIDATION_SHA256);
    let validation: serde_json::Value =
        serde_json::from_slice(&validation_bytes).expect("scale-sweep validation JSON");
    assert_eq!(
        validation["status"],
        "PASS_CAPTURED_MATRIX_COMPOSITION_AND_ORDERED_GL_JOIN"
    );
    assert_eq!(validation["pins"]["fixture_sha256"], SCALE_SWEEP_SHA256);

    let rainbow_capture_path = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_RAINBOW_SCALE_SWEEP")
            .expect("set SDOCX_FOUNTAIN_V16_RAINBOW_SCALE_SWEEP to the pinned native capture"),
    );
    let rainbow_validation_path = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_RAINBOW_SCALE_VALIDATION")
            .expect("set SDOCX_FOUNTAIN_V16_RAINBOW_SCALE_VALIDATION to its validation receipt"),
    );
    let rainbow_capture_bytes =
        fs::read(&rainbow_capture_path).expect("pinned rainbow-on V16 scale capture");
    assert_eq!(sha256(&rainbow_capture_bytes), SCALE_SWEEP_RAINBOW_SHA256);
    let rainbow_capture: serde_json::Value =
        serde_json::from_slice(&rainbow_capture_bytes).expect("rainbow-on capture JSON");
    let rainbow_validation_bytes =
        fs::read(&rainbow_validation_path).expect("rainbow-on validation receipt");
    assert_eq!(
        sha256(&rainbow_validation_bytes),
        SCALE_SWEEP_RAINBOW_VALIDATION_SHA256
    );
    let rainbow_validation: serde_json::Value =
        serde_json::from_slice(&rainbow_validation_bytes).expect("rainbow-on validation JSON");
    assert_eq!(
        rainbow_validation["status"],
        "PASS_READ_ONLY_PAIRED_ARTIFACT_VALIDATION"
    );
    assert_eq!(
        rainbow_validation["pins"]["enabled_capture_sha256"],
        SCALE_SWEEP_RAINBOW_SHA256
    );
    assert_eq!(
        rainbow_validation["pins"]["rainbow_off_control_sha256"],
        SCALE_SWEEP_SHA256
    );
    assert_eq!(
        rainbow_validation["checks"]["all_six_full_rgba_frames_hash_valid_and_byte_equal_off_controls"],
        true
    );

    assert_eq!(capture["source_input"]["point_count"].as_u64(), Some(204));
    assert_eq!(
        rainbow_capture["source_input"]["point_count"].as_u64(),
        Some(204)
    );
    assert_eq!(capture["cases"].as_array().unwrap().len(), 6);
    assert_eq!(rainbow_capture["cases"].as_array().unwrap().len(), 6);
    let output_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_SCALE_ARTIFACT_DIR")
            .expect("set SDOCX_FOUNTAIN_V16_SCALE_ARTIFACT_DIR to a fresh output directory"),
    );
    fs::create_dir(&output_dir).expect("scale artifact output directory must be new");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut artifact_cases = Vec::new();
    let mut compact_geometry_cases = Vec::new();

    for (capture, rainbow_flag, prefix) in [
        (&capture, false, "rainbow-off"),
        (&rainbow_capture, true, "rainbow-on"),
    ] {
        let source_arrays = &capture["cases"][0]["source_arrays"];
        assert_eq!(capture["source_input"]["point_count"].as_u64(), Some(204));
        for case in capture["cases"].as_array().unwrap() {
            assert_eq!(case["source_arrays"], *source_arrays);
            assert_eq!(case["advanced_setting"], "18;0;100;");
            assert_eq!(case["selected_drawable_class"], "GLV16");
            assert_eq!(
                case["rainbow_enabled_control"].as_u64(),
                Some(u64::from(rainbow_flag))
            );
            assert_eq!(case["controlled_object_size_get_f32_bits"], "0000803f");
            assert_eq!(
                case["native_upload_checkpoint"]["vector_hex_omitted"],
                false
            );

            let scale = case["zoom_factor"].as_f64().unwrap() as f32;
            let argb = u32::from_str_radix(
                case["saved_color_argb_control"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            let stroke = source_stroke_argb_width(capture, rainbow_flag, argb, 1.0);
            let scaled = if rainbow_flag {
                crate::ink::prepare_stroke_for_context_with_fountain_v16_rainbow_ignored_output_scale(
                    &stroke,
                    crate::ink::InkRenderContext::CanonicalPage,
                    scale,
                )
            } else {
                crate::ink::prepare_stroke_for_context_with_fountain_v16_output_scale(
                    &stroke,
                    crate::ink::InkRenderContext::CanonicalPage,
                    scale,
                )
            };
            assert_eq!(scaled.support, crate::ink::InkSupport::Reconstructed);
            assert_eq!(scaled.fountain_output_scale, Some(scale));
            assert!(scaled.fountain_v17_rgb.is_none());
            let plan = V16Plan::prepare(&scaled).expect("production V16Plan for explicit scale");
            let native_count = case["renderer_instance_count_u32_at_plus272"]
                .as_u64()
                .unwrap() as usize;
            assert_eq!(plan.stamps.len(), native_count, "{} count", case["case"]);
            compare_scale_sweep_plan_to_native(&plan, case);

            let native_geometry = case["rtv_addpoint_entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| {
                    std::array::from_fn::<_, 3, _>(|lane| {
                        decode_little_endian_word(
                            entry["native_addpoint_abi_arguments"]["vector3_argument_f32_bits"]
                                [lane]
                                .as_str()
                                .unwrap(),
                        )
                    })
                })
                .collect::<Vec<_>>();
            for ((point, radius), expected) in scaled
                .points
                .iter()
                .zip(scaled.dot_radii.as_ref().unwrap())
                .zip(&native_geometry)
            {
                assert_eq!(
                    [
                        (point.x as f32).to_bits(),
                        (point.y as f32).to_bits(),
                        (*radius as f32).to_bits()
                    ],
                    *expected,
                    "native AddPoint center and radius"
                );
            }
            if !rainbow_flag && case["alpha_control_case"] == "alpha128" {
                compact_geometry_cases.push(serde_json::json!({
                    "q": scale,
                    "xy_radius_f32_bits": native_geometry.iter().map(|row|
                        row.map(|word| format!("{word:08x}"))
                    ).collect::<Vec<_>>(),
                }));
            }

            let transform = sweep_svg_transform(case);
            let canonical = if rainbow_flag {
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
            let canonical_svg = sweep_svg_from_plan(&canonical, transform, false);
            let production_svg = sweep_svg_from_plan(&scaled, transform, false);
            let quad_svg = sweep_svg_from_plan(&scaled, transform, true);
            let name = case["case"].as_str().unwrap();
            let artifact_prefix = format!("{prefix}-{name}");
            for (suffix, svg) in [
                ("canonical", &canonical_svg),
                ("production", &production_svg),
                ("test-native-quad", &quad_svg),
            ] {
                write_new(
                    output_dir.join(format!("{artifact_prefix}.{suffix}.svg")),
                    svg.as_bytes(),
                );
            }
            let native_artifact = capture["pixel_artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|artifact| artifact["key"].as_str() == Some(name))
                .expect("matching captured native framebuffer artifact");
            let native_path = repo.join(native_artifact["path"].as_str().unwrap());
            let compressed = fs::read(&native_path).expect("captured native RGBA frame");
            assert_eq!(sha256(&compressed), native_artifact["compressed_sha256"]);
            let mut raw = Vec::new();
            ZlibDecoder::new(compressed.as_slice())
                .read_to_end(&mut raw)
                .expect("decode captured native RGBA");
            assert_eq!(sha256(&raw), native_artifact["raw_sha256"]);
            let native_topdown = topdown_frame(&raw);
            let canonical_pixels = raster_svg(&canonical_svg);
            let production_pixels = raster_svg(&production_svg);
            let quad_pixels = raster_svg(&quad_svg);
            let canonical_metrics = alpha_support_metrics(&native_topdown, &canonical_pixels);
            let production_metrics = alpha_support_metrics(&native_topdown, &production_pixels);
            if plan.native_quad_geometry {
                assert_eq!(
                    production_svg, quad_svg,
                    "production gate admits native quads"
                );
            } else {
                assert_ne!(
                    production_svg, quad_svg,
                    "test control forces the quad variant"
                );
                assert!(
                    production_metrics["alpha_absolute_error_sum"]
                        .as_u64()
                        .unwrap()
                        < canonical_metrics["alpha_absolute_error_sum"]
                            .as_u64()
                            .unwrap(),
                    "explicit profile must improve native alpha error: {prefix}-{name}"
                );
            }
            assert_eq!(production_metrics["support_iou"], 1.0);
            if scale == 1.0 {
                assert_eq!(canonical_svg, production_svg, "unit-scale SVG is unchanged");
                assert_eq!(canonical.points, scaled.points, "q=1 sampled centers");
                assert_eq!(canonical.dot_radii, scaled.dot_radii, "q=1 radii");
                assert_eq!(canonical.sample_ends, scaled.sample_ends, "q=1 boundaries");
            }
            artifact_cases.push(serde_json::json!({
                "case": name,
                "capture_variant": prefix,
                "rainbow_flag": rainbow_flag,
                "q": scale,
                "saved_argb": format!("0x{argb:08x}"),
                "page_px": [WIDTH, HEIGHT],
                "native_gl_projection_uniform_to_svg_transform": transform,
                "native_frame": {
                    "path": native_path,
                    "compressed_sha256": native_artifact["compressed_sha256"],
                    "raw_sha256": native_artifact["raw_sha256"],
                    "raw_bytes": native_artifact["raw_bytes"],
                    "format": native_artifact["format"],
                },
                "canonical_svg": {
                    "path": format!("{artifact_prefix}.canonical.svg"),
                    "sha256": sha256(canonical_svg.as_bytes()),
                    "against_native": canonical_metrics,
                },
                "profile_production_svg": {
                    "path": format!("{artifact_prefix}.production.svg"),
                    "sha256": sha256(production_svg.as_bytes()),
                    "uses_native_quad_by_production_gate": plan.native_quad_geometry,
                    "against_native": production_metrics,
                },
                "test_native_quad_svg": {
                    "path": format!("{artifact_prefix}.test-native-quad.svg"),
                    "sha256": sha256(quad_svg.as_bytes()),
                    "forced_only_in_test": true,
                    "against_native": alpha_support_metrics(&native_topdown, &quad_pixels),
                },
                "record_count": plan.stamps.len(),
                "all_seven_upload_lanes_exact": true,
            }));
        }
    }

    let compact_geometry = serde_json::json!({
        "schema": "sdocx.native-v16-uniform-scale-geometry.v1",
        "scope": "Native GLV16 AddPoint center/radius inputs for the controlled size-1, rainbow-off 204-point source; uniform scale only, no device or PDF parity claim.",
        "capture_sha256": SCALE_SWEEP_SHA256,
        "validation_sha256": SCALE_SWEEP_VALIDATION_SHA256,
        "stroke": source_stroke_argb_width(&capture, false, 0x80000000, 1.0),
        "cases": compact_geometry_cases,
    });
    let mut compact_bytes = serde_json::to_vec(&compact_geometry).unwrap();
    compact_bytes.push(b'\n');
    write_new(output_dir.join("native-geometry-ci.json"), &compact_bytes);
    write_new(
        output_dir.join("manifest.json"),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "sdocx.fountain-v16-scale-native-svg-candidates.v2",
            "scope": "Production and test-only V16 SVG candidates from six rainbow-off and six rainbow-flag-on native q=0.5/1/2 captures; the test quad variant does not change production admission.",
            "rainbow_off_capture_sha256": SCALE_SWEEP_SHA256,
            "rainbow_off_validation_sha256": SCALE_SWEEP_VALIDATION_SHA256,
            "rainbow_on_capture_sha256": SCALE_SWEEP_RAINBOW_SHA256,
            "rainbow_on_validation_sha256": SCALE_SWEEP_RAINBOW_VALIDATION_SHA256,
            "native_projection_transform": "captured GLV16 projection uniform and viewport with one readback-y flip",
            "records": artifact_cases,
        }))
        .unwrap(),
    );
}
