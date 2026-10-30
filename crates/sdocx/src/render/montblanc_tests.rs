use super::{
    MontblancRenderProfile, RenderOptions, render_document_svg, render_layout_page_replay_svg,
};
use crate::{
    BoundingBox, Color, Document, Page, Point, Stroke, StrokeProperties, StrokeRendering,
    StrokeStyle,
};

fn source(points: &[[f32; 2]], pressure: &[f32]) -> Stroke {
    Stroke {
        rendering: Some(StrokeRendering {
            metadata: None,
            pen_name: Some("com.samsung.android.sdk.pen.pen.preload.MontblancFountainPen".into()),
            advanced_settings: Some("1;".into()),
            tool_type_raw: 2,
            properties: StrokeProperties {
                compressed: true,
                binary_added: true,
                replay_only: false,
                stylus_channels: false,
                eraser: false,
                fixed_width: false,
                millisecond_timestamps: false,
                top_layer_pen: false,
                alpha_lock: false,
                generated: false,
                fixed_opacity: false,
                rainbow_effect: false,
                straighten: false,
                reveal_mode: false,
            },
            style: StrokeStyle {
                color_argb: Some(0xff336699),
                pen_size: Some(2.0),
                ..Default::default()
            },
        }),
        bbox: BoundingBox::default(),
        points: points
            .iter()
            .map(|p| Point {
                x: f64::from(128.0_f32 + p[0]),
                y: f64::from(128.0_f32 - p[1]),
            })
            .collect(),
        pressures: pressure.iter().map(|&p| f64::from(p)).collect(),
        timestamps: vec![],
        tilts: vec![],
        orientations: vec![],
        color: Some(Color {
            r: 51,
            g: 102,
            b: 153,
        }),
        pen_width: 2.0,
    }
}
fn document(s: Stroke) -> Document {
    Document {
        pages: vec![Page {
            uuid: "montblanc".into(),
            width: 256,
            height: 256,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![s.into()],
        }],
        metadata: Default::default(),
    }
}
fn options() -> RenderOptions {
    RenderOptions {
        montblanc_profile: Some(MontblancRenderProfile::HostUnitScale),
        ..Default::default()
    }
}
#[test]
fn public_dispatch_and_replay_fallback() {
    for (points, pressures) in [
        (&[[0., 0.], [10., 0.], [20., 2.]][..], &[0.3, 0.6, 1.][..]),
        (
            &[[1., 3.], [10., 10.], [20., 2.], [30., 0.]][..],
            &[0.3, 0.6, 1., 0.7][..],
        ),
        (
            &[[0., 0.], [5., 8.], [11., -1.], [15., 6.], [22., -3.]][..],
            &[0.3, 0.6, 1., 0.7, 0.5][..],
        ),
    ] {
        let doc = document(source(points, pressures));
        assert_ne!(
            render_document_svg(&doc, &options())[0].svg,
            render_document_svg(&doc, &RenderOptions::default())[0].svg
        );
        let layout = crate::layout_document(&doc);
        assert_eq!(
            render_layout_page_replay_svg(&doc, &layout, 0, &options())
                .unwrap()
                .svg,
            render_layout_page_replay_svg(&doc, &layout, 0, &RenderOptions::default())
                .unwrap()
                .svg
        );
    }
}
#[test]
fn unsupported_inputs_keep_generic_output() {
    let base = source(&[[0., 0.], [10., 0.], [20., 2.]], &[0.3, 0.6, 1.]);
    let mut cases = Vec::new();
    let mut s = base.clone();
    s.pen_width = 3.;
    s.rendering.as_mut().unwrap().style.pen_size = Some(3.);
    cases.push(s);
    let mut s = base.clone();
    s.rendering.as_mut().unwrap().properties.stylus_channels = true;
    cases.push(s);
    let mut s = base.clone();
    s.tilts.push(0.);
    cases.push(s);
    let mut s = base.clone();
    s.points.push(Point { x: 160., y: 130. });
    s.pressures.push(0.5);
    s.points.push(Point { x: 170., y: 130. });
    s.pressures.push(0.5);
    s.points.push(Point { x: 180., y: 130. });
    s.pressures.push(0.5);
    s.points.push(Point { x: 190., y: 130. });
    s.pressures.push(0.5);
    s.points.push(Point { x: 200., y: 130. });
    s.pressures.push(0.5);
    s.points.push(Point { x: 210., y: 130. });
    s.pressures.push(0.5);
    s.points.push(Point { x: 220., y: 130. });
    s.pressures.push(0.5);
    s.points.push(Point { x: 230., y: 130. });
    s.pressures.push(0.5);
    s.points.push(Point { x: 240., y: 130. });
    s.pressures.push(0.5);
    for i in 0..9 {
        s.points.push(Point { x: 250. + i as f64 * 10., y: 130. });
        s.pressures.push(0.5);
    }
    cases.push(s);
    let mut s = base.clone();
    s.points[1].x = 1_000_001.;
    cases.push(s);
    let mut s = base;
    s.pressures[1] = 2.;
    cases.push(s);
    for stroke in cases {
        let doc = document(stroke);
        assert_eq!(
            render_document_svg(&doc, &options())[0].svg,
            render_document_svg(&doc, &RenderOptions::default())[0].svg
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn public_pdf_uses_opt_in_and_retains_generic_fallback() {
    let stroke = source(&[[0., 0.], [10., 0.], [20., 2.]], &[0.3, 0.6, 1.]);
    let doc = document(stroke.clone());
    let candidate =
        crate::render_document_pdf(&doc, &options(), &crate::PdfOptions::default()).unwrap();
    let fallback = crate::render_document_pdf(
        &doc,
        &RenderOptions::default(),
        &crate::PdfOptions::default(),
    )
    .unwrap();
    assert_ne!(candidate, fallback);
    assert_eq!(
        lopdf::Document::load_mem(&candidate)
            .unwrap()
            .get_pages()
            .len(),
        1
    );
    let mut unsupported = stroke;
    unsupported
        .rendering
        .as_mut()
        .unwrap()
        .properties
        .fixed_width = true;
    let doc = document(unsupported);
    let candidate =
        crate::render_document_pdf(&doc, &options(), &crate::PdfOptions::default()).unwrap();
    let fallback = crate::render_document_pdf(
        &doc,
        &RenderOptions::default(),
        &crate::PdfOptions::default(),
    )
    .unwrap();
    assert_eq!(candidate, fallback);
}

#[test]
fn move_admission_boundary_falls_back() {
    for x in [4.0_f32, f32::from_bits(133.0_f32.to_bits() - 1) - 128.0] {
        let s = source(&[[0., 0.], [x, 0.], [20., 2.]], &[0.3, 0.6, 1.]);
        assert!(crate::ink::montblanc::prepare(&s).is_none());
        let doc = document(s);
        assert_eq!(
            render_document_svg(&doc, &options())[0].svg,
            render_document_svg(&doc, &RenderOptions::default())[0].svg
        );
    }
    let s = source(&[[0., 0.], [5., 0.], [20., 2.]], &[0.3, 0.6, 1.]);
    assert!(crate::ink::montblanc::prepare(&s).is_some());
    let s = source(
        &[[0., 0.], [5., 0.], [9., 0.], [20., 2.]],
        &[0.3, 0.6, 0.7, 1.],
    );
    assert!(crate::ink::montblanc::prepare(&s).is_none());
    let s = source(
        &[[0., 0.], [5., 0.], [10., 0.], [20., 2.]],
        &[0.3, 0.6, 0.7, 1.],
    );
    assert!(crate::ink::montblanc::prepare(&s).is_some());
}
#[test]
fn page_budget_falls_back_and_preserves_stroke_order() {
    let mut first = source(&[[0., 0.], [10., 0.], [20., 2.]], &[0.3, 0.6, 1.]);
    first.color = Some(Color { r: 255, g: 0, b: 0 });
    let mut second = first.clone();
    second.color = Some(Color { r: 0, g: 0, b: 255 });
    let mut doc = document(first.clone());
    doc.pages[0].objects = vec![first.into(); 12];
    doc.pages[0].objects.push(second.into());
    let svg = &render_document_svg(&doc, &options())[0].svg;
    let red = svg.find("#ff0000").unwrap();
    let blue = svg.rfind("#0000ff").unwrap();
    assert!(red < blue);
    assert!(svg.matches("<path").count() < 12 * 5000);
    // After the page budget is exhausted, the blue stroke is generic.
    assert!(!svg.contains("fill=\"#0000ff\""));
    assert_eq!(svg.matches("stroke=\"#0000ff\"").count(), 2);
}

#[test]
fn two_pixel_profile_preserves_fallbacks() {
    let opts = RenderOptions {
        montblanc_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let stroke = source(&[[0., 0.], [10., 0.], [20., 2.]], &[0.3, 0.6, 1.]);
    let doc = document(stroke.clone());
    assert_ne!(
        render_document_svg(&doc, &opts)[0].svg,
        render_document_svg(&doc, &options())[0].svg
    );
    let layout = crate::layout_document(&doc);
    assert_eq!(
        render_layout_page_replay_svg(&doc, &layout, 0, &opts)
            .unwrap()
            .svg,
        render_layout_page_replay_svg(&doc, &layout, 0, &RenderOptions::default())
            .unwrap()
            .svg
    );
    let mut unsupported = stroke;
    unsupported
        .rendering
        .as_mut()
        .unwrap()
        .properties
        .fixed_width = true;
    let doc = document(unsupported);
    assert_eq!(
        render_document_svg(&doc, &opts)[0].svg,
        render_document_svg(&doc, &RenderOptions::default())[0].svg
    );
}

#[cfg(feature = "pdf")]
#[test]
fn two_pixel_profile_sliced_output_falls_back() {
    let doc = document(source(&[[0., 0.], [10., 0.], [20., 2.]], &[0.3, 0.6, 1.]));
    let layout = crate::layout_document(&doc);
    let render = |options: &RenderOptions| {
        let request = super::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
        super::DocumentTextCache::default()
            .render_layout_page_scene_requests(
                &doc,
                &[request],
                options,
                &super::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page
            .svg
    };
    let opts = RenderOptions {
        montblanc_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    assert_eq!(render(&opts), render(&RenderOptions::default()));
}

#[test]
fn two_pixel_profile_admits_bounded_four_and_five_point_strokes() {
    let opts = RenderOptions {
        montblanc_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    for (points, pressures) in [
        (
            &[[1., 3.], [10., 10.], [20., 2.], [30., 0.]][..],
            &[0.3, 0.6, 1., 0.7][..],
        ),
        (
            &[[0., 0.], [5., 8.], [11., -1.], [15., 6.], [22., -3.]][..],
            &[0.3, 0.6, 1., 0.7, 0.5][..],
        ),
    ] {
        let stroke = source(points, pressures);
        assert!(crate::ink::montblanc::prepare(&stroke).is_some());
        let doc = document(stroke.clone());
        let generic = render_document_svg(&doc, &RenderOptions::default())[0]
            .svg
            .clone();
        let aware = render_document_svg(&doc, &opts)[0].svg.clone();
        let unit = render_document_svg(&doc, &options())[0].svg.clone();
        assert_ne!(aware, generic);
        assert_ne!(aware, unit);
        assert!(aware.contains("fill=\"#336699\""));
        assert!(aware.matches("fill-opacity=").count() > 100);
        let layout = crate::layout_document(&doc);
        assert_eq!(
            render_layout_page_replay_svg(&doc, &layout, 0, &opts)
                .unwrap()
                .svg,
            render_layout_page_replay_svg(&doc, &layout, 0, &RenderOptions::default())
                .unwrap()
                .svg
        );
        let conflict = RenderOptions {
            montblanc_profile: Some(MontblancRenderProfile::HostUnitScale),
            ..opts.clone()
        };
        assert_eq!(render_document_svg(&doc, &conflict)[0].svg, generic);
        let mut unsupported = stroke;
        unsupported
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .fixed_width = true;
        assert!(crate::ink::montblanc::prepare(&unsupported).is_none());
        let unsupported_doc = document(unsupported);
        assert_eq!(
            render_document_svg(&unsupported_doc, &opts)[0].svg,
            render_document_svg(&unsupported_doc, &RenderOptions::default())[0].svg
        );
        #[cfg(feature = "pdf")]
        {
            let render_slice = |options: &RenderOptions| {
                let request =
                    super::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
                super::DocumentTextCache::default()
                    .render_layout_page_scene_requests(
                        &doc,
                        &[request],
                        options,
                        &super::fonts::FontBook::default(),
                        &mut |_| {},
                    )
                    .remove(0)
                    .page
                    .svg
            };
            assert_eq!(render_slice(&opts), render_slice(&RenderOptions::default()));
        }
    }
}

#[test]
fn conflicting_montblanc_assertions_fall_back() {
    let mut opts = options();
    opts.montblanc_two_pixels_per_page_unit = true;
    let doc = document(source(&[[0., 0.], [10., 0.], [20., 2.]], &[0.3, 0.6, 1.]));
    assert_eq!(
        render_document_svg(&doc, &opts)[0].svg,
        render_document_svg(&doc, &RenderOptions::default())[0].svg
    );
}

#[cfg(feature = "serde")]
#[test]
fn older_render_options_deserialize_with_two_pixel_profile_disabled() {
    let mut value = serde_json::to_value(RenderOptions::default()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("montblanc_two_pixels_per_page_unit");
    let options: RenderOptions = serde_json::from_value(value).unwrap();
    assert!(!options.montblanc_two_pixels_per_page_unit);
}

#[test]
fn nine_point_profiles_admit_bounded_sizes() {
    let points = [
        [1., 2.],
        [7., 9.],
        [13., 3.],
        [20., 10.],
        [27., 4.],
        [34., 11.],
        [41., 5.],
        [48., 12.],
        [55., 6.],
    ];
    let mut too_narrow = source(&points, &[0.5; 9]);
    too_narrow.pen_width = 0.09;
    too_narrow.rendering.as_mut().unwrap().style.pen_size = Some(0.09);
    assert!(crate::ink::montblanc::prepare(&too_narrow).is_none());
    for size in [0.1, 0.2, 0.4, 0.49, 0.5, 2.0] {
        let mut stroke = source(&points, &[0.5; 9]);
        stroke.pen_width = size;
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(size);
        assert!(crate::ink::montblanc::prepare(&stroke).is_some());
        let doc = document(stroke);
        let fallback = render_document_svg(&doc, &RenderOptions::default())[0]
            .svg
            .clone();
        for profile in [
            options(),
            RenderOptions {
                montblanc_two_pixels_per_page_unit: true,
                ..Default::default()
            },
        ] {
            assert_ne!(render_document_svg(&doc, &profile)[0].svg, fallback);
        }
    }
}

#[test]
fn long_profiles_are_limited_to_supported_sizes() {
    let points = [
        [0., 0.],
        [7., 3.],
        [14., -2.],
        [21., 4.],
        [28., -1.],
        [35., 3.],
        [42., -4.],
        [49., 2.],
        [56., -3.],
        [63., 4.],
        [70., -2.],
    ];
    for n in [10, 11] {
        let mut stroke = source(&points[..n], &vec![0.5; n]);
        for size in [0.49, 0.5, 1.0, 1.5, 2.0, 2.01] {
            stroke.pen_width = size;
            stroke.rendering.as_mut().unwrap().style.pen_size = Some(size);
            assert_eq!(
                crate::ink::montblanc::prepare(&stroke).is_some(),
                (0.5..=2.0).contains(&size)
            );
            let doc = document(stroke.clone());
            let generic = render_document_svg(&doc, &RenderOptions::default())[0]
                .svg
                .clone();
            assert_eq!(
                render_document_svg(&doc, &options())[0].svg != generic,
                (0.5..=2.0).contains(&size)
            );
        }
        stroke.pen_width = 1.5;
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(1.5);
        let doc = document(stroke);
        let generic = render_document_svg(&doc, &RenderOptions::default())[0]
            .svg
            .clone();
        assert_ne!(render_document_svg(&doc, &options())[0].svg, generic);
        let two = RenderOptions {
            montblanc_two_pixels_per_page_unit: true,
            ..Default::default()
        };
        assert_ne!(render_document_svg(&doc, &two)[0].svg, generic);
        let conflict = RenderOptions {
            montblanc_profile: Some(MontblancRenderProfile::HostUnitScale),
            montblanc_two_pixels_per_page_unit: true,
            ..Default::default()
        };
        assert_eq!(render_document_svg(&doc, &conflict)[0].svg, generic);
        for size in [0.49, 0.5, 1.0, 2.0, 2.01] {
            let mut rejected = source(&points[..n], &vec![0.5; n]);
            rejected.pen_width = size;
            rejected.rendering.as_mut().unwrap().style.pen_size = Some(size);
            let rejected = document(rejected);
            assert_eq!(
                render_document_svg(&rejected, &two)[0].svg
                    != render_document_svg(&rejected, &RenderOptions::default())[0].svg,
                (0.5..=2.0).contains(&size)
            );
        }
    }
}

#[test]
fn longer_profiles_have_size_and_scale_boundaries() {
    let points: Vec<[f32; 2]> = (0..21)
        .map(|i| [(i * 6) as f32, if i % 2 == 0 { 0. } else { 3. }])
        .collect();
    for n in [12, 20, 21] {
        for size in [0.49, 0.5, 1.0, 1.5, 2.0, 2.01] {
            let mut stroke = source(&points[..n], &vec![0.5; n]);
            stroke.pen_width = size;
            stroke.rendering.as_mut().unwrap().style.pen_size = Some(size);
            let doc = document(stroke);
            let generic = render_document_svg(&doc, &RenderOptions::default())[0]
                .svg
                .clone();
            assert_eq!(
                render_document_svg(&doc, &options())[0].svg != generic,
                n <= 20 && (0.5..=2.0).contains(&size)
            );
            let two = RenderOptions {
                montblanc_two_pixels_per_page_unit: true,
                ..Default::default()
            };
            assert_eq!(
                render_document_svg(&doc, &two)[0].svg != generic,
                n <= 20 && size == 1.5
            );
        }
    }
}

#[test]
fn six_and_seven_point_profiles_and_boundaries() {
    let points = [
        [0., 0.],
        [7., 3.],
        [14., -2.],
        [21., 4.],
        [28., -1.],
        [35., 3.],
        [42., 0.],
    ];
    for n in [6, 7] {
        let mut stroke = source(&points[..n], &vec![0.5; n]);
        assert!(crate::ink::montblanc::prepare(&stroke).is_some());
        let doc = document(stroke.clone());
        let generic = render_document_svg(&doc, &RenderOptions::default())[0]
            .svg
            .clone();
        for opts in [
            options(),
            RenderOptions {
                montblanc_two_pixels_per_page_unit: true,
                ..Default::default()
            },
        ] {
            assert_ne!(render_document_svg(&doc, &opts)[0].svg, generic);
            let layout = crate::layout_document(&doc);
            assert_eq!(
                render_layout_page_replay_svg(&doc, &layout, 0, &opts)
                    .unwrap()
                    .svg,
                render_layout_page_replay_svg(&doc, &layout, 0, &RenderOptions::default())
                    .unwrap()
                    .svg
            );
            let conflict = RenderOptions {
                montblanc_profile: Some(MontblancRenderProfile::HostUnitScale),
                montblanc_two_pixels_per_page_unit: true,
                ..Default::default()
            };
            assert_eq!(render_document_svg(&doc, &conflict)[0].svg, generic);
        }
        #[cfg(feature = "pdf")]
        {
            let layout = crate::layout_document(&doc);
            let render = |opts: &RenderOptions| {
                let request =
                    super::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
                super::DocumentTextCache::default()
                    .render_layout_page_scene_requests(
                        &doc,
                        &[request],
                        opts,
                        &super::fonts::FontBook::default(),
                        &mut |_| {},
                    )
                    .remove(0)
                    .page
                    .svg
            };
            assert_eq!(render(&options()), render(&RenderOptions::default()));
            assert_eq!(
                render(&RenderOptions {
                    montblanc_two_pixels_per_page_unit: true,
                    ..Default::default()
                }),
                render(&RenderOptions::default())
            );
        }
        for size in [0.1, 0.2, 0.25, 0.3, 0.4, 0.49] {
            stroke.pen_width = size;
            stroke.rendering.as_mut().unwrap().style.pen_size = Some(size);
            assert!(crate::ink::montblanc::prepare(&stroke).is_some());
        }
        stroke.pen_width = 0.09;
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(0.09);
        assert!(crate::ink::montblanc::prepare(&stroke).is_none());
        stroke.pen_width = 2.;
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(2.);
        stroke.pressures[1] = f64::NAN;
        assert!(crate::ink::montblanc::prepare(&stroke).is_none());
        stroke.pressures.pop();
        assert!(crate::ink::montblanc::prepare(&stroke).is_none());
        stroke.pressures = vec![0.5; n];
        stroke.points.fill(Point { x: 128., y: 128. });
        assert!(crate::ink::montblanc::prepare(&stroke).is_none());
    }
}
