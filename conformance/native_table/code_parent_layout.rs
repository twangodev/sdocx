use super::*;
use widget_text_constructor::{self, DeviceProfile};

const MODEL_CODE: u64 = MODEL + 0x10000;
const TITLE_OBJECT: u64 = MODEL + 0x12000;
const BODY_OBJECT: u64 = MODEL + 0x14000;
const LAYOUT: u64 = MODEL + 0x20000;
const INSERTER: u64 = MODEL + 0x32000;
const NOTE_TEXT_MANAGER: u64 = MODEL + 0x34000;
const ANIMATOR_MANAGER: u64 = MODEL + 0x38000;
const WIDGET: u64 = 0x0c00_0000;
const CONTENT: u64 = 0x0d00_0000;
const VIEW: u64 = 0x0f00_0000;
const BODYTEXT: u64 = 0x1300_0000;
const COMPOSER: u64 = 0x1500_0000;
const HOST: u64 = 0x0300_0800;
const SET_RECT: u64 = HOST + 16;
const SET_LAYOUT: u64 = HOST + 32;
const UPDATE: u64 = HOST + 48;
const MEASURE_CHILD: u64 = HOST + 64;
const GET_TEXT_LAYOUT: u64 = HOST + 80;
const GET_HEIGHT: u64 = HOST + 96;
const GET_FIRST_LINE: u64 = HOST + 112;
const SET_PADDING: u64 = HOST + 128;
const TEXTS: [u64; 2] = [MODEL + 0x2a000, MODEL + 0x2b000];
const TEXT_VTABLE: u64 = MODEL + 0x2c000;
const PRIMARY_COLOR_GETTER: u64 = MODEL + 0xb0000;
const DOCUMENT_WIDTH_GETTER: u64 = MODEL + 0xb0020;
const DOCUMENT_DENSITY_GETTER: u64 = MODEL + 0xb0040;
const ANIMATOR_GETTER: u64 = MODEL + 0xb0060;

#[derive(Default)]
struct PlatformServices {
    random_state: u32,
}
const DRAWING_SHA256: &str = "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd";
const VIEW_SHA256: &str = "c4a17e4232c2074d3833604974d75ac961fab4d9651949bb2552704c86621dc8";
const BODYTEXT_SHA256: &str = "27324ca3807f07e0c1d0647b23eb9af1296762a8c9d892ee486f37b1eb9543f0";
const COMPOSER_SHA256: &str = "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f";
const XML_SHA256: &str = "46753f76c8c007e78777e8fe7de7b57202f966f9494d4fba2b675c3540e35dbd";
const CPP_SHA256: &str = "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4";

#[derive(Default)]
struct Child {
    frame: Option<[f32; 4]>,
    layout_frame: Option<[f32; 4]>,
    padding: Vec<[f32; 4]>,
    updates: usize,
    measures: usize,
    height_calls: usize,
    first_line_calls: usize,
}

#[derive(Default)]
struct Probe {
    children: [u64; 2],
    metrics: [Child; 2],
    calls: Vec<String>,
    heights: [f32; 2],
    first_line: f32,
}

fn map_composer(engine: Engine, path: &Path) {
    verify_library(path, COMPOSER_SHA256);
    let binary = fs::read(path).unwrap();
    assert_eq!(&binary[..4], b"\x7fELF");
    let ph_offset = u64::from_le_bytes(binary[32..40].try_into().unwrap()) as usize;
    let ph_size = u16::from_le_bytes(binary[54..56].try_into().unwrap()) as usize;
    let ph_count = u16::from_le_bytes(binary[56..58].try_into().unwrap()) as usize;
    let loads: Vec<_> = (0..ph_count)
        .map(|index| &binary[ph_offset + index * ph_size..][..ph_size])
        .filter(|header| u32::from_le_bytes(header[..4].try_into().unwrap()) == 1)
        .collect();
    assert_eq!(u64::from_le_bytes(loads[0][16..24].try_into().unwrap()), 0);
    let begin = COMPOSER & !0xfff;
    let end = loads
        .iter()
        .map(|load| {
            let address = COMPOSER + u64::from_le_bytes(load[16..24].try_into().unwrap());
            let size = u64::from_le_bytes(load[40..48].try_into().unwrap());
            (address + size + 0xfff) & !0xfff
        })
        .max()
        .unwrap();
    assert!(end > begin && end - begin <= 0x0100_0000);
    check(unsafe { uc_mem_map(engine, begin, end - begin, 7) });
    for load in loads {
        let offset = u64::from_le_bytes(load[8..16].try_into().unwrap()) as usize;
        let address = COMPOSER + u64::from_le_bytes(load[16..24].try_into().unwrap());
        let file_size = u64::from_le_bytes(load[32..40].try_into().unwrap()) as usize;
        let memory_size = u64::from_le_bytes(load[40..48].try_into().unwrap());
        assert!(file_size as u64 <= memory_size && memory_size > 0);
        assert!(address >= begin && address + memory_size <= end);
        write(engine, address, &binary[offset..offset + file_size]);
    }
    relocate_local_symbols(engine, &binary, COMPOSER);
}

