use super::*;
use crate::render::fonts::{
    PaintSpanProfile,
    fontdb::{Database, Style},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    span_name: Option<String>,
    default_name: Option<String>,
    source_flags: u8,
    caller_direction: bool,
    source_size_bits: u32,
    selected_name: Option<String>,
    factory_name: Option<String>,
    factory_direction: bool,
    parser_name: Option<String>,
    parser_matched: bool,
    parser_style: u32,
    factory_family: String,
    requested_weight: i32,
    requested_italic: i32,
    typeface_style: u32,
    resolved_family: String,
    physical_font_sha256: String,
    physical_font_style: u32,
    physical_fakery: u32,
    physical_face_index: u32,
    physical_data_size: usize,
    physical_mapping_is_pinned: bool,
    set_typeface_selects_same_physical_face: bool,
    paint_size_bits: u32,
    paint_scale_x_bits: u32,
    paint_skew_x_bits: u32,
    paint_flags: u32,
    paint_weight: u16,
    paint_italic: bool,
}

fn style_bits(style: NativeFontStyle) -> u32 {
    u32::from(style.weight()) | (u32::from(style.italic()) << 16)
}

#[test]
fn typed_name_resolution_matches_all_native_requests_styles_and_physical_files() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-text-span-font-name.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "20ca51223fa1b5a010786586a12ab4d4c5937cd5ba4e1362ff661d6fab069754"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 130);
    let book = FontBook::default();
    let mut logical_style_mismatches = 0;
    let mut empty_default_mismatches = 0;
    for case in capture.cases {
        let direction = if case.caller_direction {
            PaintShapeDirection::RightToLeft
        } else {
            PaintShapeDirection::LeftToRight
        };
        let request = NativeFontNameRequest::new(
            case.span_name.as_deref(),
            case.default_name.as_deref(),
            direction,
        )
        .unwrap();
        assert_eq!(request.span_name(), case.span_name.as_deref());
        assert_eq!(request.default_name(), case.default_name.as_deref());
        for name in [&case.selected_name, &case.factory_name, &case.parser_name] {
            assert_eq!(request.selected_name(), name.as_deref());
        }
        assert_eq!(request.direction(), direction);
        assert_eq!(case.factory_direction, case.caller_direction);
        assert_eq!(request.style_override().is_some(), case.parser_matched);
        assert_eq!(style_bits(request.parser_style()), case.parser_style);
        assert_eq!(
            request
                .style_override()
                .map_or(-1, |style| i32::from(style.weight())),
            case.requested_weight
        );
        assert_eq!(
            request
                .style_override()
                .map_or(-1, |style| i32::from(style.italic())),
            case.requested_italic
        );
        let resolution = book.resolve_native_name(&request).unwrap();
        assert_eq!(resolution.request(), &request);
        assert_eq!(resolution.family(), case.factory_family);
        assert_eq!(resolution.family(), case.resolved_family);
        assert_eq!(style_bits(resolution.typeface_style()), case.typeface_style);
        assert_eq!(resolution.typeface_style().weight(), case.paint_weight);
        assert_eq!(resolution.typeface_style().italic(), case.paint_italic);
        let face = resolution.face();
        assert_eq!(
            format!("{:x}", Sha256::digest(face.bytes())),
            case.physical_font_sha256
        );
        assert_eq!(face.index, case.physical_face_index);
        assert_eq!(face.bytes().len(), case.physical_data_size);
        assert_eq!(
            u32::from(face.weight.0) | (u32::from(face.style != Style::Normal) << 16),
            case.physical_font_style
        );
        assert_eq!(case.physical_fakery, 0);
        assert!(case.physical_mapping_is_pinned);
        assert!(case.set_typeface_selects_same_physical_face);
        let profile =
            PaintSpanProfile::new(f32::from_bits(case.source_size_bits), case.source_flags)
                .unwrap();
        assert_eq!(profile.size().to_bits(), case.paint_size_bits);
        assert_eq!(profile.scale_x().to_bits(), case.paint_scale_x_bits);
        assert_eq!(profile.skew_x().to_bits(), case.paint_skew_x_bits);
        assert_eq!(profile.packed_flags(), case.paint_flags);
        let logical_face = book
            .resolve(
                "Roboto",
                case.source_flags & 1 != 0,
                case.source_flags & 2 != 0,
            )
            .unwrap();
        logical_style_mismatches += usize::from(logical_face.id != face.id);
        if case.span_name.as_deref() == Some("")
            && case.default_name.as_deref() == Some("Roboto-Bold")
        {
            let absent =
                NativeFontNameRequest::new(None, case.default_name.as_deref(), direction).unwrap();
            empty_default_mismatches +=
                usize::from(book.resolve_native_name(&absent).unwrap().face().id != face.id);
        }
    }
    assert!(logical_style_mismatches > 0);
    assert_eq!(empty_default_mismatches, 10);
}

