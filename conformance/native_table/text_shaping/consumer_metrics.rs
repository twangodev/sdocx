use super::*;
use crate::text_span_font_name as names;

const CONFIG_SHA256: &str = "9864ad4db5012ad4b63f82fcd0375b4f6a02a92d762147805ebdc02de0f4ed22";
const NAME_FIXTURE_SHA256: &str =
    "20ca51223fa1b5a010786586a12ab4d4c5937cd5ba4e1362ff661d6fab069754";

fn profiles() -> Vec<Case> {
    let mut profiles = Vec::new();
    for size in [10.0, 12.0, 15.0, 17.0, 20.0, 24.0, 30.0, 36.0, 45.0, 100.0] {
        for (name, text) in [
            ("marker1", "1."),
            ("marker9", "9."),
            ("marker10", "10."),
            ("markeraa", "aa."),
            ("bodyABC", "ABC"),
        ] {
            let mut case = Case::regular(name, text, size);
            case.entry_geometry = true;
            profiles.push(case);
        }
    }
    for (name, size) in [("native_AB_F10", 10.0), ("native_AB_F30", 30.0)] {
        let mut case = Case::regular(name, "AB", size);
        case.entry_geometry = true;
        profiles.push(case);
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
        for case in &profiles {
            names::reset(machine, &mut environment, &mut services, fill);
            services.icu.reset();
            *shape_trace.state = ShapeTrace {
                skia: Some(skia_metrics::SkiaTrace::default()),
                ..ShapeTrace::default()
            };
            let source_profile = names::Case {
                span_name: Some("Roboto-Regular"),
                default_name: Some("Roboto-Regular"),
                flags: 0,
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
            "\"source_flags\":[0],\"native_initializers\":{:?},\"instruction_limit\":10000000,\"source_counter_seed\":0,\"native_addresses\":{{\"layout_piece\":\"Text+0x9b150\",\"hb_shape\":\"Text+0xecdcc\",\"layout_append\":\"Text+0x9dbf0\",\"entry_geometry\":[\"Text+0x773e0\",\"Text+0x77678\"],\"skia_scaler\":\"Skia+0x27d4b4\",\"ft_load_glyph\":\"Skia+0x27dfe8\"}},",
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
            "Each of 52 regular-normal/LTR consumer metric profiles resets the native environment and executes actual FontManager/XML/four-file registration, SpanRun NAME/default selection, CreateFromFontName and complete paint helper established by the pinned font-manager fixture. Ten source sizes 10, 12, 15, 17, 20, 24, 30, 36, 45, 100 each measure 1., 9., 10., aa., ABC; two additional AB controls use source sizes 10 and 30. These strings are caller-supplied metric controls, not execution of native list numbering or object-width feedback algorithms. The supplied XML configuration and four pinned Roboto files match the prior font-manager fixture; only the actual selected regular physical face is used. Each output records its selected physical path/hash/style/ID, and actual LayoutPiece slot 0 source ID is checked against it without an additional slot or fallback. Actual returned MinikinPaint/FontCollection drive native LayoutPiece, bundled HarfBuzz and Skia/FreeType; source f32size multiplication by 100, HB context/range/direction/scale/ppem and output, raw metric callbacks, load flags/matrix/outline and glyph/UTF16 geometry execute. Actual append and previously bounded SpanRun entry-conversion windows execute on produced geometry. Omitted XML language remains null. Source flags 0 only; fake-bold, underline, skew and other physical faces are excluded from this corpus. Native IDs use seed 0 and fixed file order. All three memory fills and repeated zero fill produce identical records. The pinned prior fixture provides full font/XML/native-library/name-window/host-service provenance. No host selector, shaping, metric, list numbering or layout algorithm is substituted. No device font equivalence, arbitrary fonts/configuration, fallback, wrapping/composition, whole SpanRun, raster or SVG output is established."
        ),
        canonical.unwrap().join(",\n"),
    );
}
