#![cfg(all(feature = "render", feature = "pdf"))]

use sdocx::{
    AirBrushRenderProfile, PdfOptions, Point, RenderOptions, layout_document, parse_bytes,
    render_document_pdf, render_document_svg, render_layout_page_replay_svg,
};

#[test]
fn tool0_literal_v2_matches_empty_exports_and_preserves_fallbacks() {
    for (shape, size, color) in [
        (0, 3.144, 0xff44aaee),
        (1, 8.75, 0x8044ee11),
        (2, 2.25, 0xffdf2255),
    ] {
        let mut document =
            parse_bytes(include_bytes!("assets/chinesebrush/synthetic_v3.sdocx")).unwrap();
        let mut stroke = document.pages[0].strokes().next().unwrap().clone();
        stroke.points = (0..5)
            .map(|i| Point {
                x: 128.453125
                    + match shape {
                        0 => i as f64 * 3.0,
                        1 => i as f64 * 4.0,
                        _ => -(i as f64) * 2.5,
                    },
                y: 128.515625
                    + match shape {
                        0 => (i * i) as f64 * 0.5,
                        1 => (i % 2) as f64 * 7.0,
                        _ => -(i as f64) * 1.25,
                    },
            })
            .collect();
        stroke.pressures = vec![0.3; 5];
        stroke.tilts = vec![0.12; 5];
        stroke.orientations = vec![0.2; 5];
        stroke.timestamps = vec![300, 315, 330, 345, 360];
        stroke.color = None;
        stroke.pen_width = size;
        let r = stroke.rendering.as_mut().unwrap();
        r.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.AirBrushPen".into());
        r.advanced_settings = Some(String::new());
        r.tool_type_raw = 0;
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
        r.style.pen_size = Some(size);
        r.style.color_argb = Some(color);
        r.style.stroke_type_raw = Some(0);
        r.style.color_type_raw = Some(0);
        let page = &mut document.pages[0];
        page.width = 256;
        page.height = 256;
        page.background_color = None;
        page.objects = vec![stroke.into()];
        document.metadata.dark_mode_compatibility = Some(false);
        let mut literal = document.clone();
        literal.pages[0]
            .strokes_mut()
            .next()
            .unwrap()
            .rendering
            .as_mut()
            .unwrap()
            .advanced_settings = Some("2;".into());
        let mut options = RenderOptions::default();
        options.airbrush_profile = Some(AirBrushRenderProfile::SavedV2);
        let svg = |doc: &sdocx::Document, opts: &RenderOptions| {
            render_document_svg(doc, opts)[0].svg.clone()
        };
        assert_eq!(svg(&document, &options), svg(&literal, &options));
        assert_ne!(
            svg(&literal, &options),
            svg(&literal, &RenderOptions::default())
        );
        assert_eq!(
            render_document_pdf(&document, &options, &PdfOptions::default()).unwrap(),
            render_document_pdf(&literal, &options, &PdfOptions::default()).unwrap()
        );
        let replay = |doc: &sdocx::Document| {
            let layout = layout_document(doc);
            render_layout_page_replay_svg(doc, &layout, 0, &options)
                .unwrap()
                .svg
        };
        assert_eq!(replay(&literal), replay(&document));
        let mut unsupported = literal.clone();
        unsupported.pages[0]
            .strokes_mut()
            .next()
            .unwrap()
            .rendering
            .as_mut()
            .unwrap()
            .advanced_settings = Some("3;".into());
        assert_eq!(
            svg(&unsupported, &options),
            svg(&unsupported, &RenderOptions::default())
        );
    }
}
