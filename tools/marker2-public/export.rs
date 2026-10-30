use sdocx::*;
use serde_json::{Value, json};
use std::{fs, path::Path};

fn f32_bits(value: &Value) -> f32 {
    let text = value.as_str().unwrap();
    f32::from_le_bytes(std::array::from_fn(|i| {
        u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap()
    }))
}

fn main() {
    let directory = std::env::args().nth(1).expect("output directory");
    let out = Path::new(&directory);
    fs::create_dir_all(out).unwrap();
    let fixture: Value =
        serde_json::from_slice(&fs::read(out.join("input.json")).unwrap()).unwrap();
    let mut options = RenderOptions::default();
    options.marker2_output_scale =
        (std::env::var("MARKER2_CANDIDATE").as_deref() == Ok("2")).then_some(2);
    let fonts = fonts::FontBook::default();
    let pdf_options = PdfOptions::from_font_book(&fonts);
    let mut report = Vec::new();
    let mut document = Document {
        pages: vec![Page {
            uuid: "marker2-native-control".into(),
            width: 256,
            height: 256,
            content_bbox: BoundingBox {
                x_min: 0.,
                y_min: 0.,
                x_max: 256.,
                y_max: 256.,
            },
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![],
        }],
        metadata: DocumentMetadata {
            page_dimensions: Some((256, 256)),
            ..Default::default()
        },
    };
    for (index, (name, case)) in std::iter::once(("baseline", &Value::Null))
        .chain(
            fixture["cases"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.as_str(), v)),
        )
        .enumerate()
    {
        document.pages[0].objects.clear();
        if !case.is_null() {
            let input = &case["input"];
            let points: Vec<Point> = input["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Point {
                    x: f32_bits(&p[0]) as f64,
                    y: f32_bits(&p[1]) as f64,
                })
                .collect();
            let size = f32_bits(&input["pen_size_f32_bits"]);
            let omitted = input["size_input"] == "omitted";
            let argb = u32::from_str_radix(
                input["color_argb"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            let rendering: StrokeRendering = serde_json::from_value(json!({
                "metadata":null, "pen_name":input["pen_name"], "advanced_settings":input["advanced_setting"], "tool_type_raw":input["tool_type"],
                "properties": { "compressed":input["curve_enabled"], "replay_only":false, "stylus_channels":false, "eraser":false, "fixed_width":false, "millisecond_timestamps":false, "top_layer_pen":false, "alpha_lock":false, "binary_added":true, "generated":false, "fixed_opacity":false, "rainbow_effect":false, "straighten":false, "reveal_mode":false },
                "style": { "pen_size": if omitted { None } else { Some(size) }, "color_argb":argb, "stroke_type_raw":input["stroke_type"], "trailing_data":[] }
            })).unwrap();
            let channel = |key: &str| {
                input[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| f32_bits(v) as f64)
                    .collect()
            };
            let stroke = Stroke {
                rendering: Some(rendering),
                bbox: BoundingBox {
                    x_min: 10.,
                    y_min: 20.,
                    x_max: 34.,
                    y_max: 25.,
                },
                points,
                pressures: channel("pressure_f32_bits"),
                timestamps: input["timestamps_i32"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap())
                    .collect(),
                tilts: channel("tilt_f32_bits"),
                orientations: channel("orientation_f32_bits"),
                color: Some(Color {
                    r: (argb >> 16) as u8,
                    g: (argb >> 8) as u8,
                    b: argb as u8,
                }),
                pen_width: if omitted { 0.8 } else { size },
            };
            let ink = prepare_stroke(&stroke, false);
            report.push(json!({"index":index,"case":name,"support":format!("{:?}",ink.support),"points":ink.points,"radii":ink.dot_radii,"sample_ends":ink.sample_ends,"color":ink.color,"opacity":ink.opacity}));
            let mut second = stroke.clone();
            for p in &mut second.points {
                p.x += 2.0;
                p.y -= 1.5;
            }
            document.pages[0].objects.push(PageObject::from(stroke));
            document.pages[0].objects.push(PageObject::from(second));
        }
        let page = render_page_svg(&document, 0, &options).unwrap();
        fs::write(out.join(format!("case-{index}.svg")), &page.svg).unwrap();
        fs::write(
            out.join(format!("case-{index}-svg.pdf")),
            render_svg_pages_pdf(&[page], &pdf_options).unwrap(),
        )
        .unwrap();
        fs::write(
            out.join(format!("case-{index}-direct.pdf")),
            render_document_pdf(&document, &options, &pdf_options).unwrap(),
        )
        .unwrap();
    }
    fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
