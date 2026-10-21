use std::io::Cursor;

use sha2::Digest;

use crate::{
    ChineseBrushFwidthOutputProfile, ChineseBrushRenderProfile, RenderOptions, render_document_svg,
};

const OUTPUT_WIDTH: u32 = 321;
const OUTPUT_HEIGHT: u32 = 228;
const SOURCE_PAGE_WIDTH: u32 = 1080;
const SOURCE_PAGE_HEIGHT: u32 = 1527;
const ROI: [u32; 4] = [108, 0, 140, 32];

#[derive(Debug)]
struct AlphaMetrics {
    support_xor: usize,
    alpha_mae_union: f64,
    alpha_energy_delta: i64,
}

fn native_alpha_roi() -> Vec<u8> {
    // Captured from an authored synthetic ObjectStroke through the pinned
    // ChineseBrush/RTV1 Mesa draw, not from SDK output. The capture used the
    // same packed 528-byte VBO as native_packed.bin (SHA-256
    // 44b6c25b973b37225e4258f7b44ad7dc44ff4e87dd13bb01ccf07b7f9891f49a),
    // intrinsic 144x144 atlas SHA-256
    // da75f5d1190e6fe086aa11007b7ed739421cf5d4522e34cb246df273353d2942,
    // explicit synthetic source profile 1920x1080, and the captured identity
    // SetMatrix projection: x' = x, y' = 228 - y. Native readback is
    // bottom-left-origin; this 32x32 alpha-only PNG is top-left ordered and
    // includes the real framebuffer's top-edge clipping of the first dab.
    // Full framebuffer SHA-256: 134585a69be07760817b3becc526dfdbb11c4fa930f5fb7fb1dd12dd7fe81ca3.
    let decoder = png::Decoder::new(Cursor::new(include_bytes!(
        "../../tests/assets/chinesebrush/native_identity_alpha_roi.png"
    )));
    let mut reader = decoder.read_info().unwrap();
    assert_eq!(
        (reader.info().width, reader.info().height),
        (ROI[2] - ROI[0], ROI[3] - ROI[1])
    );
    assert_eq!(reader.info().color_type, png::ColorType::Grayscale);
    assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
    let mut alpha = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut alpha).unwrap();
    alpha.truncate(info.buffer_size());
    alpha
}

fn raster_profile(document: &crate::Document, fwidth: bool) -> Vec<u8> {
    let options = RenderOptions {
        chinesebrush_profile: Some(ChineseBrushRenderProfile {
            screen_width: 1920,
            screen_height: 1080,
        }),
        chinesebrush_fwidth_output_profile: fwidth.then_some(ChineseBrushFwidthOutputProfile {
            pixels_per_page_unit: 1.0,
        }),
        ..RenderOptions::default()
    };
    let rendered = render_document_svg(document, &options)
        .into_iter()
        .next()
        .expect("fixture has one visible page");
    assert_eq!(
        (rendered.width, rendered.height),
        (SOURCE_PAGE_WIDTH, SOURCE_PAGE_HEIGHT)
    );
    let svg = strip_page_canvas(&rendered.svg, rendered.width, rendered.height);
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
        .expect("rendered source SVG parses");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(OUTPUT_WIDTH, OUTPUT_HEIGHT).unwrap();
    // The captured native identity projection maps source-device units to the
    // framebuffer at 1:1 and flips Y around the 228px output viewport. Its
    // native SetMatrix acts after the ChineseBrush model; preserve the public
    // 1080x1527 page and 1920x1080 sampler profile, and let the target pixmap
    // apply the captured full-frame clip.
    let output =
        resvg::tiny_skia::Transform::from_row(1.0, 0.0, 0.0, -1.0, 0.0, OUTPUT_HEIGHT as f32);
    resvg::render(&tree, output, &mut pixmap.as_mut());
    let mut roi = Vec::with_capacity(((ROI[2] - ROI[0]) * (ROI[3] - ROI[1])) as usize);
    for y in ROI[1]..ROI[3] {
        for x in ROI[0]..ROI[2] {
            let offset = ((y * OUTPUT_WIDTH + x) * 4 + 3) as usize;
            roi.push(pixmap.data()[offset]);
        }
    }
    roi
}

