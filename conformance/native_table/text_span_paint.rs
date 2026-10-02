use super::*;
use text_font_source::{FONT_SHA256, NativeFontEnvironment, TEXT, bytes, json_string};
use text_shaping::{HostIcu, INITIALIZERS, host_import, pinned_host, vector};

const FONT: u64 = MODEL + 0x18000;
const FONT_VECTOR: u64 = MODEL + 0x18200;
const FONT_ITEM: u64 = MODEL + 0x18300;
const FAMILY: u64 = MODEL + 0x18400;
const LOCALE: u64 = MODEL + 0x18500;
const NAME: u64 = MODEL + 0x30000;
const INPUT: u64 = MODEL + 0x30100;
const SYSTEM_FONTS: u64 = MODEL + 0x30200;
const FAMILY_VECTOR: u64 = MODEL + 0x30300;
const FAMILY_ITEM: u64 = MODEL + 0x30400;
const SPAN: u64 = MODEL + 0x30500;
const OUTPUT: u64 = MODEL + 0x30600;
const FAMILY_OUTPUT: u64 = MODEL + 0x30700;
const FACTORY: u64 = MODEL + 0x30800;

#[derive(Clone, Copy)]
struct Case {
    size: f32,
    flags: u8,
    weight: i32,
    italic: i32,
}

struct Profile {
    family: String,
    typeface_style: u32,
    size_bits: u32,
    scale_bits: u32,
    skew_bits: u32,
    letter_spacing_bits: u32,
    word_spacing_bits: u32,
    flags: u32,
    locale_id: u32,
    weight: u16,
    italic: bool,
    variant: u8,
    collection_retains_family: bool,
    family_retains_font: bool,
}

impl Profile {
    fn json(&self, case: Case) -> String {
        format!(
            "{{\"source_size_bits\":{},\"source_flags\":{},\"source_foreground\":4279312947,\"requested_typeface_weight\":{},\"requested_typeface_italic\":{},\"resolved_family\":{},\"typeface_style\":{},\"paint_size_bits\":{},\"scale_x_bits\":{},\"skew_x_bits\":{},\"letter_spacing_bits\":{},\"word_spacing_bits\":{},\"packed_minikin_flags\":{},\"locale_id\":{},\"minikin_weight\":{},\"minikin_italic\":{},\"minikin_variant\":{},\"collection_retains_supplied_family\":{},\"family_retains_supplied_font\":{}}}",
            case.size.to_bits(),
            case.flags,
            case.weight,
            case.italic,
            json_string(&self.family),
            self.typeface_style,
            self.size_bits,
            self.scale_bits,
            self.skew_bits,
            self.letter_spacing_bits,
            self.word_spacing_bits,
            self.flags,
            self.locale_id,
            self.weight,
            self.italic,
            self.variant,
            self.collection_retains_family,
            self.family_retains_font,
        )
    }
}

