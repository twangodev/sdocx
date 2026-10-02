use super::*;

const PARENT: u64 = MODEL + 0x33400;
const NAME_EVIDENCE_SHA256: &str =
    "20ca51223fa1b5a010786586a12ab4d4c5937cd5ba4e1362ff661d6fab069754";

struct Metadata {
    font: u64,
    implementation: u64,
    id: u64,
    bitmap: bool,
    language: String,
}
fn metadata(machine: &Machine, physical: u64, expected_id: u64) -> Metadata {
    let id = machine.call(TEXT + 0x88ad4, &[physical]);
    assert_eq!(id, expected_id);
    let bitmap = machine.call(TEXT + 0x88ae4, &[physical]);
    write(machine.engine, PARENT, &[0; 16]);
    register(machine.engine, REGISTER_X0 + 8, PARENT);
    machine.call(TEXT + 0x88b78, &[physical]);
    let manager = read_u64(machine.engine, PARENT);
    assert_ne!(manager, 0);
    let type_id = read_u64(machine.engine, TEXT + 0xf7a80);
    let type_info = read_u64(machine.engine, TEXT + 0xf7a88);
    let holder = machine.call(manager, &[3, PARENT, 0, type_id, type_info]);
    assert_ne!(holder, 0);
    let font = read_u64(machine.engine, holder);
    assert_ne!(font, 0);
    let implementation = read_u64(machine.engine, font + 8);
    assert_ne!(implementation, 0);
    assert_eq!(read_u64(machine.engine, implementation + 8), physical);
    assert_eq!(machine.call(TEXT + 0x85d7c, &[font]), id);
    assert_eq!(machine.call(TEXT + 0x877f8, &[implementation]), id);
    assert_eq!(machine.call(TEXT + 0x85d98, &[font]), bitmap);
    assert_eq!(machine.call(TEXT + 0x87814, &[implementation]), bitmap);
    let language_pointer = machine.call(TEXT + 0x85db0, &[font]);
    assert_eq!(
        machine.call(TEXT + 0x87834, &[implementation]),
        language_pointer
    );
    let language = cxx_string(machine.engine, language_pointer);
    machine.call(TEXT + 0x6a278, &[PARENT]);
    Metadata {
        font,
        implementation,
        id,
        bitmap: bitmap != 0,
        language,
    }
}
struct Configuration {
    name: &'static str,
    xml: String,
    requests: Vec<Case>,
}
fn configurations() -> Vec<Configuration> {
    let mut output = vec![Configuration {
        name: "omitted_language",
        xml: CONFIG.to_owned(),
        requests: cases(),
    }];
    for (name, language) in [
        ("explicit_empty_language", ""),
        ("und_language", "und"),
        ("deva_language", "und-Deva"),
    ] {
        let xml = CONFIG.replacen(
            "<family name=",
            &format!("<family lang=\"{language}\" name="),
            1,
        );
        let requests = [
            "Roboto-Regular",
            "missing",
            "Roboto-Bold",
            "Roboto-Italic",
            "Roboto-BoldItalic",
            "Roboto-Regular",
            "Roboto-Bold",
            "Roboto-Regular",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, span_name)| Case {
            span_name: Some(span_name),
            default_name: Some("Roboto-Regular"),
            flags: if index % 2 == 0 { 0 } else { 1 },
            direction: index % 2 != 0,
            size: 17.125,
        })
        .collect();
        output.push(Configuration {
            name,
            xml,
            requests,
        });
    }
    output
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
        NAME_EVIDENCE_SHA256,
    );
    let (mut environment, mut services) = setup(machine, base, text, skia, font, xml, cpp);
    let configurations = configurations();
    let mut canonical = None;
    for fill in [0, 0xa5, 0xff, 0] {
        let mut records = Vec::new();
        for configuration in &configurations {
            services.files[0].content = configuration.xml.as_bytes().to_vec();
            reset(machine, &mut environment, &mut services, fill);
            let mut recorder = TraceRecorder::new(machine);
            let mut sources = BTreeMap::new();
            let mut ids = BTreeMap::new();
            for (index, &case) in configuration.requests.iter().enumerate() {
                let profile = execute(machine, &services, case, &mut recorder);
                let metadata =
                    metadata(machine, profile.physical_source, profile.physical_source_id);
                let (first, font, implementation, source_id) = *sources
                    .entry(profile.physical_source)
                    .or_insert((index, metadata.font, metadata.implementation, metadata.id));
                assert_eq!(metadata.font, font);
                assert_eq!(metadata.implementation, implementation);
                assert_eq!(metadata.id, source_id);
                assert_eq!(
                    *ids.entry(metadata.id).or_insert(profile.physical_source),
                    profile.physical_source
                );
                records.push(format!("{{\"configuration\":{},\"request_index\":{index},\"source_first_request_index\":{first},\"same_source_as_first_request\":true,\"same_parent_font_as_first_request\":true,\"same_implementation_as_first_request\":true,\"parent_implementation_retains_selected_source\":true,\"source_id\":{},\"bitmap\":{},\"font_language\":{},\"selection\":{}}}",json_string(configuration.name),metadata.id,metadata.bitmap,json_string(&metadata.language),profile.json(case)));
            }
            assert_eq!(sources.len(), 4);
            assert_eq!(ids.len(), 4);
        }
        if let Some(expected) = &canonical {
            assert_eq!(&records, expected);
        } else {
            canonical = Some(records);
        }
    }
    let configurations = configurations
        .iter()
        .map(|configuration| {
            format!(
                "{{\"name\":{},\"xml\":{},\"request_count\":{}}}",
                json_string(configuration.name),
                json_string(&configuration.xml),
                configuration.requests.len()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        concat!(
            "{{\"apk_version\":\"4.4.45.37\",\"model_library_sha256\":\"{}\",\"base_library_sha256\":\"{}\",\"text_library_sha256\":\"{}\",\"skia_library_sha256\":\"{}\",",
            "\"name_evidence_sha256\":\"{NAME_EVIDENCE_SHA256}\",\"memory_fills\":[0,165,255],\"repeat_zero_fill\":true,\"source_counter_seed\":0,\"initialized_registry_per_configuration\":true,\"configurations\":[{}],",
            "\"native_addresses\":{{\"source_id\":\"Text+0x88ad4\",\"bitmap\":\"Text+0x88ae4\",\"parent_font\":\"Text+0x88b78\",\"parent_cast_reference\":[\"Text+0x77524\",\"Text+0x77590\"],\"font_source_id\":\"Text+0x85d7c\",\"font_bitmap\":\"Text+0x85d98\",\"font_language\":\"Text+0x85db0\",\"implementation_source_id\":\"Text+0x877f8\",\"implementation_bitmap\":\"Text+0x87814\",\"implementation_language\":\"Text+0x87834\",\"parent_any_cleanup\":\"Text+0x6a278\"}},",
            "\"capture_boundary\":{},\"cases\":[\n{}\n]}}"
        ),
        LIBRARY_SHA256,
        frames::BASE_SHA256,
        geometry::TEXT_SHA256,
        text_font_source::SKIA_SHA256,
        configurations,
        json_string(
            "Actual initialized FontManager/XML/four-file registry, NAME/default factory resolution and complete span paint helper execute under the pinned prior NAME evidence fixture. Each configuration constructs one fresh registry from the exact supplied XML; all its requests then execute without resetting the registry. Actual selected physical source ID, complete CBDT predicate, ParentFont std::any copy and the same actual cast used by SpanRunFunctor execute, followed by actual Font/FontImpl source/bitmap/language getters and native copy cleanup. Equality classes record actual source, parent-font and implementation pointer reuse while the registry remains alive; pointers are omitted from output. The baseline repeats all130 existing NAME requests; isolated explicit-empty, und and und-Deva caller-XML controls each perform8 requests. Supplied configurations and process counter seed0 are inputs, not recovered device configuration. This capture supplies no numeric SDK source identity, no UTF16 glyph/cache records and no host font-language return value. It establishes only these four unique physical registrations in one family per supplied registry, excluding duplicate font/family registration, arbitrary cache invalidation, device font choice, glyph fallback/itemization, later whole SpanRun cache construction, shaping, retained run grouping, wrapping/composition and output. Host XML/file/ICU/libc/libm/allocation and single-thread service boundaries are the same hash-pinned inputs specified by the prior NAME fixture; the real host snprintf one-C-string service also accepts the native und%s locale-format call reached by explicit language controls."
        ),
        canonical.unwrap().join(",\n"),
        NAME_EVIDENCE_SHA256 = NAME_EVIDENCE_SHA256
    );
}
