use super::*;
use frames::{BASE, BASE_SHA256};
use geometry::TEXT_SHA256;

unsafe extern "C" {
    fn uc_mem_unmap(engine: Engine, address: u64, size: u64) -> i32;
}

const TEXT: u64 = 0x0500_0000;
const MEASURE: u64 = MODEL + 0x18000;
const DRAWING: u64 = MEASURE + 0x100;
const RICH_TEXT: u64 = MEASURE + 0x200;
const RICH_IMPL: u64 = MEASURE + 0x300;
const SOURCE: u64 = MEASURE + 0x400;
const SOURCE_IMPL: u64 = MEASURE + 0x440;
const CHARACTERS: u64 = MEASURE + 0x480;
const SPAN: u64 = MEASURE + 0x500;
const FUNCTOR: u64 = MEASURE + 0x600;
const CONTEXT: u64 = MEASURE + 0x700;
const OUTPUT: u64 = MEASURE + 0x800;
const RETURN_GLYPHS: u64 = MEASURE + 0x900;
const COPY: u64 = 0x0300_2000;
const APPEND: u64 = TEXT + 0x67ebc;
const FIRST_WORD: u64 = TEXT + 0x68144;

fn pointer(engine: Engine, address: u64, value: u64) {
    write(engine, address, &value.to_le_bytes());
}

fn word(engine: Engine, address: u64, value: u32) {
    write(engine, address, &value.to_le_bytes());
}

fn vector(engine: Engine, address: u64) -> [u64; 3] {
    std::array::from_fn(|index| read_u64(engine, address + index as u64 * 8))
}

fn byte(engine: Engine, address: u64) -> u8 {
    let mut value = 0;
    check(unsafe { uc_mem_read(engine, address, ptr::from_mut(&mut value).cast(), 1) });
    value
}

#[derive(Default)]
struct Observation {
    append_inputs: Vec<String>,
    first_word_reads: usize,
    incomplete_records: Vec<String>,
}

unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    if address == COPY {
        let destination = read_register(engine, REGISTER_X0);
        let source = read_register(engine, REGISTER_X0 + 1);
        let count = usize::try_from(read_register(engine, REGISTER_X0 + 2)).unwrap();
        assert!(count <= 80);
        let mut bytes = vec![0; count];
        check(unsafe { uc_mem_read(engine, source, bytes.as_mut_ptr().cast(), count) });
        write(engine, destination, &bytes);
    } else if address == APPEND {
        let glyphs = vector(engine, read_register(engine, REGISTER_X0 + 3));
        let positions = vector(engine, read_register(engine, REGISTER_X0 + 4));
        let span = read_register(engine, REGISTER_X0 + 5);
        observation.append_inputs.push(format!(
            "{{\"range\":[{},{}],\"glyph_vector\":{glyphs:?},\"position_vector\":{positions:?},\"span_background\":{},\"span_composing\":{},\"span_flags\":{}}}",
            read_register(engine, REGISTER_X0 + 1) as i32,
            read_register(engine, REGISTER_X0 + 2) as i32,
            read_u32(engine, span + 8),
            read_u32(engine, span + 12),
            byte(engine, span + 40),
        ));
    } else {
        assert_eq!(address, FIRST_WORD);
        assert_eq!(read_register(engine, REGISTER_X0 + 9), 0);
        observation.first_word_reads += 1;
        let record = read_register(engine, REGISTER_X0 + 20);
        let glyph_rect: [f32; 4] =
            std::array::from_fn(|axis| read_float(engine, record + 88 + axis as u64 * 4));
        let layout_rect: [f32; 4] =
            std::array::from_fn(|axis| read_float(engine, record + 104 + axis as u64 * 4));
        observation.incomplete_records.push(format!(
            "{{\"glyph_rect\":{glyph_rect:?},\"layout_rect\":{layout_rect:?},\"span_bit1\":{},\"font_id\":{},\"background_before_first_word_load\":{}}}",
            byte(engine, record + 120), read_u32(engine, record + 124) as i32,
            read_u32(engine, record + 144),
        ));
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
        write(machine.engine, COPY, &0xd65f03c0_u32.to_le_bytes());
        for address in [COPY, APPEND, FIRST_WORD] {
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
    inline: bool,
    object: bool,
    dimensions: [u32; 2],
    background: u32,
    composing: u32,
    offset: [f32; 2],
    public_api: bool,
}

impl Case {
    fn fixture(&self, machine: &mut Machine, fill: u8) -> String {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        write(machine.engine, MEASURE, &vec![0; 0xa00]);
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        let recorder = Recorder::new(machine);
        machine.call(TEXT + 0x78090, &[MEASURE]);
        machine.call(TEXT + 0x790d0, &[MEASURE, 1]);
        let entries = read_u64(machine.engine, MEASURE + 24);
        let cache = read_u64(machine.engine, MEASURE + 48);
        assert_eq!(read_u64(machine.engine, MEASURE + 32) - entries, 80);
        assert_eq!(read_u64(machine.engine, MEASURE + 56) - cache, 40);
        let before_glyphs = vector(machine.engine, cache);
        let before_drawable = byte(machine.engine, cache + 34);
        for (address, value) in [
            (FUNCTOR + 16, CONTEXT),
            (CONTEXT, MEASURE + 24),
            (DRAWING + 64, RICH_TEXT),
            (DRAWING + 96, entries),
            (DRAWING + 120, cache),
            (RICH_TEXT, RICH_IMPL),
            (RICH_IMPL, SOURCE),
            (RICH_IMPL + 8, SPAN),
            (RICH_IMPL + 16, SPAN + 72),
            (RICH_IMPL + 24, SPAN + 72),
            (SOURCE + 8, SOURCE_IMPL),
            (SOURCE_IMPL + 16, CHARACTERS),
        ] {
            pointer(machine.engine, address, value);
        }
        word(machine.engine, MEASURE + 16, 1);
        word(machine.engine, DRAWING + 88, 1);
        write(machine.engine, CHARACTERS, &0xfffc_u16.to_le_bytes());
        word(machine.engine, SPAN, 17.0_f32.to_bits());
        word(machine.engine, SPAN + 4, 0xff12_3456);
        word(machine.engine, SPAN + 8, self.background);
        word(machine.engine, SPAN + 12, self.composing);
        write(
            machine.engine,
            SPAN + 40,
            &[if self.object { 2 } else { 0 }],
        );
        write(machine.engine, SPAN + 64, &[u8::from(self.inline)]);
        for (index, bits) in self.dimensions.into_iter().enumerate() {
            word(machine.engine, SPAN + 48 + index as u64 * 4, bits);
        }
        for (address, value) in [
            (CONTEXT + 20, 120.0_f32),
            (CONTEXT + 24, 10.0),
            (CONTEXT + 32, 10.0),
            (CONTEXT + 40, 0.25),
            (CONTEXT + 44, 1.5),
            (CONTEXT + 48, 0.75),
            (CONTEXT + 52, 2.25),
        ] {
            word(machine.engine, address, value.to_bits());
        }
        machine.call(TEXT + 0x779d0, &[FUNCTOR, SPAN, 0]);
        let kind = read_u32(machine.engine, entries + 48);
        let mode = read_u32(machine.engine, entries + 64);
        let advance = read_float(machine.engine, entries);
        let height = read_float(machine.engine, entries + 4);
        let ink: [f32; 4] =
            std::array::from_fn(|axis| read_float(machine.engine, entries + 32 + axis as u64 * 4));
        let produced_glyphs = vector(machine.engine, cache);
        let produced_drawable = byte(machine.engine, cache + 34);
        register(machine.engine, REGISTER_X0 + 8, RETURN_GLYPHS);
        machine.call(TEXT + 0x78274, &[MEASURE, 0]);
        let returned_glyphs = vector(machine.engine, RETURN_GLYPHS);
        let returned_drawable = byte(machine.engine, RETURN_GLYPHS + 34);
        for (axis, value) in self.offset.into_iter().enumerate() {
            check(unsafe {
                uc_reg_write(
                    machine.engine,
                    136 + axis as i32,
                    ptr::from_ref(&value).cast(),
                )
            });
        }
        for (index, value) in [DRAWING, 0, 0, OUTPUT, 0].into_iter().enumerate() {
            register(machine.engine, REGISTER_X0 + index as i32, value);
        }
        register(machine.engine, REGISTER_SP, STACK);
        register(machine.engine, REGISTER_X30, STOP);
        let error = unsafe {
            uc_emu_start(
                machine.engine,
                TEXT + if self.public_api { 0x68418 } else { 0x66c98 },
                STOP,
                1_000_000,
                1_000_000,
            )
        };
        let pc = read_register(machine.engine, 260);
        assert_eq!(error, 6, "expected unmapped read, PC {pc:x}");
        assert_eq!(pc, FIRST_WORD);
        assert_eq!(recorder.observation.first_word_reads, 1);
        assert_eq!(recorder.observation.append_inputs.len(), 1);
        let terminal = "empty-glyph-first-word-unmapped-read";
        let output = vector(machine.engine, OUTPUT);
        assert_eq!(output, [0; 3]);
        format!(
            "{{\"name\":{:?},\"inline\":{},\"object\":{},\"dimension_bits\":{:?},\"background\":{},\"composing\":{},\"offset\":{:?},\"public_get_drawn_text\":{},\"initialized_glyph_vector\":{before_glyphs:?},\"initialized_drawable\":{before_drawable},\"produced_entry\":{{\"kind\":{kind},\"object_mode\":{mode},\"advance\":{advance},\"height\":{height},\"ink\":{ink:?}}},\"produced_glyph_vector\":{produced_glyphs:?},\"produced_drawable\":{produced_drawable},\"returned_glyph_vector\":{returned_glyphs:?},\"returned_drawable\":{returned_drawable},\"append_inputs\":[{}],\"incomplete_records\":[{}],\"terminal\":{terminal:?},\"native_terminal_pc\":{},\"unicorn_error\":{error},\"output_vector\":{output:?}}}",
            self.name,
            self.inline,
            self.object,
            self.dimensions,
            self.background,
            self.composing,
            self.offset,
            self.public_api,
            recorder.observation.append_inputs.join(","),
            recorder.observation.incomplete_records.join(","),
            pc - TEXT,
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base: &Path, text: &Path) {
    map_library(machine.engine, base, BASE, BASE_SHA256);
    map_library(machine.engine, text, TEXT, TEXT_SHA256);
    check(unsafe { uc_mem_map(machine.engine, COPY, 0x1000, 7) });
    check(unsafe { uc_mem_unmap(machine.engine, 0, 0x1000) });
    for (plt, target) in [
        (0xeec30, NEW),
        (0xeebc0, NEW),
        (0xeec40, DELETE),
        (0xef2a0, TEXT + 0x78074),
        (0xef290, TEXT + 0x78274),
        (0xeeef0, TEXT + 0x61f3c),
        (0xef380, BASE + 0xc49cc),
        (0xef140, TEXT + 0x65998),
        (0xef1b0, APPEND),
        (0xef1a0, TEXT + 0x66c98),
        (0xef000, BASE + 0xb11a4),
        (0xef4b0, BASE + 0xb1538),
        (0xeed50, BASE + 0xb10e0),
        (0xf0970, COPY),
        (0xf08e0, COPY),
    ] {
        bind_native(machine.engine, TEXT + plt, target);
    }
    let mut cases = Vec::new();
    for inline in [false, true] {
        for (name, dimensions) in [
            ("body", [30.0_f32.to_bits(), 40.0_f32.to_bits()]),
            ("zero", [0, 0]),
            ("nan-width", [0x7fc00001, 40.0_f32.to_bits()]),
            ("nan-height", [30.0_f32.to_bits(), 0x7fc00001]),
        ] {
            for (color, background, composing) in [
                ("empty", 0, 0),
                ("background", 0x80123456, 0),
                ("composing", 0xff123456, 0x80654321),
            ] {
                cases.push(Case {
                    name: format!("{name}-{color}-inline-{inline}"),
                    inline,
                    object: true,
                    dimensions,
                    background,
                    composing,
                    offset: [4.25, 12.5],
                    public_api: false,
                });
            }
        }
    }
    cases.push(Case {
        name: "default-kind3-empty-run-control".into(),
        inline: false,
        object: false,
        dimensions: [30.0_f32.to_bits(), 40.0_f32.to_bits()],
        background: 0xff123456,
        composing: 0x80654321,
        offset: [0.0; 2],
        public_api: false,
    });
    cases.push(Case {
        name: "public-get-drawn-text-object".into(),
        inline: true,
        object: true,
        dimensions: [30.0_f32.to_bits(), 40.0_f32.to_bits()],
        background: 0xff123456,
        composing: 0x80654321,
        offset: [4.25, 12.5],
        public_api: true,
    });
    let captures: Vec<_> = cases
        .iter()
        .map(|case| {
            let initial = case.fixture(machine, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(
                    initial,
                    case.fixture(machine, fill),
                    "allocation fill changed {}",
                    case.name
                );
            }
            initial
        })
        .collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"memory_fills\":[0,165,255],\"guest_zero_page\":\"Model ELF header page explicitly unmapped\",\"measurement_context\":{{\"available_width\":120,\"left_reserved\":10,\"right_reserved\":10,\"object_margins\":[0.25,1.5,0.75,2.25]}},\"measure_constructor\":\"0x78090\",\"init_measure_data\":\"0x790d0\",\"measure_object_span\":\"0x779d0\",\"glyph_info\":\"0x78274\",\"get_drawn_text_run\":\"0x66c98\",\"public_get_drawn_text\":\"0x68418\",\"append_text_block\":\"0x67ebc\",\"first_word_load\":\"0x68144\",\"capture_bounds\":\"Actual native measurement constructor and initMeasureData allocate/default-initialize one entry and its GlyphInfo cache. Actual measureObjectSpan receives a supplied retained span, available width/margins and index. Actual GetGlyphInfo copies the native-produced empty cache. Whole getDrawnTextRun executes (one case enters through public GetDrawnText) with supplied single UTF-16 object replacement character, retained span and caller offsets, empty paragraph vector, null font and emoji slices. Native appendTextBlock is observed unchanged until its unconditional first glyph codeword read faults on the null vector. Guest address zero is unmapped; no glyph records or null-page contents are manufactured. Allocation, deletion and bounded memory copy/move are host supplied. Default layout rectangles remain unplaced; incomplete allocated record fields are observed before failure and are never published. Line/block placement, full upstream span conversion/measurement loop or outer GetDrawnTextData dispatch, font selection/shaping, Composer dispatch, clipping and final SVG/PDF/pixels do not execute; this capture establishes a producer/emitter barrier, not valid retained object records or application crash behavior.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
