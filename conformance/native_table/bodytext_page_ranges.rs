use super::*;

const BODY: u64 = 0x0f00_0000;
const BODY_SHA256: &str = "27324ca3807f07e0c1d0647b23eb9af1296762a8c9d892ee486f37b1eb9543f0";
const SOURCE: u64 = MODEL + 0xd0000;
const WRAPPER: u64 = MODEL + 0xd1000;
const STORAGE: u64 = MODEL + 0xd2000;
const DOCUMENT: u64 = MODEL + 0xd3000;
const DOCUMENT_VTABLE: u64 = MODEL + 0xd3100;
const PAGE_DATA: u64 = MODEL + 0xd3200;
const PAGE_COUNT: u64 = MODEL + 0xd3500;
const GET_PAGE: u64 = PAGE_COUNT + 32;
const LINE_SECTIONS: u64 = MODEL + 0xd3600;
const TEXT_SECTIONS: u64 = LINE_SECTIONS + 32;

#[derive(Clone, Copy)]
struct Input {
    name: &'static str,
    text: &'static str,
    width: f32,
    font_size: Option<f32>,
    margins: Option<[f32; 4]>,
    page_height: i32,
}

fn inputs() -> Vec<Input> {
    let ordinary = Input {
        name: "default-wide",
        text: "AV abc",
        width: 90.0,
        font_size: None,
        margins: None,
        page_height: 20,
    };
    vec![
        ordinary,
        Input {
            name: "default-narrow",
            width: 20.0,
            ..ordinary
        },
        Input {
            name: "ordinary-wrapped",
            text: "AV abc AV abc AV abc",
            width: 40.0,
            ..ordinary
        },
        Input {
            name: "hard-newline",
            text: "AV\nTo\nlast",
            ..ordinary
        },
        Input {
            name: "leading-newline",
            text: "\nAV",
            ..ordinary
        },
        Input {
            name: "consecutive-newlines",
            text: "AV\n\nTo",
            ..ordinary
        },
        Input {
            name: "newline-only",
            text: "\n",
            ..ordinary
        },
        Input {
            name: "empty-default",
            text: "",
            ..ordinary
        },
        Input {
            name: "font50-wrapped",
            text: "AV abc AV",
            font_size: Some(50.0),
            width: 100.0,
            page_height: 50,
            ..ordinary
        },
        Input {
            name: "fractional-margins",
            text: "AV abc AV",
            width: 50.0,
            margins: Some([1.25, 2.5, 3.75, 4.5]),
            ..ordinary
        },
        Input {
            name: "positive-first-top",
            text: "AV\nTo",
            margins: Some([0.0, 20.0, 0.0, 0.0]),
            ..ordinary
        },
        Input {
            name: "large-page",
            text: "AV\nTo\nlast",
            page_height: 100,
            ..ordinary
        },
    ]
}

const NATIVE_CALLS: [(&str, u64); 17] = [
    ("document_constructor", 0x3e1378),
    ("document_construct", 0x3e1408),
    ("widget_update", WIDGET + 0xd3e3c),
    ("widget_measure", WIDGET + 0xd73b0),
    ("widget_layout", WIDGET + 0xd3b88),
    ("layout_width", TEXT + 0x8ab50),
    ("text_length", TEXT + 0x8b104),
    ("line_count", TEXT + 0x8adb0),
    ("line_start", TEXT + 0x8ae34),
    ("line_end", TEXT + 0x8ae80),
    ("line_background", TEXT + 0x8b42c),
    ("line_top", TEXT + 0x8b4c0),
    ("first_empty", TEXT + 0x8ad54),
    ("default_cursor", WIDGET + 0xa36fc),
    ("page_indexer", BODY + 0xb77a4),
    ("down_line", BODY + 0xb7d90),
    ("up_line", BODY + 0xb7e88),
];

#[derive(Default)]
struct Observations {
    wrapper: u64,
    calls: BTreeMap<String, u32>,
}

unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observations = unsafe { &mut *data.cast::<Observations>() };
    let name = NATIVE_CALLS
        .iter()
        .find(|(_, target)| *target == address)
        .unwrap()
        .0;
    if observations.wrapper != 0 && address >= TEXT && address < TEXT + 0x100000 {
        assert_eq!(read_register(engine, REGISTER_X0), observations.wrapper);
    }
    let key = if matches!(
        name,
        "line_start" | "line_end" | "line_background" | "line_top" | "first_empty"
    ) {
        format!("{name}:{}", read_register(engine, REGISTER_X0 + 1) as i32)
    } else {
        name.to_owned()
    };
    *observations.calls.entry(key).or_default() += 1;
}

struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    observations: Box<Observations>,
}

impl Recorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            observations: Box::default(),
        };
        for (_, address) in NATIVE_CALLS {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    observe as *mut c_void,
                    ptr::from_mut(recorder.observations.as_mut()).cast(),
                    address,
                    address,
                )
            });
            recorder.hooks.push(hook);
        }
        recorder
    }

    fn take(&mut self) -> String {
        let calls = std::mem::take(&mut self.observations.calls)
            .into_iter()
            .map(|(name, count)| format!("{}:{count}", json_string(&name)))
            .collect::<Vec<_>>();
        format!("{{{}}}", calls.join(","))
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        for &hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, hook) });
        }
    }
}

struct PageInputs {
    pages: Vec<[i32; 5]>,
}

unsafe extern "C" fn page_interface(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let inputs = unsafe { &*data.cast::<PageInputs>() };
    assert_eq!(read_register(engine, REGISTER_X0), DOCUMENT);
    let result = if address == PAGE_COUNT {
        inputs.pages.len() as u64
    } else {
        assert_eq!(address, GET_PAGE);
        let index = read_register(engine, REGISTER_X0 + 1) as u32 as usize;
        if index < inputs.pages.len() {
            PAGE_DATA + index as u64 * 32
        } else {
            0
        }
    };
    register(engine, REGISTER_X0, result);
}

struct Pages {
    engine: Engine,
    hooks: Vec<usize>,
    inputs: Box<PageInputs>,
}

impl Pages {
    fn new(machine: &Machine, width: i32, height: i32) -> Self {
        let mut pages = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            inputs: Box::new(PageInputs {
                pages: (0..6)
                    .map(|index| [index * height, 0, 0, width, height])
                    .collect(),
            }),
        };
        write(machine.engine, DOCUMENT, &DOCUMENT_VTABLE.to_le_bytes());
        for (offset, address) in [(104, PAGE_COUNT), (120, GET_PAGE)] {
            write(
                machine.engine,
                DOCUMENT_VTABLE + offset,
                &address.to_le_bytes(),
            );
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    page_interface as *mut c_void,
                    ptr::from_mut(pages.inputs.as_mut()).cast(),
                    address,
                    address,
                )
            });
            pages.hooks.push(hook);
        }
        for (index, page) in pages.inputs.pages.iter().enumerate() {
            for (member, value) in page.iter().enumerate() {
                write(
                    machine.engine,
                    PAGE_DATA + index as u64 * 32 + member as u64 * 4,
                    &value.to_le_bytes(),
                );
            }
        }
        pages
    }
}

impl Drop for Pages {
    fn drop(&mut self) {
        for &hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, hook) });
        }
    }
}

fn ranges(machine: &Machine, vector: u64) -> Vec<[i32; 2]> {
    let start = read_u64(machine.engine, vector);
    let end = read_u64(machine.engine, vector + 8);
    assert!(end >= start && end - start <= 128 && (end - start).is_multiple_of(8));
    (0..(end - start) / 8)
        .map(|index| {
            [
                read_u32(machine.engine, start + index * 8) as i32,
                read_u32(machine.engine, start + index * 8 + 4) as i32,
            ]
        })
        .collect()
}

