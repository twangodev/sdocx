use super::*;
use text_font_source::{TEXT, json_string};

pub(super) const WIDGET: u64 = 0x0c00_0000;
pub(super) const CONTENT: u64 = 0x0d00_0000;
pub(super) const CELL_DRAWING: u64 = 0x0e00_0000;
pub(super) const CONTENT_SHA256: &str =
    "8b263185a6dfd89239cd0279bf68e4e95d887d42cc83ae5cc6fd94a4e703c24c";
const CONTEXT: u64 = MODEL + 0x50000;
const DISPLAY: u64 = CONTEXT + 0x100;
const MANAGER: u64 = CONTEXT + 0x200;
const THUNKS: u64 = MODEL + 0x60000;
pub(super) const CELL_LAYOUT: u64 = MODEL + 0x70000;

#[derive(Clone, Copy)]
pub(super) struct DeviceProfile {
    pub density: f32,
    pub scaled_density: f32,
    pub layout_direction: u32,
    pub tablet: bool,
    pub large_screen: bool,
    pub display_width: u32,
    pub manager: Option<(u32, f32, u32)>,
}
impl Default for DeviceProfile {
    fn default() -> Self {
        Self {
            density: 1.0,
            scaled_density: 1.0,
            layout_direction: 0,
            tablet: false,
            large_screen: false,
            display_width: 1080,
            manager: None,
        }
    }
}

fn scalar_getter(machine: &Machine, address: u64, value: u64, float: bool) {
    assert!(value <= u32::MAX as u64);
    let storage = address + 0x1000;
    write(machine.engine, storage, &(value as u32).to_le_bytes());
    let mut instructions = vec![
        0x5280_0000 | ((storage as u32 & 0xffff) << 5),
        0x72a0_0000 | ((storage as u32 >> 16) << 5),
        0xb940_0000,
    ];
    if float {
        instructions.push(0x1e27_0000);
    }
    instructions.push(0xd65f_03c0);
    let code: Vec<u8> = instructions
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    write(machine.engine, address, &code);
}

pub(super) fn construct_cell(machine: &Machine, destination: u64, profile: DeviceProfile) {
    for object in [CONTEXT, DISPLAY, MANAGER] {
        write(machine.engine, object, &(object + 8).to_le_bytes());
        write(machine.engine, object + 8, &[0; 248]);
    }
    scalar_getter(machine, THUNKS, DISPLAY, false);
    write(machine.engine, CONTEXT + 8 + 16, &THUNKS.to_le_bytes());
    for (index, (slot, value, float)) in [
        (48, u64::from(profile.display_width), false),
        (64, u64::from(profile.density.to_bits()), true),
        (72, u64::from(profile.density.to_bits()), true),
        (80, u64::from(profile.scaled_density.to_bits()), true),
        (112, u64::from(profile.layout_direction), false),
        (136, u64::from(profile.tablet), false),
        (144, u64::from(profile.large_screen), false),
        (152, u64::from(profile.large_screen), false),
    ]
    .into_iter()
    .enumerate()
    {
        let address = THUNKS + (index as u64 + 1) * 32;
        scalar_getter(machine, address, value, float);
        write(machine.engine, DISPLAY + 8 + slot, &address.to_le_bytes());
    }
    let manager = if let Some((document_width, document_pixel, font_delta)) = profile.manager {
        for (index, (slot, value, float)) in [
            (88, u64::from(font_delta), false),
            (128, u64::from(document_width), false),
            (144, u64::from(document_pixel.to_bits()), true),
        ]
        .into_iter()
        .enumerate()
        {
            let address = THUNKS + (index as u64 + 9) * 32;
            scalar_getter(machine, address, value, float);
            write(machine.engine, MANAGER + 8 + slot, &address.to_le_bytes());
        }
        MANAGER
    } else {
        0
    };
    assert_eq!(
        machine.call(THUNKS + 5 * 32, &[]),
        u64::from(profile.layout_direction)
    );
    machine.call(CELL_DRAWING + 0x8bff8, &[destination, CONTEXT, manager]);
    assert_eq!(
        machine.call(TEXT + 0x8bc38, &[text_wrapper(machine, destination)]),
        u64::from(profile.layout_direction)
    );
    assert_eq!(
        read_u64(machine.engine, read_u64(machine.engine, destination) + 24),
        CELL_DRAWING + 0x8c328
    );
}

pub(super) fn text_wrapper(machine: &Machine, destination: u64) -> u64 {
    let wrapper = machine.call(WIDGET + 0xd39ac, &[destination]);
    assert_eq!(wrapper, read_u64(machine.engine, destination + 368));
    wrapper
}