fn execute(
    machine: &Machine,
    environment: &mut NativeFontEnvironment,
    case: Case,
    fill: u8,
) -> Profile {
    environment.reset(machine, fill, 0);
    environment.construct_font(machine, FONT, "", 400, false);
    for initializer in INITIALIZERS {
        machine.call(TEXT + initializer, &[]);
    }
    write(machine.engine, FONT_ITEM, &[0; 16]);
    write(machine.engine, FONT_ITEM, &FONT.to_le_bytes());
    vector(machine.engine, FONT_VECTOR, FONT_ITEM, 16);
    write(machine.engine, LOCALE, b"en-Latn\0");
    machine.call(TEXT + 0x8609c, &[FAMILY, LOCALE, 0, FONT_VECTOR]);
    let family_impl = read_u64(machine.engine, FAMILY + 8);
    let native_family = read_u64(machine.engine, family_impl + 8);
    write(machine.engine, INPUT, b"Roboto\0");
    machine.call(TEXT + 0x77e2c, &[NAME, INPUT]);
    machine.call(TEXT + 0x893d4, &[SYSTEM_FONTS]);
    write(
        machine.engine,
        FAMILY_ITEM,
        &bytes(machine.engine, FAMILY + 8, 16),
    );
    vector(machine.engine, FAMILY_VECTOR, FAMILY_ITEM, 16);
    machine.call(TEXT + 0x89410, &[SYSTEM_FONTS, NAME, FAMILY_VECTOR]);
    machine.call(TEXT + 0x89978, &[SYSTEM_FONTS, NAME]);
    machine.call(TEXT + 0x8a23c, &[FACTORY]);
    let typeface = machine.call(
        TEXT + 0x8a278,
        &[
            FACTORY,
            NAME,
            case.weight as i64 as u64,
            case.italic as i64 as u64,
        ],
    );
    assert_ne!(typeface, 0);
    let typeface_style = (machine.call(TEXT + 0x89f40, &[typeface]) as u32) & 0x1ffff;
    let expected_style = if case.weight < 0 {
        400
    } else {
        case.weight as u32
    } | if case.italic == 1 { 0x10000 } else { 0 };
    assert_eq!(typeface_style, expected_style);
    assert_eq!(machine.call(TEXT + 0x89ef8, &[typeface, FAMILY_OUTPUT]), 1);
    let family_bytes = bytes(machine.engine, FAMILY_OUTPUT, 7);
    assert_eq!(family_bytes, b"Roboto\0");
    write(machine.engine, SPAN, &[0; 72]);
    write(machine.engine, SPAN, &case.size.to_bits().to_le_bytes());
    write(machine.engine, SPAN + 4, &0xff112233_u32.to_le_bytes());
    write(machine.engine, SPAN + 16, &[case.flags]);
    register(machine.engine, REGISTER_X0 + 8, OUTPUT);
    machine.call(TEXT + 0x76ad4, &[SPAN, typeface, 0]);
    let paint = read_u64(machine.engine, OUTPUT);
    assert_ne!(paint, 0);
    let collection = read_u64(machine.engine, paint + 64);
    assert_ne!(collection, 0);
    let first_family = read_u64(machine.engine, read_u64(machine.engine, collection + 8));
    let font_record = machine.call(TEXT + 0x913d4, &[first_family, u64::from(typeface_style)]);
    let native_font = read_u64(machine.engine, font_record);
    let expected_font = read_u64(machine.engine, read_u64(machine.engine, FONT + 8) + 8);
    let physical_font_holder = machine.call(TEXT + 0x8eebc, &[native_font]);
    let family_retains_font = read_u64(machine.engine, physical_font_holder) == expected_font;
    let weight = u16::from_le_bytes(bytes(machine.engine, paint + 28, 2).try_into().unwrap());
    let profile = Profile {
        family: "Roboto".to_owned(),
        typeface_style,
        size_bits: read_u32(machine.engine, paint),
        scale_bits: read_u32(machine.engine, paint + 4),
        skew_bits: read_u32(machine.engine, paint + 8),
        letter_spacing_bits: read_u32(machine.engine, paint + 12),
        word_spacing_bits: read_u32(machine.engine, paint + 16),
        flags: read_u32(machine.engine, paint + 20),
        locale_id: read_u32(machine.engine, paint + 24),
        weight,
        italic: bytes(machine.engine, paint + 30, 1)[0] != 0,
        variant: bytes(machine.engine, paint + 32, 1)[0],
        collection_retains_family: first_family == native_family,
        family_retains_font,
    };
    assert!(profile.collection_retains_family);
    assert!(profile.family_retains_font);
    assert_eq!(profile.size_bits, (case.size * 100.0).to_bits());
    assert_eq!(u32::from(profile.weight), typeface_style & 0xffff);
    assert_eq!(profile.italic, typeface_style & 0x10000 != 0);
    assert_eq!(profile.flags, 0x20000 | u32::from(case.flags & 2) * 16);
    assert_eq!(
        profile.skew_bits,
        if case.flags & 4 != 0 {
            (-0.25_f32).to_bits()
        } else {
            0
        }
    );
    profile
}

fn cases() -> Vec<Case> {
    let typefaces = [(-1, -1), (400, 0), (700, 1)];
    let mut result = Vec::new();
    for (weight, italic) in typefaces {
        for flags in (0..=15).chain([64, 128, 192]) {
            result.push(Case {
                size: 17.125,
                flags,
                weight,
                italic,
            });
        }
        let threshold = 20.48_f32.to_bits();
        for size in [
            f32::from_bits(threshold - 1),
            f32::from_bits(threshold),
            f32::from_bits(threshold + 1),
            0.005,
            0.01,
            0.000001,
        ] {
            for flags in [0, 1] {
                result.push(Case {
                    size,
                    flags,
                    weight,
                    italic,
                });
            }
        }
    }
    result
}