fn rect(engine: Engine, address: u64) -> [f32; 4] {
    std::array::from_fn(|axis| read_float(engine, address + axis as u64 * 4))
}

fn float_return(engine: Engine, values: &[f32]) {
    for (axis, value) in values.iter().enumerate() {
        register(engine, 136 + axis as i32, u64::from(value.to_bits()));
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

fn assert_arena_disjoint(machine: &Machine) {
    let mut regions = vec![
        (MODEL_CODE, MODEL_CODE + 0x1000, "model codeblock"),
        (TITLE_OBJECT, TITLE_OBJECT + 0x1000, "title text"),
        (BODY_OBJECT, BODY_OBJECT + 0x1000, "body text"),
        (LAYOUT, LAYOUT + 0x800, "drawing layout"),
        (INSERTER, INSERTER + 0x400, "composer inserter"),
        (
            NOTE_TEXT_MANAGER,
            NOTE_TEXT_MANAGER + 0x1000,
            "note text manager",
        ),
        (
            ANIMATOR_MANAGER,
            ANIMATOR_MANAGER + 0x200,
            "animator manager",
        ),
        (TEXTS[0], TEXTS[0] + 0x20, "title metrics text"),
        (TEXTS[1], TEXTS[1] + 0x20, "body metrics text"),
        (TEXT_VTABLE, TEXT_VTABLE + 0x100, "metrics text vtable"),
        (
            MODEL + 0x50000,
            MODEL + 0x50300,
            "widget context and managers",
        ),
        (MODEL + 0x60000, MODEL + 0x60300, "widget getter code"),
        (MODEL + 0x61000, MODEL + 0x7140, "widget getter storage"),
        (
            PRIMARY_COLOR_GETTER,
            PRIMARY_COLOR_GETTER + 0x20,
            "primary color getter code",
        ),
        (
            PRIMARY_COLOR_GETTER + 0x1000,
            PRIMARY_COLOR_GETTER + 0x1004,
            "primary color getter storage",
        ),
        (
            DOCUMENT_WIDTH_GETTER,
            DOCUMENT_WIDTH_GETTER + 0x20,
            "document width getter code",
        ),
        (
            DOCUMENT_WIDTH_GETTER + 0x1000,
            DOCUMENT_WIDTH_GETTER + 0x1004,
            "document width getter storage",
        ),
        (
            DOCUMENT_DENSITY_GETTER,
            DOCUMENT_DENSITY_GETTER + 0x20,
            "document density getter code",
        ),
        (
            DOCUMENT_DENSITY_GETTER + 0x1000,
            DOCUMENT_DENSITY_GETTER + 0x1004,
            "document density getter storage",
        ),
        (
            ANIMATOR_GETTER,
            ANIMATOR_GETTER + 0x20,
            "animator getter code",
        ),
        (
            ANIMATOR_GETTER + 0x1000,
            ANIMATOR_GETTER + 0x1004,
            "animator getter storage",
        ),
    ];
    regions.sort_by_key(|region| region.0);
    for pair in regions.windows(2) {
        assert!(
            pair[0].1 <= pair[1].0,
            "{} overlaps {}",
            pair[0].2,
            pair[1].2
        );
    }
    let heap_end = machine.heap.cursor;
    assert!(
        heap_end <= TLS,
        "native allocator reached the TLS/stack arena"
    );
    for &(start, end, name) in &regions {
        assert!(
            heap_end <= start || HEAP >= end,
            "native heap overlaps {name}"
        );
    }
}

fn extra_host_import(engine: Engine, name: &str, args: [u64; 8], data: *mut c_void) -> Option<u64> {
    let services = unsafe { &mut *data.cast::<PlatformServices>() };
    if name.starts_with("pthread_mutexattr_") {
        return Some(0);
    }
    match name {
        "sincosf" => {
            let value = f32::from_bits(args[0] as u32);
            let (sin, cos) = value.sin_cos();
            write(engine, args[1], &sin.to_bits().to_le_bytes());
            write(engine, args[2], &cos.to_bits().to_le_bytes());
            Some(0)
        }
        "syscall" if args[0] == 0xb2 => {
            let count = args[2] as usize;
            assert!(count <= 4096);
            let mut bytes = vec![0; count];
            for byte in &mut bytes {
                services.random_state = services
                    .random_state
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223);
                *byte = (services.random_state >> 24) as u8;
            }
            write(engine, args[1], &bytes);
            Some(count as u64)
        }
        _ => None,
    }
}

fn child_json(child: &Child) -> String {
    format!(
        "{{\"frame\":{},\"layout_frame\":{},\"padding\":{:?},\"updates\":{},\"measures\":{},\"height_calls\":{},\"first_line_calls\":{}}}",
        child
            .frame
            .map_or("null".to_owned(), |value| format!("{value:?}")),
        child
            .layout_frame
            .map_or("null".to_owned(), |value| format!("{value:?}")),
        child.padding,
        child.updates,
        child.measures,
        child.height_calls,
        child.first_line_calls,
    )
}

unsafe extern "C" fn supplied_child(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let probe = unsafe { &mut *data.cast::<Probe>() };
    let receiver = read_register(engine, REGISTER_X0);
    if let Some(index) = TEXTS.iter().position(|value| *value == receiver) {
        assert_eq!(read_register(engine, REGISTER_X0 + 1), 0);
        if address == GET_HEIGHT {
            probe.metrics[index].height_calls += 1;
            float_return(engine, &[probe.heights[index]]);
        } else {
            assert_eq!(address, GET_FIRST_LINE);
            probe.metrics[index].first_line_calls += 1;
            float_return(engine, &[probe.first_line]);
        }
        probe.calls.push(format!(
            "text-{index}-{}",
            if address == GET_HEIGHT {
                "height"
            } else {
                "first-line"
            }
        ));
        return;
    }
    let index = match probe.children.iter().position(|value| *value == receiver) {
        Some(index) => index,
        None => {
            let index = probe
                .children
                .iter()
                .position(|value| *value == 0)
                .expect("more than two native CodeBlockLayout children");
            probe.children[index] = receiver;
            index
        }
    };
    let child = &mut probe.metrics[index];
    match address {
        SET_RECT => {
            child.frame = Some(std::array::from_fn(|axis| {
                f32::from_bits(read_register(engine, 136 + axis as i32) as u32)
            }));
            probe.calls.push(format!("child-{index}-set-position"));
        }
        SET_LAYOUT => {
            child.layout_frame = Some(rect(engine, read_register(engine, REGISTER_X0 + 1)));
            probe
                .calls
                .push(format!("child-{index}-update-screen-rect"));
        }
        UPDATE => {
            child.updates += 1;
            probe.calls.push(format!("child-{index}-update"));
        }
        MEASURE_CHILD => {
            child.measures += 1;
            probe.calls.push(format!("child-{index}-measure"));
        }
        GET_TEXT_LAYOUT => {
            register(engine, REGISTER_X0, TEXTS[index]);
            probe.calls.push(format!("child-{index}-get-text-layout"));
        }
        SET_PADDING => {
            let list = read_register(engine, REGISTER_X0 + 1);
            let implementation = read_u64(engine, list + 8);
            let count = read_u32(engine, implementation + 16) as usize;
            assert!(count <= 8);
            let mut node = read_u64(engine, implementation);
            child.padding.clear();
            for _ in 0..count {
                assert!((HEAP..TLS).contains(&node), "invalid list node {node:x}");
                child
                    .padding
                    .push(rect(engine, read_u64(engine, node + 16)));
                node = read_u64(engine, node + 8);
            }
            probe.calls.push(format!("child-{index}-set-padding"));
        }
        _ => panic!("unexpected child callback {address:x}"),
    }
}

fn install_callbacks(machine: &Machine, probe: &mut Box<Probe>) -> Vec<usize> {
    let engine = machine.engine;
    let mut hooks = Vec::new();
    for address in (HOST..=SET_PADDING).step_by(16) {
        write(engine, address, &0xd65f03c0_u32.to_le_bytes());
        let mut hook = 0;
        check(unsafe {
            uc_hook_add(
                engine,
                &mut hook,
                4,
                supplied_child as *mut c_void,
                ptr::from_mut(probe.as_mut()).cast(),
                address,
                address,
            )
        });
        hooks.push(hook);
    }
    hooks
}

fn drop_callbacks(engine: Engine, hooks: Vec<usize>) {
    for hook in hooks {
        check(unsafe { uc_hook_del(engine, hook) });
    }
}

fn bind_layout_imports(machine: &Machine) {
    splits::load_lists(machine);
    for (plt, target) in [
        (0xb8b10, frames::BASE + 0xb108c),
        (0xb8b20, frames::BASE + 0xb109c),
        (0xb9280, frames::BASE + 0xb11a4),
        (0xb8960, frames::BASE + 0xbe518),
        (0xb9330, DRAWING_BASE + 0x73694),
        (0xb92b0, 0x474570),
        (0xb93b0, frames::BASE + 0x9c87c),
        (0xb93e0, SET_PADDING),
        (0xb93f0, UPDATE),
    ] {
        bind_native(machine.engine, DRAWING_BASE + plt, target);
    }
}

fn direct_layout(machine: &mut Machine, context: u64, code: u64, density: f32) -> String {
    let layout = LAYOUT;
    write(machine.engine, layout, &[0; 2048]);
    machine.call(
        DRAWING_BASE + 0x72dec,
        &[layout, context, code, 0, u64::from(density.to_bits()), 0],
    );
    let children = [
        read_u64(machine.engine, layout + 672),
        read_u64(machine.engine, layout + 680),
    ];
    let child_vtables = [
        read_u64(machine.engine, children[0]),
        read_u64(machine.engine, children[1]),
    ];
    let offsets = [336_u64, 376, 392, 952];
    let mut originals = Vec::new();
    for vtable in child_vtables {
        for offset in offsets {
            originals.push((vtable, offset, read_u64(machine.engine, vtable + offset)));
        }
    }
    bind_layout_imports(machine);
    let mut probe = Box::new(Probe {
        children,
        heights: [91.25, 43.75],
        first_line: 17.25,
        ..Probe::default()
    });
    for text in TEXTS {
        write(machine.engine, text, &TEXT_VTABLE.to_le_bytes());
    }
    write(machine.engine, TEXT_VTABLE + 16, &GET_HEIGHT.to_le_bytes());
    write(
        machine.engine,
        TEXT_VTABLE + 224,
        &GET_FIRST_LINE.to_le_bytes(),
    );
    for vtable in child_vtables {
        for (offset, function) in [
            (336, SET_RECT),
            (392, SET_LAYOUT),
            (376, MEASURE_CHILD),
            (952, GET_TEXT_LAYOUT),
        ] {
            write(machine.engine, vtable + offset, &function.to_le_bytes());
        }
    }
    let hooks = install_callbacks(machine, &mut probe);
    machine.call(DRAWING_BASE + 0x732fc, &[layout]);
    machine.call(DRAWING_BASE + 0x738f4, &[layout]);
    let first_page = f32::from_bits(read_register(machine.engine, 136) as u32);
    let output = format!(
        "{{\"copy\":{:?},\"title\":{:?},\"body\":{:?},\"measured\":{:?},\"minimum_first_page_height\":{first_page:?},\"calls\":{:?}}}",
        rect(machine.engine, layout + 592),
        rect(machine.engine, layout + 608),
        rect(machine.engine, layout + 624),
        rect(machine.engine, layout + 560),
        probe.calls,
    );
    drop_callbacks(machine.engine, hooks);
    for (vtable, offset, method) in originals {
        write(machine.engine, vtable + offset, &method.to_le_bytes());
    }
    output
}

#[derive(Clone, Copy)]
struct Profile {
    name: &'static str,
    width: u32,
    density: f32,
}

fn capture_case(
    machine: &mut Machine,
    context: u64,
    code: u64,
    inserter: u64,
    profile: Profile,
) -> String {
    scalar_getter(
        machine,
        DOCUMENT_WIDTH_GETTER,
        u64::from(profile.width),
        false,
    );
    scalar_getter(
        machine,
        DOCUMENT_DENSITY_GETTER,
        u64::from(profile.density.to_bits()),
        true,
    );
    write(
        machine.engine,
        context + 152,
        &DOCUMENT_WIDTH_GETTER.to_le_bytes(),
    );
    write(
        machine.engine,
        context + 160,
        &DOCUMENT_DENSITY_GETTER.to_le_bytes(),
    );
    let layout_geometry = direct_layout(machine, context, code, profile.density);

    let real_vtable = DRAWING_BASE + 0xc3928;
    let offsets = [336_u64, 376, 392, 952];
    let originals: Vec<_> = offsets
        .iter()
        .map(|offset| (*offset, read_u64(machine.engine, real_vtable + offset)))
        .collect();
    let mut probe = Box::new(Probe {
        heights: [91.25, 43.75],
        first_line: 17.25,
        ..Probe::default()
    });
    for text in TEXTS {
        write(machine.engine, text, &TEXT_VTABLE.to_le_bytes());
    }
    write(machine.engine, TEXT_VTABLE + 16, &GET_HEIGHT.to_le_bytes());
    write(
        machine.engine,
        TEXT_VTABLE + 224,
        &GET_FIRST_LINE.to_le_bytes(),
    );
    for (offset, method) in [
        (336, SET_RECT),
        (392, SET_LAYOUT),
        (376, MEASURE_CHILD),
        (952, GET_TEXT_LAYOUT),
    ] {
        write(machine.engine, real_vtable + offset, &method.to_le_bytes());
    }
    let hooks = install_callbacks(machine, &mut probe);
    let mut returned = [0.0_f32; 4];
    machine.call(COMPOSER + 0x3a0b14, &[inserter, code]);
    for (axis, value) in returned.iter_mut().enumerate() {
        *value = f32::from_bits(read_register(machine.engine, 136 + axis as i32) as u32);
    }
    drop_callbacks(machine.engine, hooks);
    for (offset, method) in originals {
        write(machine.engine, real_vtable + offset, &method.to_le_bytes());
    }
    format!(
        "{{\"profile\":{{\"name\":{:?},\"document_width\":{},\"density\":{}}},\"supplied_child_metrics\":{{\"title_height\":91.25,\"body_height\":43.75,\"first_line_height\":17.25}},\"native_parent\":{{\"measured_rect\":{:?},\"title_child\":{},\"body_child\":{},\"callbacks\":{:?}}},\"native_code_layout\":{layout_geometry}}}",
        profile.name,
        profile.width,
        profile.density,
        returned,
        child_json(&probe.metrics[0]),
        child_json(&probe.metrics[1]),
        probe.calls,
    )
}

fn capture_profile(
    profile: Profile,
    model: &Path,
    base: &Path,
    drawing: &Path,
    bodytext: &Path,
    cpp: &Path,
    text: &Path,
    skia: &Path,
    font: &Path,
    xml: &Path,
    widget: &Path,
    content: &Path,
    view: &Path,
    composer: &Path,
) -> String {
    let paths = cell_model_lifecycle::Paths {
        model,
        base,
        drawing,
        body: bodytext,
        cpp,
    };
    let mut machine = cell_model_lifecycle::initialize_with_bases(&paths, 0, 0x0e000000, BODYTEXT);
    assert_arena_disjoint(&machine);
    machine.call_instruction_limit = 10_000_000;
    machine.call_timeout_micros = 0;
    let additional = vec![
        (xml, 0x0a00_0000, XML_SHA256),
        (widget, WIDGET, geometry::WIDGET_SHA256),
        (content, CONTENT, widget_text_constructor::CONTENT_SHA256),
        (view, VIEW, VIEW_SHA256),
    ];
    map_composer(machine.engine, composer);
    let preloaded = [
        (model, 0, LIBRARY_SHA256),
        (base, frames::BASE, frames::BASE_SHA256),
        (drawing, DRAWING_BASE, DRAWING_SHA256),
        (bodytext, BODYTEXT, BODYTEXT_SHA256),
        (cpp, 0x0e000000, CPP_SHA256),
        (composer, COMPOSER, COMPOSER_SHA256),
    ];
    let mut environment = text_font_source::NativeFontEnvironment::with_preloaded_libraries(
        &mut machine,
        base,
        text,
        skia,
        font,
        &additional,
        &preloaded,
    );
    let mut services = Box::<PlatformServices>::default();
    environment.set_host_import_handler(extra_host_import, ptr::from_mut(services.as_mut()).cast());
    environment.reset(&machine, 0, 0);
    for initializer in [0x2a2ed0, 0x2863a0, 0x3c35ec, 0x3c5918] {
        machine.call(initializer, &[]);
    }
    write(machine.engine, MODEL_CODE, &[0; 4096]);
    write(machine.engine, TITLE_OBJECT, &[0; 4096]);
    write(machine.engine, BODY_OBJECT, &[0; 4096]);
    machine.call(0x4741b8, &[MODEL_CODE]);
    machine.call(0x474258, &[MODEL_CODE]);
    machine.call(0x3e1378, &[TITLE_OBJECT]);
    machine.call(0x3e14f0, &[TITLE_OBJECT]);
    machine.call(0x3e1378, &[BODY_OBJECT]);
    machine.call(0x3e14f0, &[BODY_OBJECT]);
    machine.call(0x474368, &[MODEL_CODE, TITLE_OBJECT]);
    machine.call(0x474498, &[MODEL_CODE, BODY_OBJECT]);
    assert_arena_disjoint(&machine);
    let (context, _) =
        widget_text_constructor::configure_context(&machine, DeviceProfile::default());
    scalar_getter(&machine, PRIMARY_COLOR_GETTER, 0, false);
    write(
        machine.engine,
        context + 80,
        &PRIMARY_COLOR_GETTER.to_le_bytes(),
    );
    for (axis, value) in [0.0_f32, 0.0, profile.width as f32, 200.0]
        .into_iter()
        .enumerate()
    {
        register(
            machine.engine,
            136 + axis as i32,
            u64::from(value.to_bits()),
        );
    }
    machine.call(0x2ca380, &[MODEL_CODE, 0]);
    write(machine.engine, INSERTER, &[0; 1024]);
    machine.call(COMPOSER + 0x3a0860, &[INSERTER, context]);
    write(machine.engine, NOTE_TEXT_MANAGER, &[0; 4096]);
    machine.call(COMPOSER + 0x3a13a0, &[NOTE_TEXT_MANAGER, context]);
    write(
        machine.engine,
        INSERTER + 24,
        &NOTE_TEXT_MANAGER.to_le_bytes(),
    );
    write(machine.engine, ANIMATOR_MANAGER, &[0; 512]);
    machine.call(VIEW + 0x75e88, &[ANIMATOR_MANAGER]);
    assert_arena_disjoint(&machine);
    scalar_getter(&machine, ANIMATOR_GETTER, ANIMATOR_MANAGER, false);
    write(
        machine.engine,
        context + 8 + 48,
        &ANIMATOR_GETTER.to_le_bytes(),
    );
    capture_case(&mut machine, context, MODEL_CODE, INSERTER, profile)
}

pub(super) fn capture(
    _machine: Machine,
    model: &Path,
    base: &Path,
    drawing: &Path,
    bodytext: &Path,
    cpp: &Path,
    text: &Path,
    skia: &Path,
    font: &Path,
    xml: &Path,
    widget: &Path,
    content: &Path,
    view: &Path,
    composer: &Path,
) {
    let profiles = [
        Profile {
            name: "one-page-240x200-density-1",
            width: 240,
            density: 1.0,
        },
        Profile {
            name: "one-page-320x200-density-1_5",
            width: 320,
            density: 1.5,
        },
    ];
    let cases = profiles.map(|profile| {
        capture_profile(
            profile, model, base, drawing, bodytext, cpp, text, skia, font, xml, widget, content,
            view, composer,
        )
    });
    println!(
        "{{\"native_object_construction\":{{\"model_codeblock_ctor\":\"Model+0x4741b8\",\"model_codeblock_construct\":\"Model+0x474258\",\"model_text_ctor\":\"Model+0x3e1378\",\"model_text_construct\":\"Model+0x3e14f0\",\"drawing_codelayout_ctor\":\"Drawing+0x72dec\",\"composer_parent\":\"Composer+0x3a0b14\",\"animator_update_manager_ctor\":\"View+0x75e88\"}},\"capture_boundary\":\"Actual native Model CodeBlock and Text constructors/setters; actual Composer BodyTextObjectInserter parent constructs and measures Drawing CodeBlockLayout. ScrollEditTextView frame, update, measure, and TextLayout metrics; Context document width/density; and AnimatorUpdateManager availability are supplied emulator interfaces. Supplied values define an explicit metric profile and do not claim Android layout output or native text shaping.\",\"cases\":[{}]}}",
        cases.join(",")
    );
}