fn measured_lines(machine: &Machine, wrapper: u64) -> String {
    let count = machine.call(TEXT + 0x8adb0, &[wrapper]);
    assert!(count <= 64);
    let lines = (0..count).map(|index| {
        let start = machine.call(TEXT + 0x8ae34, &[wrapper, index]) as u32 as i32;
        let end = machine.call(TEXT + 0x8ae80, &[wrapper, index]) as u32 as i32;
        let background = returned_rect(machine, TEXT + 0x8b42c, &[wrapper, index]);
        machine.call(TEXT + 0x8b4c0, &[wrapper, index]);
        let top = read_register(machine.engine, 136) as u32;
        format!("{{\"line\":{index},\"source_inclusive_utf16\":[{start},{end}],\"line_top_bits\":{top},\"background_rect_bits\":{background:?}}}")
    }).collect::<Vec<_>>();
    format!("[{}]", lines.join(","))
}

fn model_source(machine: &Machine, length: u64) -> String {
    let font_sizes = (0..=length)
        .map(|index| {
            machine.call(0x39f504, &[SOURCE + 40, index]);
            read_register(machine.engine, 136) as u32
        })
        .collect::<Vec<_>>();
    let foregrounds = (0..=length)
        .map(|index| machine.call(0x39f4ec, &[SOURCE + 40, index]) as u32)
        .collect::<Vec<_>>();
    let margins = [0x39e680, 0x39e6c4, 0x39e708, 0x39e74c]
        .map(|getter| float_getter(machine, getter, SOURCE + 40));
    let common = read_u64(machine.engine, read_u64(machine.engine, SOURCE + 56) + 8);
    let vector = STORAGE + 0x800;
    write(machine.engine, vector, &[0; 24]);
    machine.call(0x3e380c, &[common, vector]);
    let begin = read_u64(machine.engine, vector);
    let end = read_u64(machine.engine, vector + 8);
    assert!(end >= begin && end - begin <= 2048 && (end - begin).is_multiple_of(8));
    let paragraphs = (begin..end).step_by(8).map(|pointer| {
        let paragraph = read_u64(machine.engine, pointer);
        let kind = machine.call(0x413d8c, &[paragraph]);
        let start = machine.call(0x413e48, &[paragraph]) as u32 as i32;
        let end = machine.call(0x413f04, &[paragraph]) as u32 as i32;
        let alignment = if kind == 3 { machine.call(0x40e5e4, &[paragraph]).to_string() } else { "null".into() };
        format!("{{\"kind\":{kind},\"paragraph_index_range\":[{start},{end}],\"alignment\":{alignment}}}")
    }).collect::<Vec<_>>();
    if begin != 0 {
        machine.call(0x47ac00, &[begin]);
    }
    format!(
        "{{\"font_size_at_utf16_including_end_bits\":{font_sizes:?},\"foreground_at_utf16_including_end\":{foregrounds:?},\"margin_bits\":{margins:?},\"paragraphs\":[{}],\"observation_after_page_indexing\":true}}",
        paragraphs.join(",")
    )
}

