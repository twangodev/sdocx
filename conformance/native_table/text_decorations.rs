use super::*;
use frames::{BASE, BASE_SHA256};
use geometry::TEXT_SHA256;

const TEXT: u64 = 0x0500_0000;
const DRAWING: u64 = MODEL + 0x18000;
const RICH_TEXT: u64 = DRAWING + 0x300;
const RICH_IMPL: u64 = DRAWING + 0x400;
const SPAN: u64 = DRAWING + 0x600;
const ENTRIES: u64 = DRAWING + 0x700;
const PAINT: u64 = DRAWING + 496;
const PAINT_IMPL: u64 = DRAWING + 0x900;
const COPY_IMPL: u64 = DRAWING + 0xa00;
const PAINT_VTABLE: u64 = DRAWING + 0xb00;
const TEXT_CANVAS: u64 = DRAWING + 0xd00;
const CANVAS_IMPL: u64 = DRAWING + 0xe00;
const CANVAS_VTABLE: u64 = DRAWING + 0xf00;
const RESOLVED_SPAN: u64 = DRAWING + 0x1100;
const HOST: u64 = 0x0300_1000;
const COPY_BYTES: u64 = HOST;
const COPY_PAINT: u64 = HOST + 32;
const DRAW_RECT: u64 = HOST + 64;
const SETTER_BASE: u64 = HOST + 128;
const GET_UNDERLINE: u64 = HOST + 640;
const GET_STRIKE: u64 = HOST + 672;
const DESTROY_PAINT: u64 = HOST + 704;
const SET_PAINT: u64 = TEXT + 0x63b98;
const DECORATIONS: u64 = TEXT + 0x666d4;

fn pointer(engine: Engine, address: u64, value: u64) {
    write(engine, address, &value.to_le_bytes());
}

fn word(engine: Engine, address: u64, value: u32) {
    write(engine, address, &value.to_le_bytes());
}

fn float(engine: Engine, address: u64, value: f32) {
    assert!(value.is_finite());
    write(engine, address, &value.to_le_bytes());
}

fn copy_bytes(engine: Engine, destination: u64, source: u64, count: usize) {
    assert!(count <= 144);
    let mut bytes = vec![0; count];
    check(unsafe { uc_mem_read(engine, source, bytes.as_mut_ptr().cast(), count) });
    write(engine, destination, &bytes);
}

#[derive(Default)]
struct Observation {
    commands: Vec<String>,
    paint_copies: usize,
    paint_deletes: usize,
}

