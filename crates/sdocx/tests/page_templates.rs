#[allow(dead_code)]
mod support;
use sdocx::{DiagnosticCode, PageTemplateSource};

fn template_note(id: u32) -> sdocx::ParsedDocument {
    sdocx::parse_bytes_detailed(&support::archive(&support::page(
        &[vec![]],
        1 << 9,
        &id.to_le_bytes(),
    )))
    .unwrap()
}

#[test]
fn background_fields_and_unknown_template_identifiers_are_preserved() {
    let uri = "template://future";
    let mut fields = (uri.len() as u16).to_le_bytes().to_vec();
    for c in uri.encode_utf16() {
        fields.extend(c.to_le_bytes());
    }
    for value in [42_u32, 99, 0xff123456, 1848, 90, u32::MAX] {
        fields.extend(value.to_le_bytes());
    }
    let mask = (1 << 2) | (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 9);
    let parse = |fields: &[u8]| {
        sdocx::parse_bytes_detailed(&support::archive(&support::page(&[vec![]], mask, fields)))
    };
    let parsed = parse(&fields).unwrap();
    let page = &parsed.document.pages[0];
    let background = &page.background;
    assert_eq!(background.template_uri.as_deref(), Some(uri));
    assert_eq!(background.image_id, Some(42));
    assert_eq!(background.image_mode, Some(99));
    assert_eq!(background.width, Some(1848));
    assert_eq!(background.rotation, Some(90));
    assert_eq!(page.template.unwrap().id, u32::MAX);
    assert_eq!(page.template.unwrap().source, PageTemplateSource::BuiltIn);
    assert_eq!(page.background_color.unwrap().r, 0x12);
    assert!(
        parsed
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnsupportedPageTemplate)
    );
    for end in 0..fields.len() {
        assert!(parse(&fields[..end]).is_err());
    }
}

#[test]
fn templates_without_native_scaling_are_reported_instead_of_guessed() {
    for id in [1, 2, 3, 7, 8, 9, 10, 65536, u32::MAX] {
        let parsed = template_note(id);
        assert!(
            parsed
                .report
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::UnsupportedPageTemplate)
        );
    }
    assert!(
        !template_note(0)
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnsupportedPageTemplate)
    );
}

#[cfg(feature = "render")]
fn template_document(id: u32, width: u32, height: u32, orientation: i32) -> sdocx::Document {
    let mut document = template_note(id).document;
    document.pages[0].width = width;
    document.pages[0].height = height;
    document.metadata.default_page_dimensions = Some((width, height));
    document.metadata.orientation = Some(orientation);
    document
}

#[cfg(feature = "render")]
#[test]
fn native_grid_is_vector_scales_by_document_axis_and_clears_the_top_margin() {
    use sdocx::{RenderColorMode, RenderOptions, render_document_svg};
    let document = template_document(7, 1848, 2613, 0);
    let svg = &render_document_svg(&document, &Default::default())[0].svg;
    // APK-derived geometry, independently checked against the Samsung PDF:
    // measured source-space pitches are 91.6983 and 83.1642 (JPEG rounding).
    assert!(svg.contains("stroke-dasharray=\"0 91.700000\""));

    assert!(svg.contains("stroke=\"#010102\" stroke-opacity=\"0.2\""));
    assert_eq!(svg.matches("data-page-template=\"dots\"").count(), 1);
    assert_eq!(svg.matches(" H 1848").count(), 31);
    assert!(!svg.contains("<image "));
    // First center is below the clear native top margin.
    let first_y = svg
        .split("d=\"M 0 ")
        .nth(1)
        .unwrap()
        .split(' ')
        .next()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((56.0..57.0).contains(&first_y));
    let mut options = RenderOptions::default();
    options.color_mode = RenderColorMode::Dark;
    assert!(
        render_document_svg(&document, &options)[0]
            .svg
            .contains("stroke=\"#fafafa\" stroke-opacity=\"0.2\"")
    );
    let landscape = template_document(7, 2613, 1848, 1);
    let svg_landscape = &render_document_svg(&landscape, &Default::default())[0].svg;
    assert!(svg_landscape.contains("stroke-dasharray=\"0 91.700000\""));
    for (id, y) in [(7, "53.500000"), (8, "73.500000"), (9, "118.500000")] {
        let smaller = template_document(id, 1080, 1527, 0);
        let svg = &render_document_svg(&smaller, &Default::default())[0].svg;
        assert!(
            svg.contains(&format!("stroke-dasharray=\"0 {y}\"")),
            "{id}: {svg}"
        );
    }
}

#[cfg(feature = "render")]
#[test]
fn unknown_or_ambiguous_templates_keep_the_solid_background() {
    let mut document = template_document(7, 1848, 2613, 0);
    document.pages[0].background_color = Some(sdocx::Color {
        r: 18,
        g: 52,
        b: 86,
    });
    for orientation in [None, Some(99)] {
        document.metadata.orientation = orientation;
        let svg = &sdocx::render_document_svg(&document, &Default::default())[0].svg;
        assert!(!svg.contains("data-page-template="));
        assert!(svg.contains("fill=\"#123456\""));
    }
    document.metadata.orientation = Some(0);
    for id in [4, 10, 65536] {
        document.pages[0].template.as_mut().unwrap().id = id;
        assert!(
            !sdocx::render_document_svg(&document, &Default::default())[0]
                .svg
                .contains("data-page-template=")
        );
    }
    document.pages[0].template.as_mut().unwrap().id = 7;
    document.pages[0].background.rotation = Some(90);
    assert!(
        !sdocx::render_document_svg(&document, &Default::default())[0]
            .svg
            .contains("data-page-template=")
    );
}

#[cfg(feature = "render")]
#[test]
fn extreme_template_geometry_has_bounded_render_work() {
    let document = template_document(7, 1, u32::MAX, 0);
    let svg = &sdocx::render_document_svg(&document, &Default::default())[0].svg;
    assert!(!svg.contains("data-page-template="));
    assert!(svg.len() < 1024);
}

#[cfg(feature = "render")]
#[test]
fn ruled_templates_share_native_spacing_and_use_theme_specific_alpha() {
    use sdocx::{RenderColorMode, RenderOptions, render_document_svg};
    for (id, pitch) in [(1, 83.16), (2, 117.81), (3, 194.04)] {
        let doc = template_document(id, 1848, 2613, 0);
        let svg = &render_document_svg(&doc, &Default::default())[0].svg;
        assert!(svg.contains("data-page-template=\"lines\""));
        let ys: Vec<f64> = svg
            .split("M 0 ")
            .skip(1)
            .map(|row| row.split(' ').next().unwrap().parse().unwrap())
            .collect();
        assert!((ys[1] - ys[0] - pitch).abs() < 0.001);
        assert!((ys[0] - 52.0).abs() < 1.0);
        assert!(svg.contains("stroke=\"#010102\" stroke-opacity=\"0.2\""));
        assert!(!svg.contains("stroke-dasharray"));
        let mut dark = RenderOptions::default();
        dark.color_mode = RenderColorMode::Dark;
        assert!(
            render_document_svg(&doc, &dark)[0]
                .svg
                .contains("stroke=\"#fafafa\" stroke-opacity=\"0.3\"")
        );
        let landscape = template_document(id, 2613, 1848, 1);
        assert!(
            render_document_svg(&landscape, &Default::default())[0]
                .svg
                .contains(&format!("M 0 {:.6}", ys[0]))
        );
    }
}
