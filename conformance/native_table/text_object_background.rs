use super::*;
use frames::{BASE, BASE_SHA256};
use geometry::TEXT_SHA256;

const TEXT: u64 = 0x0500_0000;
const PARAGRAPH: u64 = MODEL + 0x18000;
const MEASURE: u64 = PARAGRAPH + 0x100;
const RICH_PARAGRAPH: u64 = PARAGRAPH + 0x200;
const LINE: u64 = PARAGRAPH + 0x300;
const BLOCK: u64 = PARAGRAPH + 0x400;
const BLOCK_POINTER: u64 = PARAGRAPH + 0x500;
const ENTRY: u64 = PARAGRAPH + 0x600;
const MAP: u64 = PARAGRAPH + 0x700;
const PLACED_LINE: u64 = PARAGRAPH + 0x800;
const SPAN: u64 = PARAGRAPH + 0x900;
const FUNCTOR: u64 = PARAGRAPH + 0xa00;
const CONTEXT: u64 = PARAGRAPH + 0xb00;
const ENTRY_VECTOR: u64 = PARAGRAPH + 0xc00;
const DRAWING: u64 = PARAGRAPH + 0xd00;
const RICH_TEXT: u64 = PARAGRAPH + 0xe00;
const RICH_IMPL: u64 = PARAGRAPH + 0xf00;
const TEXT_CANVAS: u64 = PARAGRAPH + 0x1000;
const CANVAS_IMPL: u64 = PARAGRAPH + 0x1100;
const CANVAS_VTABLE: u64 = PARAGRAPH + 0x1200;
const PAINT_IMPL: u64 = PARAGRAPH + 0x1300;
const PAINT_VTABLE: u64 = PARAGRAPH + 0x1400;
const HOST: u64 = 0x0300_1800;
const COPY: u64 = HOST;
const CONSTRUCT_PAINT: u64 = HOST + 32;
const DESTROY_PAINT: u64 = HOST + 64;
const SET_STYLE: u64 = HOST + 96;
const SET_COLOR: u64 = HOST + 128;
const DRAW_RECT: u64 = HOST + 160;

fn pointer(engine: Engine, address: u64, value: u64) {
    write(engine, address, &value.to_le_bytes());
}

fn word(engine: Engine, address: u64, value: u32) {
    write(engine, address, &value.to_le_bytes());
}

fn float(engine: Engine, address: u64, value: f32) {
    assert!(value.is_finite());
    word(engine, address, value.to_bits());
}

fn rect(engine: Engine, address: u64) -> [f32; 4] {
    std::array::from_fn(|axis| read_float(engine, address + axis as u64 * 4))
}

fn write_rect(engine: Engine, address: u64, bounds: [f32; 4]) {
    for (axis, value) in bounds.into_iter().enumerate() {
        float(engine, address + axis as u64 * 4, value);
    }
}

#[derive(Default)]
struct Observation {
    commands: Vec<String>,
    constructors: usize,
    destructors: usize,
}

unsafe extern "C" fn interface(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    let object = read_register(engine, REGISTER_X0);
    let argument = read_register(engine, REGISTER_X0 + 1);
    match address {
        COPY => {
            let size = usize::try_from(read_register(engine, REGISTER_X0 + 2)).unwrap();
            assert!(size <= 80);
            let mut bytes = vec![0; size];
            check(unsafe { uc_mem_read(engine, argument, bytes.as_mut_ptr().cast(), size) });
            write(engine, object, &bytes);
        }
        CONSTRUCT_PAINT => {
            pointer(engine, object, 0);
            pointer(engine, object + 8, PAINT_IMPL);
            observation.constructors += 1;
        }
        DESTROY_PAINT => {
            assert_eq!(object, PAINT_IMPL);
            observation.destructors += 1;
        }
        SET_STYLE => {
            assert_eq!(object, PAINT_IMPL);
            word(engine, object + 8, argument as u32);
        }
        SET_COLOR => {
            assert_eq!(object, PAINT_IMPL);
            word(engine, object + 12, argument as u32);
        }
        DRAW_RECT => {
            assert_eq!(object, CANVAS_IMPL);
            let paint = read_register(engine, REGISTER_X0 + 2);
            assert_eq!(read_u64(engine, paint + 8), PAINT_IMPL);
            let rectangle = rect(engine, argument);
            observation.commands.push(format!(
                "{{\"rect\":{rectangle:?},\"rect_bits\":{:?},\"style\":{},\"color\":{}}}",
                rectangle.map(f32::to_bits),
                read_u32(engine, PAINT_IMPL + 8),
                read_u32(engine, PAINT_IMPL + 12)
            ));
        }
        _ => unreachable!(),
    }
}

struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    observation: Box<Observation>,
}

impl Recorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            observation: Box::default(),
        };
        for address in [
            COPY,
            CONSTRUCT_PAINT,
            DESTROY_PAINT,
            SET_STYLE,
            SET_COLOR,
            DRAW_RECT,
        ] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    interface as *mut c_void,
                    ptr::from_mut(recorder.observation.as_mut()).cast(),
                    address,
                    address,
                )
            });
            recorder.hooks.push(hook);
        }
        recorder
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

#[derive(Clone)]
struct Case {
    name: String,
    inline: bool,
    object: bool,
    width_bits: u32,
    height_bits: u32,
    context_margins: [f32; 4],
    stored_margins: [f32; 2],
    background: u32,
    composing: u32,
    background_enabled: bool,
    offset: [f32; 2],
    cursor: f32,
    line_top_margin: f32,
    alignment: u32,
}

impl Default for Case {
    fn default() -> Self {
        Self {
            name: String::new(),
            inline: true,
            object: true,
            width_bits: 30.0_f32.to_bits(),
            height_bits: 40.0_f32.to_bits(),
            context_margins: [0.0; 4],
            stored_margins: [0.0; 2],
            background: 0x8012_3456,
            composing: 0,
            background_enabled: true,
            offset: [4.25, 12.5],
            cursor: 3.25,
            line_top_margin: 0.0,
            alignment: 0,
        }
    }
}

