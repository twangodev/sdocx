use sdocx::{
    InkRasterCanvas, InkRasterContext, InkRasterInverseScale, InkRasterOrigin,
    Pencil2V14HostGrainState, Stroke,
};
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("conformance/fountain-v14-saved-scene-inputs.json")).unwrap(),
    )
    .unwrap();
    let mut stroke: Stroke = serde_json::from_value(data["strokes"][1]["stroke"].clone()).unwrap();
    for point in &mut stroke.points {
        point.x += 3.25;
        point.y -= 2.5;
    }
    for pressure in &mut stroke.pressures {
        *pressure *= 0.75;
    }
    stroke.pen_width = f32::from_bits(0x4111de6a) as _;
    stroke.rendering.as_mut().unwrap().style.color_argb = Some(4294425088);
    stroke.rendering.as_mut().unwrap().pen_name =
        Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into());
    if std::env::args().nth(1).as_deref() != Some("original") {
        let source_rendering = stroke.rendering.as_ref().unwrap();
        let mut style = serde_json::to_value(sdocx::StrokeStyle::default()).unwrap();
        style["pen_size"] = serde_json::json!(stroke.pen_width);
        style["color_argb"] = serde_json::json!(source_rendering.style.color_argb);
        style["size_level"] = serde_json::json!(0);
        style["stroke_type_raw"] = serde_json::json!(0);
        style["color_type_raw"] = serde_json::json!(0);
        let props = serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
        stroke.rendering = Some(serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap());
    }
    let prior = std::fs::read(std::env::var("SDOCX_PENCIL2_PRIOR").unwrap()).unwrap();
    let pixels = prior
        .chunks_exact(192 * 4)
        .rev()
        .flat_map(|row| row.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]))
        .collect();
    let mut canvas = InkRasterCanvas::from_pixels(
        InkRasterContext {
            width: 192,
            height: 192,
            pixels_per_unit: 1,
            origin: InkRasterOrigin::BottomLeft,
        },
        pixels,
    )
    .unwrap();
    let projection = [
        0x3c2aaaab, 0, 0, 0, 0, 0x3c2aaaab, 0, 0, 0, 0, 0xb8000100, 0, 0xbf800000, 0xbf800000,
        0x80000000, 0x3f800000,
    ]
    .map(f32::from_bits);
    let mut matrix = [0.; 16];
    for i in 0..4 {
        matrix[5 * i] = 1.;
    }
    matrix[12] = 12.;
    matrix[13] = 12.;
    let mut grain = Pencil2V14HostGrainState::default();
    grain.set_start_point([stroke.points[0].x as f32, stroke.points[0].y as f32]);
    let offset = grain.draw(projection, matrix, [192, 192]).unwrap();
    println!(
        "computed offset bits: {:08x} {:08x}",
        offset[0].to_bits(),
        offset[1].to_bits()
    );
    match canvas.paint_pencil2_with_grain_offset(
        &stroke,
        InkRasterInverseScale::One,
        [12., 12.],
        offset,
    ) {
        Ok(()) => {
            let bytes: Vec<u8> = canvas
                .pixels()
                .iter()
                .flat_map(|p| p.iter().copied())
                .collect();
            std::fs::write(std::env::var("SDOCX_PENCIL2_OUTPUT").unwrap(), bytes).unwrap();
            if let Ok(directory) = std::env::var("SDOCX_PENCIL2_EXPORT_DIR") {
                let directory = Path::new(&directory);
                std::fs::write(directory.join("canvas.png"), canvas.to_png().unwrap()).unwrap();
                std::fs::write(
                    directory.join("canvas.svg"),
                    canvas.rendered_page(0).unwrap().svg,
                )
                .unwrap();
                #[cfg(feature = "export-pdf")]
                {
                    let mut options = sdocx::PdfOptions::default();
                    options.dpi = 72.;
                    std::fs::write(
                        directory.join("canvas.pdf"),
                        canvas.to_pdf(&options).unwrap(),
                    )
                    .unwrap();
                }
            }
            println!("paint accepted");
        }
        Err(e) => {
            eprintln!("paint rejected: {e:?}");
            std::process::exit(2);
        }
    }
}