unsafe extern "C" fn interface(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    let object = read_register(engine, REGISTER_X0);
    let argument = read_register(engine, REGISTER_X0 + 1);
    match address {
        COPY_BYTES => {
            copy_bytes(
                engine,
                object,
                argument,
                usize::try_from(read_register(engine, REGISTER_X0 + 2)).unwrap(),
            );
        }
        COPY_PAINT => {
            assert_eq!(argument, PAINT);
            pointer(engine, object, 0);
            pointer(engine, object + 8, COPY_IMPL);
            copy_bytes(engine, COPY_IMPL, PAINT_IMPL, 40);
            observation.paint_copies += 1;
        }
        DRAW_RECT => {
            assert_eq!(object, CANVAS_IMPL);
            let rect = std::array::from_fn::<_, 4, _>(|axis| {
                read_float(engine, argument + axis as u64 * 4)
            });
            let paint = read_register(engine, REGISTER_X0 + 2);
            let implementation = read_u64(engine, paint + 8);
            assert!([PAINT_IMPL, COPY_IMPL].contains(&implementation));
            observation.commands.push(format!(
                "{{\"rect\":{rect:?},\"rect_bits\":{:?},\"color\":{},\"alpha\":{},\"copied_paint\":{}}}",
                rect.map(f32::to_bits),
                read_u32(engine, implementation + 8),
                read_u32(engine, implementation + 12),
                implementation == COPY_IMPL
            ));
        }
        GET_UNDERLINE => {
            register(
                engine,
                REGISTER_X0,
                u64::from(read_u32(engine, object + 16)),
            );
        }
        GET_STRIKE => {
            register(
                engine,
                REGISTER_X0,
                u64::from(read_u32(engine, object + 20)),
            );
        }
        DESTROY_PAINT => {
            assert_eq!(object, COPY_IMPL);
            observation.paint_deletes += 1;
        }
        _ if (SETTER_BASE..SETTER_BASE + 13 * 32).contains(&address) => {
            assert!([PAINT_IMPL, COPY_IMPL].contains(&object));
            match (address - SETTER_BASE) / 32 {
                1 => word(engine, object + 24, read_register(engine, 136) as u32),
                2 => word(engine, object + 28, read_register(engine, 136) as u32),
                4 => word(engine, object + 16, argument as u32),
                5 => word(engine, object + 32, argument as u32),
                6 => word(engine, object + 20, argument as u32),
                8 => word(engine, object + 8, argument as u32),
                9 => word(engine, object + 12, argument as u32),
                _ => {}
            }
        }
        _ => unreachable!("interface {address:x}"),
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
            COPY_BYTES,
            COPY_PAINT,
            DRAW_RECT,
            GET_UNDERLINE,
            GET_STRIKE,
            DESTROY_PAINT,
        ]
        .into_iter()
        .chain((0..13).map(|index| SETTER_BASE + index * 32))
        {
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
struct FontUnits {
    screen_unit: u32,
    delta: f32,
    document_pixel: f32,
    density: f32,
    scaled_density: f32,
}

impl Default for FontUnits {
    fn default() -> Self {
        Self {
            screen_unit: 0,
            delta: 0.0,
            document_pixel: 1.0,
            density: 1.0,
            scaled_density: 1.0,
        }
    }
}

impl FontUnits {
    fn json(&self) -> String {
        format!(
            "{{\"screen_unit\":{},\"font_size_delta\":{:?},\"document_pixel\":{:?},\"density\":{:?},\"scaled_density\":{:?}}}",
            self.screen_unit, self.delta, self.document_pixel, self.density, self.scaled_density
        )
    }
}

#[derive(Clone)]
struct Case {
    name: String,
    style: u8,
    suggestion_enabled: bool,
    link: bool,
    links_enabled: bool,
    correction_foreground: bool,
    forced_strike: bool,
    opacity: f32,
    foreground: u32,
    underline: u32,
    size: f32,
    units: FontUnits,
    rich_impl: bool,
    offset: [f32; 2],
    entries: [[f32; 3]; 2],
    direction: u32,
}

impl Default for Case {
    fn default() -> Self {
        Self {
            name: String::new(),
            style: 0x10,
            suggestion_enabled: true,
            link: false,
            links_enabled: false,
            correction_foreground: false,
            forced_strike: false,
            opacity: 1.0,
            foreground: 0x8012_3456,
            underline: 0x4065_4321,
            size: 20.0,
            units: FontUnits::default(),
            rich_impl: true,
            offset: [4.25, 12.5],
            entries: [[10.0, 2.0, 30.0], [12.0, 12.0, 30.0]],
            direction: 0,
        }
    }
}

impl Case {
    fn fixture(&self, machine: &Machine, fill: u8) -> String {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        for (address, size) in [
            (DRAWING, 520),
            (RICH_TEXT, 8),
            (RICH_IMPL, 240),
            (SPAN, 144),
            (ENTRIES, 160),
            (PAINT_IMPL, 40),
            (COPY_IMPL, 40),
            (PAINT_VTABLE, 264),
            (TEXT_CANVAS, 16),
            (CANVAS_IMPL, 16),
            (CANVAS_VTABLE, 104),
            (RESOLVED_SPAN, 72),
        ] {
            write(machine.engine, address, &vec![0; size]);
        }
        for (address, value) in [
            (DRAWING + 64, RICH_TEXT),
            (DRAWING + 96, ENTRIES),
            (RICH_TEXT, if self.rich_impl { RICH_IMPL } else { 0 }),
            (RICH_IMPL + 8, SPAN),
            (RICH_IMPL + 16, SPAN + 144),
            (RICH_IMPL + 24, SPAN + 144),
            (PAINT + 8, PAINT_IMPL),
            (PAINT_IMPL, PAINT_VTABLE),
            (TEXT_CANVAS + 8, CANVAS_IMPL),
            (CANVAS_IMPL, CANVAS_VTABLE),
            (CANVAS_VTABLE + 96, DRAW_RECT),
            (PAINT_VTABLE + 8, DESTROY_PAINT),
            (PAINT_VTABLE + 224, GET_UNDERLINE),
            (PAINT_VTABLE + 232, GET_STRIKE),
        ] {
            pointer(machine.engine, address, value);
        }
        for (index, slot) in [16, 24, 32, 40, 72, 80, 88, 104, 120, 128, 112, 96, 64]
            .into_iter()
            .enumerate()
        {
            pointer(
                machine.engine,
                PAINT_VTABLE + slot,
                SETTER_BASE + index as u64 * 32,
            );
        }
        word(machine.engine, DRAWING + 88, 2);
        write(
            machine.engine,
            DRAWING + 512,
            &[u8::from(self.suggestion_enabled)],
        );
        write(
            machine.engine,
            RICH_IMPL + 112,
            &[u8::from(self.links_enabled)],
        );
        word(machine.engine, RICH_IMPL + 188, self.units.screen_unit);
        for (address, value) in [
            (172, self.units.delta),
            (192, self.units.document_pixel),
            (196, self.units.density),
            (200, self.units.scaled_density),
        ] {
            float(machine.engine, RICH_IMPL + address, value);
        }
        float(machine.engine, SPAN, self.size);
        word(machine.engine, SPAN + 4, self.foreground);
        word(machine.engine, SPAN + 32, self.underline);
        word(machine.engine, SPAN + 36, 0xc0ab_cdef);
        write(machine.engine, SPAN + 16, &[self.style]);
        write(machine.engine, SPAN + 40, &[u8::from(self.link)]);
        write(
            machine.engine,
            SPAN + 66,
            &[u8::from(self.correction_foreground)],
        );
        copy_bytes(machine.engine, SPAN + 72, SPAN, 72);
        for (index, values) in self.entries.iter().enumerate() {
            let entry = ENTRIES + index as u64 * 80;
            for (offset, value) in [0, 8, 12].into_iter().zip(values) {
                float(machine.engine, entry + offset, *value);
            }
            word(machine.engine, entry + 52, self.direction);
        }
        let recorder = Recorder::new(machine);
        register(machine.engine, 136, u64::from(self.opacity.to_bits()));
        machine.call(
            SET_PAINT,
            &[DRAWING, PAINT, SPAN, 0, u64::from(self.forced_strike), 0],
        );
        let paint = [
            read_u32(machine.engine, PAINT_IMPL + 8),
            read_u32(machine.engine, PAINT_IMPL + 12),
            read_u32(machine.engine, PAINT_IMPL + 16),
            read_u32(machine.engine, PAINT_IMPL + 20),
        ];
        register(machine.engine, REGISTER_X0 + 8, RESOLVED_SPAN);
        machine.call(TEXT + 0x61f3c, &[RICH_TEXT, 0]);
        let resolved_size = read_float(machine.engine, RESOLVED_SPAN);
        let resolved_style = read_u32(machine.engine, RESOLVED_SPAN + 16) & 0xff;
        for (axis, value) in self.offset.into_iter().enumerate() {
            register(
                machine.engine,
                136 + axis as i32,
                u64::from(value.to_bits()),
            );
        }
        machine.call(DECORATIONS, &[DRAWING, TEXT_CANVAS, 0, 1]);
        let observation = recorder.observation.as_ref();
        assert_eq!(observation.paint_copies, observation.paint_deletes);
        format!(
            "{{\"name\":{:?},\"style\":{},\"suggestion_enabled\":{},\"link\":{},\"links_enabled\":{},\"correction_foreground\":{},\"forced_strike\":{},\"opacity\":{:?},\"foreground\":{},\"underline\":{},\"font_size\":{:?},\"font_units\":{},\"rich_impl\":{},\"offset\":{:?},\"entries_advance_x_y\":{:?},\"direction\":{},\"resolved_decoration_span_size\":{resolved_size:?},\"resolved_decoration_span_style\":{resolved_style},\"paint_color_alpha_underline_strike\":{paint:?},\"paint_copies\":{},\"paint_deletes\":{},\"commands\":[{}]}}",
            self.name,
            self.style,
            self.suggestion_enabled,
            self.link,
            self.links_enabled,
            self.correction_foreground,
            self.forced_strike,
            self.opacity,
            self.foreground,
            self.underline,
            self.size,
            self.units.json(),
            self.rich_impl,
            self.offset,
            self.entries,
            self.direction,
            observation.paint_copies,
            observation.paint_deletes,
            observation.commands.join(",")
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base: &Path, text: &Path) {
    frames::load_base(machine, base);
    map_library(machine.engine, text, TEXT, TEXT_SHA256);
    check(unsafe { uc_mem_map(machine.engine, HOST, 0x1000, 7) });
    for (plt, target) in [
        (0xeeef0, TEXT + 0x61f3c),
        (0xeed50, BASE + 0xb10e0),
        (0xf0970, COPY_BYTES),
        (0xeee40, COPY_PAINT),
    ] {
        bind_native(machine.engine, TEXT + plt, target);
    }
    let mut cases = Vec::new();
    for style in 0..64 {
        for enabled in [false, true] {
            cases.push(Case {
                name: format!("style-{style:02x}-enabled-{enabled}"),
                style,
                suggestion_enabled: enabled,
                ..Case::default()
            });
        }
    }
    for style in [0, 4, 8, 0x10, 0x20, 0x3c] {
        for link in [false, true] {
            for enabled in [false, true] {
                for foreground in [false, true] {
                    cases.push(Case {
                        name: format!("gates-{style:02x}-{link}-{enabled}-{foreground}"),
                        style,
                        link,
                        links_enabled: enabled,
                        correction_foreground: foreground,
                        ..Case::default()
                    });
                }
            }
        }
    }
    for style in [4, 8, 0x10, 0x20, 0x3c] {
        for size in [-20.0, 0.0, 0.5, 17.25, 144.0] {
            for screen_unit in 0..5 {
                cases.push(Case {
                    name: format!("geometry-{style:02x}-{size}-{screen_unit}"),
                    style,
                    size,
                    units: FontUnits {
                        screen_unit,
                        delta: 3.5,
                        document_pixel: 0.25,
                        density: 2.0,
                        scaled_density: -0.5,
                    },
                    ..Case::default()
                });
            }
        }
    }
    for style in [4, 8, 0x10, 0x20, 0x3c] {
        for opacity in [0.0, 0.25, 0.5, 1.0] {
            for alpha in [0, 0x40, 0xff] {
                cases.push(Case {
                    name: format!("alpha-{style:02x}-{opacity}-{alpha}"),
                    style,
                    opacity,
                    foreground: (alpha << 24) | 0x123456,
                    underline: (alpha << 24) | 0x654321,
                    ..Case::default()
                });
            }
        }
    }
    for (name, entries, direction) in [
        ("rtl-positive", [[10.0, 24.0, 30.0], [12.0, 12.0, 30.0]], 1),
        (
            "rtl-negative-advance",
            [[-10.0, 24.0, 30.0], [-12.0, 12.0, 30.0]],
            1,
        ),
        ("reversed-ltr", [[10.0, 30.0, 30.0], [12.0, 2.0, 30.0]], 0),
        (
            "different-baselines",
            [[10.0, 2.0, 30.0], [12.0, 12.0, -80.0]],
            0,
        ),
        (
            "fractional",
            [[0.125, -4.25, 0.75], [0.375, -4.125, 0.75]],
            0,
        ),
    ] {
        cases.push(Case {
            name: name.into(),
            style: 0x3c,
            entries,
            direction,
            ..Case::default()
        });
    }
    cases.push(Case {
        name: "forced-strike".into(),
        style: 0,
        forced_strike: true,
        ..Case::default()
    });
    cases.push(Case {
        name: "null-rich-implementation".into(),
        rich_impl: false,
        style: 0x3c,
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
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"base_library_sha256\":\"{BASE_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"memory_fills\":[0,165,255],\"set_text_paint\":\"0x63b98\",\"draw_text_decorations\":\"0x666d4\",\"get_span\":\"0x61f3c\",\"capture_boundary\":\"Complete native setTextPaint, drawTextDecorations, GetSpan and default-entry initialization execute; Base RectF::Set executes. Retained span, rich-text font-unit/link configuration, entry positions/advances/direction and opacity inputs are supplied. Paint interface setters/getters, paint copy/destruction, memory copy and canvas rectangle dispatch are host supplied. Captures contain native rectangle commands and paint color/alpha arguments; no SkCanvas pixels, font selection, shaping, theme conversion or full drawTextRun execute. Endpoint direction is supplied but drawTextDecorations does not branch on it.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
