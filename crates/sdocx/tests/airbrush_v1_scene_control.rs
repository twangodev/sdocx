#![cfg(feature = "pdf")]

use base64::Engine;
use sha2::{Digest, Sha256};
use sdocx::{
    AirBrushRenderProfile, PdfOptions, Point, RenderOptions, parse_bytes, render_document_pdf,
    render_document_svg,
};
use std::io::Read;

#[test]
fn two_ordered_objects_against_native_host_control() {
    let native: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/airbrush-v1-scene/native-opaque.json"
    ))
    .unwrap();
    let mut doc = parse_bytes(include_bytes!("assets/chinesebrush/synthetic_v3.sdocx")).unwrap();
    let mut first = doc.pages[0].strokes().next().unwrap().clone();
    first.points = [
        (120., 120.),
        (120.125, 120.25),
        (122., 121.),
        (124., 118.),
        (124.0625, 118.),
        (126., 123.),
    ]
    .map(|(x, y)| Point { x, y })
    .into();
    first.pressures = vec![0., 0.1, 0.9, 0.3, 1., 0.5];
    first.tilts = vec![0., 0.1, 0.5, 0.2, 0.7, 0.3];
    first.orientations = vec![0., 0.2, 0.9, 0.4, 1.2, 0.6];
    first.timestamps = vec![300, 301, 310, 450, 451, 900];
    first.pen_width = 8.75;
    first.color = None;
    let r = first.rendering.as_mut().unwrap();
    r.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.AirBrushPen".into());
    r.advanced_settings = None;
    r.tool_type_raw = 2;
    r.properties.compressed = true;
    r.properties.replay_only = false;
    r.properties.eraser = false;
    r.properties.fixed_width = false;
    r.properties.top_layer_pen = false;
    r.properties.alpha_lock = false;
    r.properties.fixed_opacity = false;
    r.properties.rainbow_effect = false;
    r.properties.straighten = false;
    r.properties.reveal_mode = false;
    r.style.pen_size = Some(8.75);
    r.style.color_argb = Some(0x8044ee11);
    r.style.stroke_type_raw = Some(0);
    r.style.color_type_raw = Some(0);
    let mut second = first.clone();
    for p in &mut second.points {
        p.x += 2.;
        p.y += 1.;
    }
    second.rendering.as_mut().unwrap().style.color_argb = Some(0xffd93672);
    doc.pages[0].width = 256;
    doc.pages[0].height = 256;
    doc.pages[0].background_color = None;
    doc.metadata.dark_mode_compatibility = Some(false);
    doc.pages[0].objects = vec![first.into(), second.into()];
    let mut options = RenderOptions::default();
    options.airbrush_profile = Some(AirBrushRenderProfile::SavedV1);
    if let Some(path) = std::env::var_os("SDOCX_AIRBRUSH_SCENE_PDF") {
        let pdf = render_document_pdf(&doc, &options, &PdfOptions::default()).unwrap();
        std::fs::write(path, pdf).unwrap();
    }
    let svg = &render_document_svg(&doc, &options)[0].svg;
    assert!(svg.contains("sdocx-airbrush-v2-r8-v3-"));
    let tree = resvg::usvg::Tree::from_str(svg, &Default::default()).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let compressed = base64::engine::general_purpose::STANDARD
        .decode(native["second"]["data_base64"].as_str().unwrap())
        .unwrap();
    let mut bottom = Vec::new();
    flate2::read::ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut bottom)
        .unwrap();
    assert_eq!(bottom.len(), 256 * 256 * 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bottom)),
        "349699e5ae29169749bead92a1fd35ba4c34466a86b4fb89e85a83eea0bb71a8"
    );
    let expected: Vec<u8> = bottom.chunks_exact(1024).rev().flatten().copied().collect();
    let error: u64 = pixmap
        .data()
        .iter()
        .zip(&expected)
        .map(|(&a, &b)| u64::from(a.abs_diff(b)))
        .sum();
    assert_eq!(error, 7, "source-generated SVG raster versus native RGBA");
}
