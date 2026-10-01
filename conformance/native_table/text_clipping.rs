use super::*;
use frames::{BASE, BASE_SHA256};

const COMPOSER: u64 = 0x0600_0000;
const COMPOSER_SHA256: &str = "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f";
const PDF: u64 = 0x0700_0000;
const PDF_SHA256: &str = "cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04";
const SOURCE: u64 = MODEL + 0x9000;
const SOURCE_VTABLE: u64 = MODEL + 0xa000;
const SOURCE_IMPL: u64 = MODEL + 0xa800;
const SOURCE_DATA: u64 = MODEL + 0xa900;
const RUN: u64 = MODEL + 0xb000;
const WRITER: u64 = MODEL + 0xc000;
const PDF_WRITER: u64 = MODEL + 0xc100;
const PDF_WRITER_VTABLE: u64 = MODEL + 0xc200;
const PAINT: u64 = MODEL + 0xc300;
const PAINT_VTABLE: u64 = MODEL + 0xc400;
const PAGE_RECT: u64 = 0x0300_0300;
const TRANSLATE: u64 = 0x0300_0320;
const CLIP_START: u64 = COMPOSER + 0x37f6ac;
const CLIP_END: u64 = COMPOSER + 0x37f7b0;
const PDF_GATE: u64 = PDF + 0xa23d4;
const PDF_CLIP: u64 = PDF + 0xa23e4;
const PDF_NO_CLIP: u64 = PDF + 0xa2474;

#[derive(Default, Debug, PartialEq)]
struct Observation {
    selected: bool,
    world_clip: Option<[f32; 4]>,
    translation: Option<[f32; 2]>,
    backend_clip: bool,
}

fn rectangle(engine: Engine, address: u64) -> [f32; 4] {
    std::array::from_fn(|axis| read_float(engine, address + axis as u64 * 4))
}

fn write_rectangle(engine: Engine, address: u64, bounds: [f32; 4]) {
    for (axis, coordinate) in bounds.into_iter().enumerate() {
        assert!(coordinate.is_finite());
        write(engine, address + axis as u64 * 4, &coordinate.to_le_bytes());
    }
}

unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    match address {
        PAGE_RECT => {
            for (axis, coordinate) in [0.0_f32, 0.0, 600.0, 800.0].into_iter().enumerate() {
                register(engine, 136 + axis as i32, u64::from(coordinate.to_bits()));
            }
        }
        TRANSLATE => {
            assert_eq!(read_register(engine, REGISTER_X0), PAINT);
            observation.translation = Some(std::array::from_fn(|axis| {
                f32::from_bits(read_register(engine, 136 + axis as i32) as u32)
            }));
        }
        address if address == COMPOSER + 0x37f6e8 => observation.selected = true,
        address if address == COMPOSER + 0x37f724 => {
            observation.world_clip = Some(rectangle(engine, STACK + 32));
        }
        PDF_CLIP | PDF_NO_CLIP => {
            observation.backend_clip = address == PDF_CLIP;
            register(engine, 260, STOP);
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
        for address in [PAGE_RECT, TRANSLATE] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
        }
        for address in [
            PAGE_RECT,
            TRANSLATE,
            COMPOSER + 0x37f6e8,
            COMPOSER + 0x37f724,
            PDF_CLIP,
            PDF_NO_CLIP,
        ] {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    observe as *mut c_void,
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

struct Case {
    name: String,
    source: [f32; 4],
    run: [f32; 4],
    origin: [f32; 2],
    scale: f32,
}

fn optional<const N: usize>(value: Option<[f32; N]>) -> String {
    value.map_or_else(|| "null".into(), |value| format!("{value:?}"))
}

impl Case {
    fn fixture(&self, machine: &Machine, recorder: &mut Recorder, fill: u8) -> String {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        *recorder.observation = Observation::default();
        for (address, value) in [
            (SOURCE, SOURCE_VTABLE),
            (SOURCE + 16, SOURCE_IMPL),
            (SOURCE_IMPL + 24, SOURCE_DATA),
            (SOURCE_VTABLE + 168, 0x2caa60),
            (WRITER + 32, PDF_WRITER),
            (PDF_WRITER, PDF_WRITER_VTABLE),
            (PDF_WRITER_VTABLE + 48, PAGE_RECT),
            (PAINT, PAINT_VTABLE),
            (PAINT_VTABLE + 272, TRANSLATE),
        ] {
            write(machine.engine, address, &value.to_le_bytes());
        }
        write_rectangle(machine.engine, SOURCE_DATA + 8, self.source);
        write_rectangle(machine.engine, RUN + 104, self.run);
        write(machine.engine, WRITER + 8, &self.scale.to_le_bytes());
        for (index, value) in [
            (19, WRITER),
            (20, PAINT),
            (21, RUN),
            (22, WRITER + 8),
            (23, SOURCE),
        ] {
            register(machine.engine, REGISTER_X0 + index, value);
        }
        for (index, value) in [(10, self.origin[1]), (11, self.origin[0])] {
            register(machine.engine, 136 + index, u64::from(value.to_bits()));
        }
        register(machine.engine, REGISTER_SP, STACK);
        register(machine.engine, REGISTER_X30, STOP);
        check(unsafe { uc_emu_start(machine.engine, CLIP_START, CLIP_END, 1_000_000, 1000) });
        assert_eq!(read_register(machine.engine, 260), CLIP_END);
        let explicit_clip = rectangle(machine.engine, STACK + 32);
        assert_eq!(rectangle(machine.engine, SOURCE_DATA + 8), self.source);
        assert_eq!(rectangle(machine.engine, STACK + 48), self.run);
        register(machine.engine, REGISTER_X0 + 24, STACK + 32);
        check(unsafe { uc_emu_start(machine.engine, PDF_GATE, STOP, 1_000_000, 1000) });
        assert_eq!(read_register(machine.engine, 260), STOP);
        let observation = recorder.observation.as_ref();
        assert_eq!(observation.selected, observation.world_clip.is_some());
        assert_eq!(observation.selected, observation.translation.is_some());
        format!(
            "{{\"name\":{:?},\"source_bbox\":{:?},\"run_bbox\":{:?},\"cell_origin\":{:?},\"pdf_scale\":{:?},\"selected\":{},\"world_clip\":{},\"pdf_clip\":{explicit_clip:?},\"paint_translation\":{},\"backend_clip\":{}}}",
            self.name,
            self.source,
            self.run,
            self.origin,
            self.scale,
            observation.selected,
            optional(observation.world_clip),
            optional(observation.translation),
            observation.backend_clip,
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base: &Path, composer: &Path, pdf: &Path) {
    frames::load_base(machine, base);
    map_library(machine.engine, composer, COMPOSER, COMPOSER_SHA256);
    map_library(machine.engine, pdf, PDF, PDF_SHA256);
    for (plt, target) in [
        (COMPOSER + 0x54db40, BASE + 0xb109c),
        (COMPOSER + 0x54f2e0, BASE + 0xb11a4),
        (COMPOSER + 0x54f2d0, BASE + 0xb1350),
        (COMPOSER + 0x54f390, BASE + 0xb1510),
        (COMPOSER + 0x54f520, BASE + 0xb1808),
        (COMPOSER + 0x5531a0, BASE + 0xb0fb0),
        (PDF + 0xaaaf0, BASE + 0xb11bc),
    ] {
        bind_native(machine.engine, plt, target);
    }
    let mut recorder = Recorder::new(machine);
    let mut cases = Vec::new();
    for (name, run) in [
        ("inside", [2.0, 3.0, 12.0, 15.0]),
        (
            "below-height",
            [2.0, 3.0, 12.0, f32::from_bits(20.0_f32.to_bits() - 1)],
        ),
        ("equal-height", [2.0, 3.0, 12.0, 20.0]),
        (
            "above-height",
            [2.0, 3.0, 12.0, f32::from_bits(20.0_f32.to_bits() + 1)],
        ),
        ("overflow", [2.0, 15.0, 12.0, 25.0]),
        ("outside-horizontal", [40.0, 15.0, 50.0, 25.0]),
        ("outside-vertical", [2.0, 30.0, 12.0, 40.0]),
        ("touch-bottom", [2.0, 20.0, 12.0, 25.0]),
        ("touch-right", [30.0, 15.0, 40.0, 25.0]),
        ("negative-top", [-5.0, -10.0, 35.0, 25.0]),
        ("zero-width", [2.0, 15.0, 2.0, 25.0]),
        ("zero-height", [2.0, 25.0, 12.0, 25.0]),
        ("inverted-width", [12.0, 15.0, 2.0, 25.0]),
        ("inverted-height", [2.0, 35.0, 12.0, 25.0]),
    ] {
        for origin in [[0.0, 0.0], [100.25, 200.75], [-20.5, -30.25]] {
            for scale in [1.0, 0.75, 1.0 / 1.8] {
                cases.push(Case {
                    name: format!("{name}-{origin:?}-{scale:?}"),
                    source: [origin[0], origin[1], origin[0] + 30.0, origin[1] + 20.0],
                    run,
                    origin,
                    scale,
                });
            }
        }
    }
    for (name, source) in [
        ("fractional-height", [100.25, 200.75, 130.125, 220.875]),
        ("different-source-origin", [400.0, 500.0, 430.0, 520.0]),
        ("zero-source-height", [100.0, 200.0, 130.0, 200.0]),
        ("inverted-source-height", [100.0, 220.0, 130.0, 200.0]),
        ("zero-source-width", [100.0, 200.0, 100.0, 220.0]),
        ("inverted-source-width", [130.0, 200.0, 100.0, 220.0]),
    ] {
        cases.push(Case {
            name: name.into(),
            source,
            run: [2.0, 15.0, 12.0, 25.0],
            origin: [100.0, 200.0],
            scale: 0.75,
        });
    }
    let captures = cases
        .iter()
        .map(|case| {
            let expected = case.fixture(machine, &mut recorder, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(
                    case.fixture(machine, &mut recorder, fill),
                    expected,
                    "{} allocation fill",
                    case.name
                );
            }
            expected
        })
        .collect::<Vec<_>>();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"memory_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"composer_library_sha256\":\"{COMPOSER_SHA256}\",\"pdf_library_sha256\":\"{PDF_SHA256}\",\"clip_start\":\"0x37f6ac\",\"clip_end\":\"0x37f7b0\",\"pdf_gate\":\"0xa23d4\",\"measurement_inputs\":\"Supplied source object rectangles, DrawnText run rectangles, rounded cell origins and PDF scale. Unmodified Composer clip instructions, Model GetRect and Base rectangle/point helpers execute. The Pdfium text handler executes its empty-clip gate. PDF page bounds [0,0,600,800] and paint translation recording are host interfaces. Shaping, run-rectangle production, model rectangle producers, font objects and final PDF paths/pixels are not executed.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n"),
    );
}
