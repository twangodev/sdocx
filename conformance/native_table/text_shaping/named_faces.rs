use super::*;
use crate::text_span_font_name as names;

const CONFIG_SHA256: &str = "9864ad4db5012ad4b63f82fcd0375b4f6a02a92d762147805ebdc02de0f4ed22";
const NAME_FIXTURE_SHA256: &str =
    "20ca51223fa1b5a010786586a12ab4d4c5937cd5ba4e1362ff661d6fab069754";

fn cases() -> Vec<Case> {
    let mut cases = vec![
        Case::regular("av", "AV", 17.0),
        Case::regular("to_fractional", "To", 17.125),
        Case::regular(
            "mixed_marks",
            "x\u{327}\u{301}χ\u{327}\u{301}Х\u{327}\u{301}",
            17.125,
        ),
        Case::regular("ligature", "office", 17.125),
        Case::regular("threshold_below", "To", f32::from_bits(1101256457)),
        Case::regular("threshold_exact", "To", f32::from_bits(1101256458)),
        Case::regular("threshold_above", "To", f32::from_bits(1101256459)),
    ];
    let mut context = Case::regular("partial_context", "😀AVx\u{327}\u{301}y🙂", 17.125);
    context.range = Some([2, 8]);
    cases.push(context);
    let mut rtl = Case::regular("rtl_marks", "AVx\u{327}\u{301}", 17.125);
    rtl.rtl = true;
    cases.push(rtl);
    for case in &mut cases {
        case.entry_geometry = true;
    }
    cases
}

fn profiles() -> Vec<(&'static str, u8, Case)> {
    let mut profiles = Vec::new();
    for name in [
        "Roboto-Regular",
        "Roboto-Bold",
        "Roboto-Italic",
        "Roboto-BoldItalic",
    ] {
        for flags in [0, 1, 4] {
            profiles.extend(cases().into_iter().map(|case| (name, flags, case)));
        }
    }
    for (name, text) in [("native_marker9", "9."), ("native_marker10", "10.")] {
        let mut case = Case::regular(name, text, 20.0);
        case.entry_geometry = true;
        profiles.push(("Roboto-Regular", 0, case));
    }
    profiles
}