impl Case {
    fn fixture(&self, machine: &Machine, fill: u8) -> String {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        write(machine.engine, PARAGRAPH, &vec![0; 0x1500]);
        for (address, value) in [
            (FUNCTOR + 16, CONTEXT),
            (CONTEXT, ENTRY_VECTOR),
            (ENTRY_VECTOR, ENTRY),
            (PARAGRAPH + 96, MEASURE),
            (PARAGRAPH + 104, PLACED_LINE),
            (PARAGRAPH + 112, PLACED_LINE),
            (PARAGRAPH + 120, PLACED_LINE + 56),
            (PARAGRAPH + 152, MAP),
            (PARAGRAPH + 184, BLOCK_POINTER),
            (MEASURE, ENTRY),
            (MEASURE + 8, RICH_PARAGRAPH),
            (LINE, BLOCK_POINTER),
            (LINE + 8, BLOCK_POINTER + 8),
            (BLOCK_POINTER, BLOCK),
            (DRAWING + 64, RICH_TEXT),
            (DRAWING + 96, ENTRY),
            (RICH_TEXT, RICH_IMPL),
            (RICH_IMPL + 8, SPAN),
            (RICH_IMPL + 16, SPAN + 72),
            (RICH_IMPL + 24, SPAN + 72),
            (TEXT_CANVAS + 8, CANVAS_IMPL),
            (CANVAS_IMPL, CANVAS_VTABLE),
            (CANVAS_VTABLE + 96, DRAW_RECT),
            (PAINT_IMPL, PAINT_VTABLE),
            (PAINT_VTABLE + 8, DESTROY_PAINT),
            (PAINT_VTABLE + 112, SET_STYLE),
            (PAINT_VTABLE + 120, SET_COLOR),
        ] {
            pointer(machine.engine, address, value);
        }
        float(machine.engine, CONTEXT + 20, 120.0);
        float(machine.engine, CONTEXT + 24, 10.0);
        float(machine.engine, CONTEXT + 32, 10.0);
        for (axis, margin) in self.context_margins.into_iter().enumerate() {
            float(machine.engine, CONTEXT + 40 + axis as u64 * 4, margin);
        }
        float(machine.engine, SPAN, 17.0);
        word(machine.engine, SPAN + 8, self.background);
        word(machine.engine, SPAN + 12, self.composing);
        write(
            machine.engine,
            SPAN + 40,
            &[if self.object { 2 } else { 0 }],
        );
        word(machine.engine, SPAN + 48, self.width_bits);
        word(machine.engine, SPAN + 52, self.height_bits);
        for (axis, margin) in self.stored_margins.into_iter().enumerate() {
            float(machine.engine, SPAN + 56 + axis as u64 * 4, margin);
        }
        write(machine.engine, SPAN + 64, &[u8::from(self.inline)]);
        write(machine.engine, MEASURE + 72, &[1]);
        float(machine.engine, MEASURE + 36, 1_000_000.0);
        float(machine.engine, RICH_PARAGRAPH + 36, 1.35);
        word(machine.engine, RICH_PARAGRAPH + 28, self.alignment);
        word(machine.engine, PARAGRAPH + 132, 1);
        word(machine.engine, DRAWING + 88, 1);
        write(
            machine.engine,
            RICH_IMPL + 220,
            &[u8::from(self.background_enabled)],
        );
        let recorder = Recorder::new(machine);
        machine.call(TEXT + 0x65920, &[ENTRY]);
        float(machine.engine, ENTRY + 60, 17.0);
        machine.call(TEXT + 0x779d0, &[FUNCTOR, SPAN, 0]);
        let measured = [
            read_float(machine.engine, ENTRY),
            read_float(machine.engine, ENTRY + 4),
        ];
        let local = rect(machine.engine, ENTRY + 32);
        let kind = read_u32(machine.engine, ENTRY + 48);
        let object_type = read_u32(machine.engine, ENTRY + 64);
        float(machine.engine, LINE + 24, 17.0);
        float(machine.engine, LINE + 28, measured[1]);
        float(machine.engine, LINE + 32, self.line_top_margin);
        float(machine.engine, LINE + 36, measured[1]);
        write_rect(
            machine.engine,
            BLOCK + 8,
            [0.0, 0.0, measured[0], measured[1]],
        );
        write_rect(machine.engine, BLOCK + 24, [4.25, 0.0, 104.25, measured[1]]);
        write(machine.engine, BLOCK + 41, &[u8::from(kind == 5)]);
        for (axis, margin) in self.stored_margins.into_iter().enumerate() {
            float(machine.engine, BLOCK + 48 + axis as u64 * 4, margin);
        }
        register(machine.engine, 136, u64::from(self.cursor.to_bits()));
        machine.call(TEXT + 0x6b4a4, &[PARAGRAPH, LINE]);
        let cursor = f32::from_bits(read_register(machine.engine, 136) as u32);
        assert!(cursor.is_finite());
        let position = [
            read_float(machine.engine, ENTRY + 8),
            read_float(machine.engine, ENTRY + 12),
        ];
        let layout = rect(machine.engine, ENTRY + 16);
        let placed_object = rect(machine.engine, ENTRY + 32);
        for (axis, offset) in self.offset.into_iter().enumerate() {
            register(
                machine.engine,
                136 + axis as i32,
                u64::from(offset.to_bits()),
            );
        }
        machine.call(TEXT + 0x64f18, &[DRAWING, TEXT_CANVAS, 0, 0]);
        let observation = recorder.observation.as_ref();
        assert_eq!(observation.constructors, observation.destructors);
        format!(
            "{{\"name\":{:?},\"inline\":{},\"object\":{},\"width_bits\":{},\"height_bits\":{},\"font_metric\":17,\"context_width_left_right\":[120,10,10],\"context_margins\":{:?},\"stored_margins\":{:?},\"mapped_background\":{},\"mapped_composing_background\":{},\"background_enabled\":{},\"offset\":{:?},\"initial_cursor\":{:?},\"line_top_margin\":{:?},\"alignment\":{},\"measured_advance_height\":{measured:?},\"local_object_rect\":{local:?},\"entry_kind\":{kind},\"object_type\":{object_type},\"post_cursor\":{cursor:?},\"position\":{position:?},\"layout_rect\":{layout:?},\"placed_object_rect\":{placed_object:?},\"paint_constructors\":{},\"paint_destructors\":{},\"commands\":[{}]}}",
            self.name,
            self.inline,
            self.object,
            self.width_bits,
            self.height_bits,
            self.context_margins,
            self.stored_margins,
            self.background,
            self.composing,
            self.background_enabled,
            self.offset,
            self.cursor,
            self.line_top_margin,
            self.alignment,
            observation.constructors,
            observation.destructors,
            observation.commands.join(",")
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base: &Path, text: &Path) {
    map_library(machine.engine, base, BASE, BASE_SHA256);
    map_library(machine.engine, text, TEXT, TEXT_SHA256);
    check(unsafe { uc_mem_map(machine.engine, HOST & !0xfff, 0x1000, 7) });
    for (plt, target) in [
        (0xef860, TEXT + 0x6cb0c),
        (0xef810, TEXT + 0x6c7ec),
        (0xef940, TEXT + 0x8e100),
        (0xef7e0, TEXT + 0x6c61c),
        (0xeed50, BASE + 0xb10e0),
        (0xeee10, BASE + 0xb108c),
        (0xeee20, BASE + 0xb109c),
        (0xef000, BASE + 0xb11a4),
        (0xef4b0, BASE + 0xb1538),
        (0xeeef0, TEXT + 0x61f3c),
        (0xf0970, COPY),
        (0xef220, CONSTRUCT_PAINT),
    ] {
        bind_native(machine.engine, TEXT + plt, target);
    }
    let mut cases = Vec::new();
    for inline in [false, true] {
        for (name, context_margins, stored_margins) in [
            ("frame", [0.0; 4], [0.0; 2]),
            ("body", [4.0, 0.0, 4.0, 0.0], [0.0; 2]),
            ("fractional-margins", [0.25, 1.5, 0.75, 2.25], [0.0; 2]),
            ("block-margins", [0.0; 4], [30.0, 30.0]),
        ] {
            cases.push(Case {
                name: format!("{name}-inline-{inline}"),
                inline,
                context_margins,
                stored_margins,
                line_top_margin: stored_margins[0],
                ..Case::default()
            });
        }
        for (name, width_bits, height_bits) in [
            ("nan-width", 0x7fc00001, 40.0_f32.to_bits()),
            ("nan-height", 30.0_f32.to_bits(), 0x7fc00001),
            ("nan-both", 0x7fc00001, 0x7fc00002),
            ("zero", 0, 0),
            ("fractional", 17.25_f32.to_bits(), 5.5_f32.to_bits()),
        ] {
            cases.push(Case {
                name: format!("{name}-inline-{inline}"),
                inline,
                width_bits,
                height_bits,
                ..Case::default()
            });
        }
        for (name, background, composing) in [
            ("no-color", 0, 0),
            ("ordinary", 0xff123456, 0),
            ("composing", 0, 0x80654321),
            ("composing-wins", 0xff123456, 0x80654321),
            ("mapped-zero-fallback", 0xff123456, 0),
            ("alpha-zero-nonzero", 0x00123456, 0),
            ("alpha-zero-composing", 0xff123456, 0x00654321),
        ] {
            cases.push(Case {
                name: format!("{name}-inline-{inline}"),
                inline,
                background,
                composing,
                ..Case::default()
            });
        }
    }
    cases.push(Case {
        name: "background-disabled".into(),
        background_enabled: false,
        ..Case::default()
    });
    cases.push(Case {
        name: "object-flag-disabled".into(),
        object: false,
        ..Case::default()
    });
    cases.push(Case {
        name: "negative-draw-offset".into(),
        offset: [-10.25, -12.5],
        ..Case::default()
    });
    cases.push(Case {
        name: "line-top-margin".into(),
        line_top_margin: 4.75,
        ..Case::default()
    });
    cases.push(Case {
        name: "center-alignment".into(),
        alignment: 2,
        ..Case::default()
    });
    cases.push(Case {
        name: "right-alignment".into(),
        alignment: 1,
        ..Case::default()
    });
    cases.push(Case {
        name: "mapped-white-ordinary".into(),
        background: 0x00ff_ffff,
        ..Case::default()
    });
    cases.push(Case {
        name: "mapped-white-composing".into(),
        composing: 0x00ff_ffff,
        ..Case::default()
    });
    let captures: Vec<_> = cases
        .into_iter()
        .map(|case| {
            let expected = case.fixture(machine, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(
                    case.fixture(machine, fill),
                    expected,
                    "{} memory fill",
                    case.name
                );
            }
            expected
        })
        .collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"base_library_sha256\":\"{BASE_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"memory_fills\":[0,165,255],\"entry_constructor\":\"0x65920\",\"measure_object_span\":\"0x779d0\",\"set_layout\":\"0x6b4a4\",\"draw_background\":\"0x64f18\",\"capture_boundary\":\"Native entry constructor, complete measureObjectSpan, SetLayout/GetBaseline, GetSpan and drawBackgroundColor execute unchanged, with actual Base rectangle helpers. Object dimensions, margins, font metric17, layout context width/margins, mapped retained span colors, draw offsets and a single-entry logical map are supplied. Line/block metrics are explicitly supplied using native measured advance/height plus the supplied font metric and margins; GetBlockInfo, wrapping and obstacle selection do not execute. Paint construction/style/color/destruction, memory copy and canvas rectangle recording are host supplied. No native font resolution, shaping, Widget object conversion, theme mapping, glyphless retained-run emission, Composer/PDF background policy or pixels execute.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