fn strip_page_canvas(svg: &str, width: u32, height: u32) -> String {
    let document = roxmltree::Document::parse(svg).expect("rendered SVG is well-formed");
    let root = document.root_element();
    assert_eq!(root.tag_name().name(), "svg");
    let view_box = root
        .attribute("viewBox")
        .expect("rendered SVG has an explicit viewBox")
        .split_ascii_whitespace()
        .map(|value| value.parse::<f64>().expect("numeric SVG viewBox"))
        .collect::<Vec<_>>();
    assert_eq!(view_box.len(), 4);
    assert_eq!(view_box, [0.0, 0.0, f64::from(width), f64::from(height)]);

    // The public renderer puts its page-color rectangle first in the scene.
    // Remove only that exact full-page canvas; all authored source objects and
    // the ChineseBrush stroke remain in normal SVG paint order.
    let canvases = root
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "rect")
        .filter(|node| {
            ["x", "y"].into_iter().all(|name| {
                node.attribute(name)
                    .and_then(|value| value.parse::<f64>().ok())
                    == Some(0.0)
            }) && node
                .attribute("width")
                .and_then(|value| value.parse::<f64>().ok())
                == Some(f64::from(width))
                && node
                    .attribute("height")
                    .and_then(|value| value.parse::<f64>().ok())
                    == Some(f64::from(height))
        })
        .collect::<Vec<_>>();
    assert_eq!(canvases.len(), 1, "expected one exact page-color canvas");
    let range = canvases[0].range();
    let mut ink_only = String::with_capacity(svg.len() - range.len());
    ink_only.push_str(&svg[..range.start]);
    ink_only.push_str(&svg[range.end..]);
    ink_only
}

fn compare_alpha(reference: &[u8], candidate: &[u8]) -> AlphaMetrics {
    assert_eq!(reference.len(), candidate.len());
    let support_xor = reference
        .iter()
        .zip(candidate)
        .filter(|(reference, candidate)| (**reference > 0) != (**candidate > 0))
        .count();
    let union_count = reference
        .iter()
        .zip(candidate)
        .filter(|(reference, candidate)| **reference > 0 || **candidate > 0)
        .count();
    let alpha_error = reference
        .iter()
        .zip(candidate)
        .map(|(reference, candidate)| i64::from(*candidate) - i64::from(*reference))
        .collect::<Vec<_>>();
    let alpha_mae_union = reference
        .iter()
        .zip(candidate)
        .filter(|(reference, candidate)| **reference > 0 || **candidate > 0)
        .map(|(reference, candidate)| (i16::from(*candidate) - i16::from(*reference)).abs())
        .map(f64::from)
        .sum::<f64>()
        / union_count.max(1) as f64;
    AlphaMetrics {
        support_xor,
        alpha_mae_union,
        alpha_energy_delta: alpha_error.iter().sum(),
    }
}

#[test]
fn captured_native_fringe_is_better_matched_by_explicit_fwidth_than_hard_circle() {
    let archive = include_bytes!("../../tests/assets/chinesebrush/synthetic_v3.sdocx");
    let packed = include_bytes!("../../tests/assets/chinesebrush/native_packed.bin");
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(archive)),
        "bdefbc3d23005761dc912f65cc0309d33e07ed6769b9c984c76699312fa5a729"
    );
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(packed)),
        "44b6c25b973b37225e4258f7b44ad7dc44ff4e87dd13bb01ccf07b7f9891f49a"
    );
    let document = crate::parse_bytes(archive).expect("pinned synthetic source archive");
    let reference = native_alpha_roi();
    let hard_circle = compare_alpha(&reference, &raster_profile(&document, false));
    let explicit_fwidth = compare_alpha(&reference, &raster_profile(&document, true));

    // The captured native output contains a clipped dab at the ROI's top edge.
    // These broad bounds protect the recovered finite-fringe behavior without
    // pretending SVG mask rasterization is pixel-identical to GLES fwidth.
    assert!(hard_circle.support_xor >= 20, "{hard_circle:?}");
    assert!(explicit_fwidth.support_xor <= 2, "{explicit_fwidth:?}");
    assert!(
        explicit_fwidth.alpha_mae_union < hard_circle.alpha_mae_union * 0.6,
        "hard-circle={hard_circle:?}; explicit-fwidth={explicit_fwidth:?}"
    );
    assert!(
        explicit_fwidth.alpha_energy_delta.abs() < hard_circle.alpha_energy_delta.abs(),
        "hard-circle={hard_circle:?}; explicit-fwidth={explicit_fwidth:?}"
    );
}