pub(super) fn snapshot(machine: &Machine, destination: u64, stage: &str) -> String {
    let wrapper = text_wrapper(machine, destination);
    let implementation = read_u64(machine.engine, wrapper + 64);
    let rich_state = read_u64(machine.engine, implementation);
    let single_line = machine.call(TEXT + 0x8b2ac, &[wrapper]);
    let word_wrap = machine.call(TEXT + 0x8b2f0, &[wrapper]);
    assert_eq!(single_line, 0);
    assert_eq!(word_wrap, 1);
    machine.call(TEXT + 0x8b860, &[wrapper]);
    let font_size = f32::from_bits(read_register(machine.engine, 136) as u32);
    let gravity = machine.call(TEXT + 0x8b8b4, &[wrapper]);
    let direction = machine.call(TEXT + 0x8bc38, &[wrapper]);
    let text_length = machine.call(TEXT + 0x8b104, &[wrapper]);
    assert_eq!(text_length, 0);
    let margins: Vec<f32> = [0x8b8f0, 0x8b90c, 0x8b928, 0x8b944]
        .map(|getter| {
            machine.call(TEXT + getter, &[wrapper]);
            f32::from_bits(read_register(machine.engine, 136) as u32)
        })
        .to_vec();
    format!(
        "{{\"stage\":{},\"genuine_cell_vtable\":true,\"bound_object_present\":{},\"wrapper_default_text_length\":{text_length},\"native_flags\":{:?},\"single_line\":false,\"word_wrap\":true,\"default_font_size\":{font_size:?},\"gravity\":{gravity},\"layout_direction\":{direction},\"screen_unit\":{},\"document_pixel\":{:?},\"density\":{:?},\"scaled_density\":{:?},\"margins\":{margins:?},\"text_scale\":{:?},\"layout_width\":{:?},\"bullet_document_pixel\":{:?}}}",
        json_string(stage),
        read_u64(machine.engine, destination + 416) != 0,
        text_font_source::bytes(machine.engine, rich_state + 112, 4),
        read_u32(machine.engine, rich_state + 188),
        read_float(machine.engine, rich_state + 192),
        read_float(machine.engine, rich_state + 196),
        read_float(machine.engine, rich_state + 200),
        read_float(machine.engine, destination + 600),
        read_float(machine.engine, destination + 524),
        read_float(
            machine.engine,
            read_u64(machine.engine, destination + 432) + 124
        ),
    )
}

pub(super) struct Paths<'a> {
    pub model: &'a Path,
    pub base: &'a Path,
    pub text: &'a Path,
    pub skia: &'a Path,
    pub font: &'a Path,
    pub xml: &'a Path,
    pub cpp: &'a Path,
    pub widget: &'a Path,
    pub content: &'a Path,
    pub drawing: &'a Path,
}