pub(super) fn capture(machine: &mut Machine, base: &Path, text: &Path, skia: &Path, font: &Path) {
    pinned_host(
        "/usr/lib/x86_64-linux-gnu/libicuuc.so.76.1",
        "a8e433e81075732faf255b17d4a25ce28632e41fef1a75e727ee7f4ed73ab151",
    );
    pinned_host(
        "/usr/lib/x86_64-linux-gnu/libicudata.so.76.1",
        "a04b2b906193fa1e40f968a3d16d7d6c844a1fafbdd5bce6e9f67b01c124ff24",
    );
    machine.call_instruction_limit = 10_000_000;
    machine.call_timeout_micros = 0;
    let mut environment = NativeFontEnvironment::new(machine, base, text, skia, font);
    let mut host = HostIcu::new(machine);
    environment.set_host_import_handler(host_import, ptr::from_mut(host.as_mut()).cast());
    let outputs: Vec<_> = cases()
        .into_iter()
        .map(|case| {
            let mut canonical = None;
            for fill in [0, 0xa5, 0xff, 0] {
                let json = execute(machine, &mut environment, case, fill).json(case);
                if let Some(expected) = &canonical {
                    assert_eq!(&json, expected);
                } else {
                    canonical = Some(json);
                }
            }
            canonical.unwrap()
        })
        .collect();
    println!(
        "{{\"font_sha256\":\"{FONT_SHA256}\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{}\",\"text_library_sha256\":\"{}\",\"skia_library_sha256\":\"{}\",\"memory_fills\":[0,165,255],\"repeat_zero_fill\":true,\"native_initializers\":{:?},\"source_family\":\"Roboto\",\"host_icu_uc_sha256\":\"a8e433e81075732faf255b17d4a25ce28632e41fef1a75e727ee7f4ed73ab151\",\"host_icu_data_sha256\":\"a04b2b906193fa1e40f968a3d16d7d6c844a1fafbdd5bce6e9f67b01c124ff24\",\"supplied_file_font_weight\":400,\"supplied_file_font_italic\":false,\"native_addresses\":{{\"font_family_constructor\":\"Text+0x8609c\",\"register_fallback\":\"Text+0x89410\",\"register_default\":\"Text+0x89978\",\"create_from_family\":\"Text+0x8a278\",\"get_family_name\":\"Text+0x89ef8\",\"get_typeface_style\":\"Text+0x89f40\",\"complete_span_paint_helper\":\"Text+0x76ad4\",\"size_multiply\":\"Text+0x76b44\",\"final_typeface_style_copy\":[\"Text+0x76d18\",\"Text+0x76d24\"]}},\"capture_boundary\":\"Actual hash-pinned file Font and FontFamily, full SystemFontsImplMinikin RegisterFallback/RegisterDefault and actual family cache, full CreateFromFamilyName, family/style getters and complete Span paint helper execute. Caller supplies single regular Roboto family, en-Latn locale, requested Typeface weight/italic, source f32 size/foreground/flags and null feature String; inferred -1/-1 Typeface style executes native best-match physical-font flag inspection. Final MinikinPaint fields are observed after complete helper, including final Typeface style copy; Typeface getter is masked to its consumed low17 style bits because the fourth storage byte is unwritten allocation padding; supplied family/font pointer retention is checked. Host allocation/file/memory, bounded nonthrowing new and single-thread synchronization plus pinned host ICU property/locale services are boundaries. Typeface metadata controls700/italic do not prove a matching physical face, fake-bold metric parity or device FontManager font-name/XML/system-default resolution. Tiny positive size profiles establish constructor fields only; metrics below paint size1, whole SpanRunFunctor, layout/shaping, fallback, wrapping and composition do not execute.\",\"cases\":[\n{}\n]}}",
        frames::BASE_SHA256,
        geometry::TEXT_SHA256,
        text_font_source::SKIA_SHA256,
        INITIALIZERS,
        outputs.join(",\n")
    );
}