fn sample(
    machine: &mut Machine,
    environment: &mut NativeFontEnvironment,
    host: &mut Host,
    input: Input,
    fill: u8,
) -> String {
    reset_host(machine, environment, &mut host.cell, fill);
    machine.heap.cursor = MODEL + 0x80000;
    machine.heap.limit = MODEL + 0xc0000;
    write(machine.engine, SOURCE, &vec![0; 0x4000]);
    widget_text_constructor::configure_context(
        machine,
        widget_text_constructor::DeviceProfile::default(),
    );
    for (address, instructions) in [
        (COLOR, [0x2a0103e0_u32, 0xd65f03c0]),
        (DOCUMENT_WIDTH, [0x52800000, 0xd65f03c0]),
    ] {
        for (index, instruction) in instructions.into_iter().enumerate() {
            write(
                machine.engine,
                address + index as u64 * 4,
                &instruction.to_le_bytes(),
            );
        }
    }
    write(machine.engine, CONTEXT + 8 + 80, &COLOR.to_le_bytes());
    write(
        machine.engine,
        CONTEXT + 8 + 144,
        &DOCUMENT_WIDTH.to_le_bytes(),
    );
    let mut recorder = Recorder::new(machine);
    machine.call(0x3e1378, &[SOURCE]);
    let string = spen_string(machine, STORAGE, Some(input.text));
    assert_eq!(machine.call(0x3e1408, &[SOURCE, string, 0]) & 1, 1);
    if let Some(size) = input.font_size {
        float_arguments(machine, [size, 0., 0., 0.]);
        machine.call(0x3b06fc, &[read_u64(machine.engine, SOURCE + 56)]);
    }
    if let Some(margins) = input.margins {
        float_arguments(machine, margins);
        machine.call(0x39e678, &[SOURCE + 40]);
    }
    machine.call(WIDGET + 0xd2fb4, &[WRAPPER, CONTEXT, 0]);
    machine.call(WIDGET + 0xd3974, &[WRAPPER, SOURCE]);
    machine.call(WIDGET + 0xd3e3c, &[WRAPPER]);
    float_arguments(machine, [input.width, 0., 0., 0.]);
    machine.call(WIDGET + 0xd398c, &[WRAPPER]);
    float_arguments(machine, [1000., 0., 0., 0.]);
    machine.call(WIDGET + 0xd399c, &[WRAPPER]);
    machine.call(
        WIDGET + 0xd73b0,
        &[WRAPPER, 0, u32::MAX as u64, u32::MAX as u64],
    );
    machine.call(
        WIDGET + 0xd3b88,
        &[WRAPPER, 0, u32::MAX as u64, u32::MAX as u64, 0, 0],
    );
    let wrapper = machine.call(WIDGET + 0xd39ac, &[WRAPPER]);
    assert_eq!(wrapper, read_u64(machine.engine, WRAPPER + 368));
    recorder.observations.wrapper = wrapper;
    let source_text = read_spen_string(machine.engine, machine.call(0x39c9e8, &[SOURCE + 40]));
    assert_eq!(source_text.as_deref(), Some(input.text));
    let length = machine.call(TEXT + 0x8b104, &[wrapper]);
    assert_eq!(length as usize, input.text.encode_utf16().count());
    let width = machine.call(TEXT + 0x8ab50, &[wrapper]) as u32 as i32;
    let producer_calls = recorder.take();
    let lines = measured_lines(machine, wrapper);
    let first_empty = returned_rect(machine, TEXT + 0x8ad54, &[wrapper, width as u32 as u64]);
    let cursor = returned_rect(machine, WIDGET + 0xa36fc, &[wrapper]);
    let observation_calls = recorder.take();
    let pages = Pages::new(machine, width, input.page_height);
    let line_count = machine.call(TEXT + 0x8adb0, &[wrapper]);
    recorder.take();
    let mut queries = Vec::new();
    for page in 0..=pages.inputs.pages.len() {
        for line in 0..line_count.max(1) {
            let down = machine.call(BODY + 0xb7d90, &[wrapper, DOCUMENT, page as u64, line]);
            let up = machine.call(BODY + 0xb7e88, &[wrapper, DOCUMENT, page as u64, line]);
            assert!(down <= 1 && up <= 1);
            queries.push(format!(
                "{{\"page\":{page},\"line\":{line},\"down\":{},\"up\":{}}}",
                down != 0,
                up != 0
            ));
        }
    }
    let boundary_calls = recorder.take();
    let mut stages = Vec::new();
    for (name, first_page) in [("cold", 0_u64), ("repeat", 0), ("rescan-page1", 1)] {
        machine.call(
            BODY + 0xb77a4,
            &[wrapper, DOCUMENT, first_page, LINE_SECTIONS, TEXT_SECTIONS],
        );
        stages.push(format!("{{\"stage\":{},\"first_page\":{first_page},\"line_sections\":{:?},\"text_sections\":{:?},\"native_calls\":{}}}", json_string(name), ranges(machine, LINE_SECTIONS), ranges(machine, TEXT_SECTIONS), recorder.take()));
    }
    assert_eq!(measured_lines(machine, wrapper), lines);
    let model = model_source(machine, length);
    format!(
        "{{\"name\":{},\"input\":{{\"source_utf8\":{},\"requested_width_bits\":{},\"requested_height_bits\":{},\"requested_font_size_bits\":{},\"requested_margin_bits\":{},\"page_records\":{:?}}},\"produced\":{{\"native_source_utf8\":{},\"text_length_utf16\":{length},\"layout_width\":{width},\"default_font_size_bits\":{},\"model_source\":{model},\"lines\":{lines},\"first_empty_rect_bits\":{first_empty:?},\"default_cursor_rect_bits\":{cursor:?},\"producer_calls\":{producer_calls},\"getter_observation_calls\":{observation_calls}}},\"boundaries\":[{}],\"boundary_calls\":{boundary_calls},\"stages\":[{}]}}",
        json_string(input.name),
        json_string(input.text),
        input.width.to_bits(),
        1000_f32.to_bits(),
        input
            .font_size
            .map_or("null".into(), |size| size.to_bits().to_string()),
        input.margins.map_or("null".into(), |margins| format!(
            "{:?}",
            margins.map(f32::to_bits)
        )),
        pages.inputs.pages,
        optional_json(source_text.as_deref()),
        float_getter(machine, TEXT + 0x8b860, wrapper),
        queries.join(","),
        stages.join(",")
    )
}