pub(super) fn capture(machine: &mut Machine, paths: Paths<'_>) {
    let (mut environment, services) = text_span_font_name::setup_with_preloaded_libraries(
        machine,
        paths.base,
        paths.text,
        paths.skia,
        paths.font,
        paths.xml,
        paths.cpp,
        &[
            (paths.widget, WIDGET, geometry::WIDGET_SHA256),
            (paths.content, CONTENT, CONTENT_SHA256),
            (paths.drawing, CELL_DRAWING, DRAWING_SHA256),
        ],
        &[(paths.model, 0, LIBRARY_SHA256)],
    );
    let mut host =
        text_span_font_name::cell_host::install_host(machine, &mut environment, services);
    let cases = [
        ("phone-null-manager", DeviceProfile::default()),
        (
            "tablet-scaled-direction1",
            DeviceProfile {
                density: 1.25,
                scaled_density: 1.5,
                layout_direction: 1,
                tablet: true,
                ..DeviceProfile::default()
            },
        ),
        (
            "large-screen-manager",
            DeviceProfile {
                density: 2.0,
                scaled_density: 2.25,
                large_screen: true,
                manager: Some((720, 1.75, 3)),
                ..DeviceProfile::default()
            },
        ),
    ];
    let mut canonical = None;
    for fill in [0, 165, 255, 0] {
        let mut output = Vec::new();
        for (name, profile) in cases {
            text_span_font_name::cell_host::reset_host(
                machine,
                &mut environment,
                &mut host,
                fill,
            );
            construct_cell(machine, CELL_LAYOUT, profile);
            let constructed = snapshot(machine, CELL_LAYOUT, "constructed");
            machine.call(WIDGET + 0xd3974, &[CELL_LAYOUT, 0]);
            let null_assigned = snapshot(machine, CELL_LAYOUT, "set-null-object");
            let object = MODEL + 0x71000;
            machine.call(0x3c17f0, &[object, 0, 0]);
            assert_eq!(machine.call(0x396e14, &[object]), 4);
            assert_eq!(machine.call(0x2d4c40, &[object]), 1);
            let source = machine.call(0x39c9e8, &[object + 40]);
            assert_eq!(source, 0);
            for (index, value) in [1.25_f32, 2.5, 3.75, 4.5].into_iter().enumerate() {
                register(
                    machine.engine,
                    136 + index as i32,
                    u64::from(value.to_bits()),
                );
            }
            assert_eq!(machine.call(0x39e678, &[object + 40]) & 1, 1);
            machine.call(WIDGET + 0xd3974, &[CELL_LAYOUT, object]);
            let assigned = snapshot(machine, CELL_LAYOUT, "set-native-type4-object");
            let mut scales = Vec::new();
            for scale in [0.75_f32, 1.5, 0.0, -1.0, 1.5] {
                register(machine.engine, 136, u64::from(scale.to_bits()));
                machine.call(WIDGET + 0xd948c, &[CELL_LAYOUT]);
                scales.push(format!(
                    "{{\"requested_scale\":{scale:?},\"state\":{}}}",
                    snapshot(machine, CELL_LAYOUT, "set-text-scale")
                ));
            }
            output.push(format!(
                "{{\"name\":{},\"device_density\":{:?},\"scaled_density\":{:?},\"layout_direction\":{},\"tablet\":{},\"large_screen\":{},\"manager_inputs\":{},\"native_model_shape_type\":4,\"native_model_belongs_to_table\":true,\"native_model_default_text\":null,\"supplied_object_margins\":[1.25,2.5,3.75,4.5],\"states\":[{constructed},{null_assigned},{assigned}],\"scale_controls\":[{}]}}",
                json_string(name), profile.density, profile.scaled_density, profile.layout_direction,
                profile.tablet, profile.large_screen,
                profile.manager.map_or_else(|| "null".into(), |(width,pixel,delta)| format!("{{\"document_width\":{width},\"document_pixel\":{pixel:?},\"font_delta\":{delta}}}")),
                scales.join(",")
            ));
        }
        if let Some(expected) = &canonical {
            assert_eq!(&output, expected);
        } else {
            canonical = Some(output);
        }
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"memory_fills\":[0,165,255],\"repeat_zero_fill\":true,\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{}\",\"text_library_sha256\":\"{}\",\"skia_library_sha256\":\"{}\",\"font_sha256\":\"{}\",\"widget_library_sha256\":{},\"content_library_sha256\":{},\"drawing_library_sha256\":{},\"name_environment\":{},\"native_addresses\":{{\"model_content_constructor\":\"Model+0x3c17f0\",\"model_shape_type_getter\":\"Model+0x396e14\",\"model_table_membership_getter\":\"Model+0x2d4c40\",\"cell_layout_constructor\":\"Drawing+0x8bff8\",\"cell_virtual_update_bound\":\"Drawing+0x8c328\",\"text_layout_constructor\":\"Widget+0xd2fb4\",\"constant_constructor\":\"Content+0x133e0\",\"text_wrapper_constructor\":\"Text+0x8a820\",\"text_wrapper_construct\":\"Text+0x8a8e8\",\"rich_text_construct\":\"Text+0x61d7c\",\"set_object\":\"Widget+0xd3974\",\"set_text_scale\":\"Widget+0xd948c\",\"update_bound\":\"Widget+0xd713c\"}},\"capture_boundary\":{},\"cases\":[{}]}}",
        frames::BASE_SHA256,
        geometry::TEXT_SHA256,
        text_font_source::SKIA_SHA256,
        text_font_source::FONT_SHA256,
        json_string(geometry::WIDGET_SHA256),
        json_string(CONTENT_SHA256),
        json_string(DRAWING_SHA256),
        text_span_font_name::dependency_metadata(),
        json_string(
            "Actual Drawing ObjectTableCellLayout constructor, Widget ObjectTextLayout constructor, Content Constant constructor/pixel lookup, TextLayout constructor/Construct, RichText and drawing constructors, bullet initialization, actual Model TableCellContentObject constructor/Construct and complete SetObject/SetTextScale execute. Model shape type4 and table membership are observed through actual getters; native class identity and virtual updateBound target are checked. Caller supplies IContext/IDisplay/ITextManager density/direction/profile/document width/pixel/font delta and margins via actual ComponentText setter; native constant tables, Model defaults, scale/margin multiplications and C++ algorithms execute. Model default text getter returns null and updateBound selects its absent-text font-size branch; SetObject does not copy text into the Text wrapper, whose constructor-default length remains0 because updateText is not called. Mode getters and initialized semantic fields exclude padding. Pinned FontManager XML/font files plus host allocation/byte/file services, bounded deterministic UUID identifiers, single-thread synchronization and ICU services are boundaries. No font producer, measurement/width conversion, paragraph placement or cached emission executes."
        ),
        canonical.unwrap().join(",")
    );
}