#[test]
fn native_style_parser_uses_priority_positive_positions_and_substrings() {
    for (pattern, weight, italic) in [
        ("-Regular", 400, false),
        ("-Italic", 400, true),
        ("-BoldItalic", 700, true),
        ("-Bold", 700, false),
        ("-ThinItalic", 100, true),
        ("-Thin", 100, false),
        ("-LightItalic", 300, true),
        ("-Light", 300, false),
        ("-MediumItalic", 500, true),
        ("-Medium", 500, false),
        ("-BlackItalic", 900, true),
        ("-Black", 900, false),
    ] {
        let name = format!("Family{pattern}Extra");
        let request =
            NativeFontNameRequest::new(Some(&name), None, PaintShapeDirection::LeftToRight)
                .unwrap();
        assert_eq!(
            request.style_override(),
            Some(NativeFontStyle { weight, italic })
        );
        let at_zero =
            NativeFontNameRequest::new(Some(pattern), None, PaintShapeDirection::LeftToRight)
                .unwrap();
        assert_eq!(at_zero.style_override(), None);
    }
    for (name, expected) in [
        ("Family-Regular-Italic", REGULAR),
        (
            "Family-BoldItalic-Italic",
            NativeFontStyle {
                weight: 400,
                italic: true,
            },
        ),
        (
            "-Bold-Bold",
            NativeFontStyle {
                weight: 700,
                italic: false,
            },
        ),
        (
            "族-BoldExtra",
            NativeFontStyle {
                weight: 700,
                italic: false,
            },
        ),
    ] {
        assert_eq!(
            NativeFontNameRequest::new(Some(name), None, PaintShapeDirection::LeftToRight)
                .unwrap()
                .style_override(),
            Some(expected)
        );
    }
    for name in ["family-bold", "Family Bold", "", "-Bold"] {
        assert_eq!(
            NativeFontNameRequest::new(Some(name), None, PaintShapeDirection::LeftToRight)
                .unwrap()
                .style_override(),
            None
        );
    }
}

#[test]
fn caller_configuration_uses_exact_registered_names_and_preserves_native_family_metadata() {
    let config = NativeFontNameConfig::new("default-native")
        .unwrap()
        .with_family_alias("default-native", "Roboto")
        .unwrap()
        .with_family_alias("registered-native", "Roboto Mono")
        .unwrap()
        .with_font_file("Some-Family-Bold.ttf", "registered-native")
        .unwrap();
    let book = FontBook::default().with_native_name_config(config);
    for name in ["Some-Family-Bold", "Some-Family"] {
        let request =
            NativeFontNameRequest::new(Some(name), None, PaintShapeDirection::LeftToRight).unwrap();
        let resolved = book.resolve_native_name(&request).unwrap();
        assert_eq!(resolved.family(), "registered-native");
        assert_eq!(resolved.face().family, "Roboto Mono");
        assert!(!resolved.used_default_family());
    }
    for name in ["Some", "Some-Family-Bold-Other", "unknown"] {
        let request =
            NativeFontNameRequest::new(Some(name), None, PaintShapeDirection::LeftToRight).unwrap();
        let resolved = book.resolve_native_name(&request).unwrap();
        assert_eq!(resolved.family(), "default-native");
        assert_eq!(resolved.face().family, "Roboto");
        assert!(resolved.used_default_family());
    }
    for name in [None, Some("")] {
        let request =
            NativeFontNameRequest::new(name, None, PaintShapeDirection::RightToLeft).unwrap();
        let resolved = book.resolve_native_name(&request).unwrap();
        assert_eq!(resolved.family(), "");
        assert_eq!(resolved.face().family, "Roboto");
    }
}

#[test]
fn malformed_names_missing_configuration_and_unavailable_aliases_fail_explicitly() {
    let direction = PaintShapeDirection::LeftToRight;
    assert!(matches!(
        NativeFontNameRequest::new(Some("Roboto\0-Bold"), None, direction),
        Err(NativeFontNameError::EmbeddedNul)
    ));
    assert!(matches!(
        NativeFontNameRequest::new(Some(&"a".repeat(MAX_NAME_BYTES + 1)), None, direction),
        Err(NativeFontNameError::InputBudget)
    ));
    assert!(matches!(
        NativeFontNameConfig::new(""),
        Err(NativeFontNameError::EmptyFamily)
    ));
    assert!(matches!(
        NativeFontNameConfig::new("native")
            .unwrap()
            .with_font_file("Shared-Regular.ttf", "first")
            .unwrap()
            .with_font_file("Shared-Bold.ttf", "second"),
        Err(NativeFontNameError::ConflictingRegistration)
    ));
    let request = NativeFontNameRequest::new(None, Some("Roboto"), direction).unwrap();
    let book = FontBook::new(Arc::new(Database::new()));
    assert!(matches!(
        book.resolve_native_name(&request),
        Err(NativeFontNameError::UnsupportedConfiguration)
    ));
    let missing_alias =
        FontBook::default().with_native_name_config(NativeFontNameConfig::new("native").unwrap());
    assert!(matches!(
        missing_alias.resolve_native_name(&request),
        Err(NativeFontNameError::UnsupportedConfiguration)
    ));
    let unavailable = FontBook::default().with_native_name_config(
        NativeFontNameConfig::new("native")
            .unwrap()
            .with_family_alias("native", "Missing Family")
            .unwrap(),
    );
    assert!(matches!(
        unavailable.resolve_native_name(&request),
        Err(NativeFontNameError::Font(FontError::MissingFont { .. }))
    ));
}