pub(super) fn capture(
    machine: &mut Machine,
    paths: widget_text_constructor::Paths<'_>,
    body: &Path,
) {
    let (mut environment, fonts) = setup_with_preloaded_libraries(
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
            (body, BODY, BODY_SHA256),
        ],
        &[(paths.model, 0, LIBRARY_SHA256)],
    );
    let mut host = Box::new(Host {
        cell: install_host(machine, &mut environment, fonts),
        math_calls: BTreeMap::new(),
    });
    environment.set_host_import_handler(host_import, ptr::from_mut(host.as_mut()).cast());
    let mut output = Vec::new();
    for input in inputs() {
        let mut canonical = None;
        for fill in [0, 85, 165, 255, 0] {
            eprintln!("case {} allocation fill {fill}", input.name);
            let observed = sample(machine, &mut environment, &mut host, input, fill);
            if let Some(expected) = &canonical {
                assert_eq!(
                    &observed, expected,
                    "native allocation fill {fill} diverged"
                );
            } else {
                canonical = Some(observed);
            }
        }
        output.push(canonical.unwrap());
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"dependencies\":{},\"model_library_sha256\":{},\"base_library_sha256\":{},\"text_library_sha256\":{},\"skia_library_sha256\":{},\"xml_library_sha256\":{},\"cpp_library_sha256\":\"4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4\",\"widget_library_sha256\":{},\"content_library_sha256\":{},\"drawing_library_sha256\":{},\"bodytext_library_sha256\":{},\"memory_fills\":[0,85,165,255],\"repeat_zero_fill\":true,\"capture_boundary\":{},\"cases\":[{}]}}",
        dependency_metadata(),
        json_string(LIBRARY_SHA256),
        json_string(frames::BASE_SHA256),
        json_string(geometry::TEXT_SHA256),
        json_string(text_font_source::SKIA_SHA256),
        json_string(XML_SHA256),
        json_string(geometry::WIDGET_SHA256),
        json_string(CONTENT_SHA256),
        json_string(DRAWING_SHA256),
        json_string(BODY_SHA256),
        json_string(
            "Actual Model ObjectTextBox constructor/Construct and optional font/margin setters provide ordinary UTF16 source. Actual Widget constructor, SetObject, Update, Measure and Layout run with native Content/Text conversion, pinned font selection/shaping and paragraph placement. Actual TextLayout line/source/background/first-empty getters and Widget GetDefaultCursorRect execute unchanged; complete BodyTextPageIndexer boundary predicates and UpdateTextRangeOnEachPage consume that genuine wrapper. Only IBodyTextDocument page-count/page getters return explicitly supplied integer page records. Native line geometry, source owners, fallback rectangles and output sections are observed, never supplied. Repeat/rescan stages reuse actual previous native section vectors; their contents are not seeded from expected outputs. Source Model getters observe effective font/foreground/margins/paragraph records only after produced page output. Allocation fills vary all owned storage; caller source/context/page storage is initialized explicitly. Host ICU76.1/Unicode16, libc/allocation/file/single-thread services, explicit default device and identity color services match the frozen bodytext placement loader. No objects, source parsing, native page-bound conversion, full BodyTextLayout construction, obstacle producer, document repagination, Composer/PDF backend or pixels execute."
        ),
        output.join(",")
    );
}