pub(super) fn capture(
    machine: &mut Machine,
    base: &Path,
    text: &Path,
    skia: &Path,
    font: &Path,
    xml: &Path,
    cpp: &Path,
) {
    pinned_host(
        "conformance/table-text-span-font-name.json",
        NAME_FIXTURE_SHA256,
    );
    let (mut environment, mut services) = names::setup(machine, base, text, skia, font, xml, cpp);
    let mut shape_trace = TraceRecorder::new(machine, None, true, false);
    let profiles = profiles();
    let mut canonical = None;
    for fill in [0, 0xa5, 0xff, 0] {
        let mut outputs = Vec::new();
        for &(name, flags, ref case) in &profiles {
            names::reset(machine, &mut environment, &mut services, fill);
            services.icu.reset();
            *shape_trace.state = ShapeTrace {
                skia: Some(skia_metrics::SkiaTrace::default()),
                ..ShapeTrace::default()
            };
            let source_profile = names::Case {
                span_name: Some(name),
                default_name: Some("Roboto-Regular"),
                flags,
                direction: case.rtl,
                size: case.font_size,
            };
            let mut name_trace = names::TraceRecorder::new(machine);
            let profile = names::execute(machine, &services, source_profile, &mut name_trace);
            drop(name_trace);
            let shape = case.execute_prepared(
                machine,
                &mut shape_trace,
                &mut services.icu,
                profile.paint,
                profile.physical_source_id,
                None,
            );
            let font_records = read_u64(machine.engine, PIECE + 184);
            assert_eq!(read_u64(machine.engine, PIECE + 192) - font_records, 16);
            let native_font = read_u64(machine.engine, read_u64(machine.engine, font_records));
            let holder = machine.call(TEXT + 0x8eebc, &[native_font]);
            let physical = read_u64(machine.engine, holder);
            let layout_source_id = machine.call(TEXT + 0x88ad4, &[physical]);
            assert_eq!(layout_source_id, profile.physical_source_id);
            let indices = read_u64(machine.engine, PIECE);
            let indices_end = read_u64(machine.engine, PIECE + 8);
            assert!(
                bytes(machine.engine, indices, (indices_end - indices) as usize)
                    .iter()
                    .all(|&index| index == 0)
            );
            outputs.push(format!(
                        "{{\"source_profile\":{},\"layout_source_id\":{},\"layout_uses_selected_physical_face\":true,\"shape\":{}}}",
                        profile.json(source_profile), layout_source_id, shape,
                    ));
        }
        if let Some(expected) = &canonical {
            assert_eq!(&outputs, expected);
        } else {
            canonical = Some(outputs);
        }
    }
    println!(
        concat!(
            "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",",
            "\"model_library_sha256\":\"{}\",\"base_library_sha256\":\"{}\",\"text_library_sha256\":\"{}\",\"skia_library_sha256\":\"{}\",",
            "\"xml_library_sha256\":\"46753f76c8c007e78777e8fe7de7b57202f966f9494d4fba2b675c3540e35dbd\",\"cpp_library_sha256\":\"4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4\",",
            "\"font_manager_evidence_sha256\":\"{}\",\"supplied_config_sha256\":\"{}\",\"memory_fills\":[0,165,255],\"repeat_zero_fill\":true,",
            "\"host_icu_uc_sha256\":\"a8e433e81075732faf255b17d4a25ce28632e41fef1a75e727ee7f4ed73ab151\",\"host_icu_data_sha256\":\"a04b2b906193fa1e40f968a3d16d7d6c844a1fafbdd5bce6e9f67b01c124ff24\",",
            "\"host_libc_sha256\":\"fa430b8f298f817a266046af84a77533185ad6fc4406c7d3787b5a0a0c207826\",\"host_libm_sha256\":\"6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3\",",
            "\"source_flags\":[0,1,4],\"native_initializers\":{:?},\"instruction_limit\":10000000,\"source_counter_seed\":0,\"native_addresses\":{{\"layout_piece\":\"Text+0x9b150\",\"hb_shape\":\"Text+0xecdcc\",\"layout_append\":\"Text+0x9dbf0\",\"entry_geometry\":[\"Text+0x773e0\",\"Text+0x77678\"],\"skia_scaler\":\"Skia+0x27d4b4\",\"ft_load_glyph\":\"Skia+0x27dfe8\"}},",
            "\"capture_boundary\":{},\"cases\":[\n{}\n]}}"
        ),
        LIBRARY_SHA256,
        frames::BASE_SHA256,
        geometry::TEXT_SHA256,
        text_font_source::SKIA_SHA256,
        NAME_FIXTURE_SHA256,
        CONFIG_SHA256,
        INITIALIZERS,
        json_string(
            "Each case resets the native environment and executes the full FontManager/XML/four-file registration, actual SpanRun NAME/default selection, actual CreateFromFontName suffix parser and complete paint helper captured in the pinned font-manager evidence fixture. The supplied XML and four pinned Roboto files are the same bounded configuration; each profile records actual selected physical path/hash/style/ID. Actual returned MinikinPaint and FontCollection are consumed directly by native LayoutPiece, bundled HarfBuzz and Skia/FreeType. Actual LayoutPiece font slot0 source ID is checked against the selected physical font, with no additional physical slot or fallback. Captured HB input context/range/direction/scale/ppem and output, metric callback values, Skia scaler/load flags/matrix/outline and full/owner/glyph/UTF16 geometry execute. Actual append and the previously bounded SpanRun entry-conversion windows execute on produced geometry; constructor-only style0..7 profile controls do not claim fake-bold metrics. Omitted XML family language is recorded as null rather than an inferred locale label. Source flags0,1,4 establish normal/underline/base-skew paths on four physical styles; two additional regular-normal F20 numeric marker controls execute9. and10.; fake-bold source flag2 is excluded. Native process-local IDs use seed0 and fixed file order. The pinned prior fixture provides full XML/library/font/name-window and host-service provenance. No host font/XML/suffix selector or shaping/metric/layout algorithm is substituted. No device font equivalence, arbitrary fonts/configuration, fallback, whole SpanRun after paint conversion, wrapping/composition, raster or SVG output is established."
        ),
        canonical.unwrap().join(",\n"),
    );
}
