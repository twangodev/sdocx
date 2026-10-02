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
const SOURCE_VECTOR: u64 = MEASURE + 0xa00;
const RECORDS: u64 = MEASURE + 0xb00;
const FONTS: u64 = MEASURE + 0xd00;
const FONT_IMPL: u64 = MEASURE + 0xe00;
const FONT_VTABLE: u64 = MEASURE + 0xe40;
const LANGUAGE: u64 = MEASURE + 0xef0;
const HOLDER: u64 = MEASURE + 0xf00;
const WRAPPER: u64 = MEASURE + 0xf10;
const ADVANCES: u64 = MEASURE + 0xf40;
const PARAGRAPH: u64 = MODEL + 0x1a000;
const LAYOUT_MEASURE: u64 = PARAGRAPH + 0x100;
const RICH_PARAGRAPH: u64 = PARAGRAPH + 0x200;
const LINE: u64 = PARAGRAPH + 0x300;
const BLOCK: u64 = PARAGRAPH + 0x400;
const BLOCK_POINTER: u64 = PARAGRAPH + 0x500;
const MAP: u64 = PARAGRAPH + 0x700;
const PLACED_LINE: u64 = PARAGRAPH + 0x800;
const COPY: u64 = 0x0300_2000;
const GET_ID: u64 = COPY + 32;
const GET_BITMAP: u64 = COPY + 64;
const GET_LANGUAGE: u64 = COPY + 96;
const NOOP: u64 = COPY + 128;
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

fn scalar_words(engine: Engine, address: u64) -> Vec<u32> {
    let [begin, end, _] = vector(engine, address);
    assert!(end >= begin && end - begin <= 96 && (end - begin).is_multiple_of(4));
    (begin..end)
        .step_by(4)
        .map(|address| read_u32(engine, address))
        .collect()
}

fn glyph_words(engine: Engine, address: u64) -> Vec<u32> {
    let words = scalar_words(engine, address);
    assert!(words.len().is_multiple_of(3));
    words
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
    producer_glyph_count: usize,
}

impl Case {
    fn fixture(&self, machine: &mut Machine, fill: u8) -> String {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        write(machine.engine, MEASURE, &vec![0; 0x2900]);
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
        let mut supplied_shaped = Vec::new();
        if self.producer_glyph_count == 0 {
            machine.call(TEXT + 0x779d0, &[FUNCTOR, SPAN, 0]);
        } else {
            supplied_shaped = self.produce_shaped_object(machine, cache);
        }
        let kind = read_u32(machine.engine, entries + 48);
        let mode = read_u32(machine.engine, entries + 64);
        let advance = read_float(machine.engine, entries);
        let height = read_float(machine.engine, entries + 4);
        let ink: [f32; 4] =
            std::array::from_fn(|axis| read_float(machine.engine, entries + 32 + axis as u64 * 4));
        let produced_glyphs = vector(machine.engine, cache);
        let produced_drawable = byte(machine.engine, cache + 34);
        let produced_glyph_words = glyph_words(machine.engine, cache);
        let placed = if self.producer_glyph_count == 0 {
            "null".into()
        } else {
            self.place_object(machine, entries, advance, height)
        };
        register(machine.engine, REGISTER_X0 + 8, RETURN_GLYPHS);
        machine.call(TEXT + 0x78274, &[MEASURE, 0]);
        let returned_glyphs = vector(machine.engine, RETURN_GLYPHS);
        let returned_drawable = byte(machine.engine, RETURN_GLYPHS + 34);
        assert_eq!(
            produced_glyph_words,
            glyph_words(machine.engine, RETURN_GLYPHS)
        );
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
        assert_eq!(recorder.observation.first_word_reads, 1);
        assert_eq!(recorder.observation.append_inputs.len(), 1);
        let output = vector(machine.engine, OUTPUT);
        let (terminal, runs) = if self.producer_glyph_count == 0 {
            assert_eq!(error, 6, "expected unmapped read, PC {pc:x}");
            assert_eq!(pc, FIRST_WORD);
            assert_eq!(output, [0; 3]);
            ("isolated-helper-missing-upstream-glyph", Vec::new())
        } else {
            assert_eq!(error, 0, "native emitter failed, PC {pc:x}");
            assert_eq!(pc, STOP);
            assert_eq!(output[1] - output[0], 8);
            let record = read_u64(machine.engine, output[0]);
            let codes = scalar_words(machine.engine, record + 8);
            let positions = scalar_words(machine.engine, record + 32);
            assert_eq!(codes.len(), self.producer_glyph_count);
            assert_eq!(
                codes,
                produced_glyph_words
                    .chunks_exact(3)
                    .map(|glyph| glyph[0])
                    .collect::<Vec<_>>()
            );
            assert_eq!(byte(machine.engine, record + 120), 1);
            assert_eq!(read_u32(machine.engine, record + 124), 7);
            assert_eq!(read_u32(machine.engine, record + 144), self.background);
            let origin: [f32; 2] = std::array::from_fn(|axis| {
                read_float(machine.engine, record + 80 + axis as u64 * 4)
            });
            let layout: [f32; 4] = std::array::from_fn(|axis| {
                read_float(machine.engine, record + 104 + axis as u64 * 4)
            });
            let ink: [f32; 4] = std::array::from_fn(|axis| {
                read_float(machine.engine, record + 88 + axis as u64 * 4)
            });
            let run = format!(
                "{{\"range_inclusive\":[{},{}],\"codewords\":{codes:?},\"position_bits\":{positions:?},\"origin\":{origin:?},\"layout_rect\":{layout:?},\"ink_rect\":{ink:?},\"span_bit1\":{},\"font_id\":{},\"font_size\":{:?},\"foreground\":{},\"style\":{},\"ordinary_background\":{}}}",
                read_u32(machine.engine, record),
                read_u32(machine.engine, record + 4),
                byte(machine.engine, record + 120),
                read_u32(machine.engine, record + 124) as i32,
                read_float(machine.engine, record + 132),
                read_u32(machine.engine, record + 136),
                byte(machine.engine, record + 140),
                read_u32(machine.engine, record + 144),
            );
            ("producer-window-retained-record-published", vec![run])
        };
        format!(
            "{{\"name\":{:?},\"inline\":{},\"object\":{},\"dimension_bits\":{:?},\"background\":{},\"composing\":{},\"offset\":{:?},\"public_get_drawn_text\":{},\"supplied_shaped_records\":[{}],\"initialized_glyph_vector\":{before_glyphs:?},\"initialized_drawable\":{before_drawable},\"produced_entry\":{{\"kind\":{kind},\"object_mode\":{mode},\"advance\":{advance},\"height\":{height},\"ink\":{ink:?}}},\"produced_glyph_vector\":{produced_glyphs:?},\"produced_drawable\":{produced_drawable},\"produced_glyph_words\":{produced_glyph_words:?},\"placed_entry\":{placed},\"returned_glyph_vector\":{returned_glyphs:?},\"returned_drawable\":{returned_drawable},\"append_inputs\":[{}],\"incomplete_records\":[{}],\"terminal\":{terminal:?},\"native_terminal_pc\":{},\"unicorn_error\":{error},\"output_vector\":{output:?},\"runs\":[{}]}}",
            self.name,
            self.inline,
            self.object,
            self.dimensions,
            self.background,
            self.composing,
            self.offset,
            self.public_api,
            supplied_shaped.join(","),
            recorder.observation.append_inputs.join(","),
            recorder.observation.incomplete_records.join(","),
            if pc == STOP { 0 } else { pc - TEXT },
            runs.join(","),
        )
    }

    fn produce_shaped_object(&self, machine: &Machine, cache: u64) -> Vec<String> {
        assert!(self.producer_glyph_count <= 2);
        write(machine.engine, STACK, &vec![0; 0x600]);
        for (address, value) in [
            (FUNCTOR, SOURCE_VECTOR),
            (CONTEXT + 8, MEASURE + 48),
            (SOURCE_VECTOR, CHARACTERS),
            (STACK + 168, RECORDS),
            (STACK + 176, RECORDS + self.producer_glyph_count as u64 * 64),
            (STACK + 192, ADVANCES),
            (HOLDER, WRAPPER),
            (WRAPPER, 1),
            (cache + 24, FONTS),
            (FONTS + 8, FONT_IMPL),
            (FONT_IMPL, FONT_VTABLE),
            (FONT_IMPL + 16, LANGUAGE),
            (FONT_VTABLE + 48, GET_ID),
            (FONT_VTABLE + 56, GET_BITMAP),
            (FONT_VTABLE + 80, GET_LANGUAGE),
        ] {
            pointer(machine.engine, address, value);
        }
        word(machine.engine, SOURCE_VECTOR + 8, 1);
        word(machine.engine, FONT_IMPL + 8, 7);
        write(machine.engine, cache + 34, &[1]);
        write(machine.engine, LANGUAGE, &[4, b'e', b'n']);
        word(machine.engine, ADVANCES, 1000.0_f32.to_bits());
        let mut records = Vec::new();
        for index in 0..self.producer_glyph_count {
            let record = RECORDS + index as u64 * 64;
            let codeword = 0x4101 + index as u32;
            let position = [index as f32 * 225.0, -75.0];
            let bounds = [0.0_f32, -1200.0, 800.0, 600.0];
            pointer(machine.engine, record, HOLDER);
            word(machine.engine, record + 16, codeword);
            pointer(machine.engine, record + 40, 0);
            for (axis, value) in position.into_iter().enumerate() {
                word(
                    machine.engine,
                    record + 28 + axis as u64 * 4,
                    value.to_bits(),
                );
            }
            for (axis, value) in bounds.into_iter().enumerate() {
                word(
                    machine.engine,
                    record + 48 + axis as u64 * 4,
                    value.to_bits(),
                );
            }
            records.push(format!("{{\"owner_utf16\":0,\"codeword\":{codeword},\"position\":{position:?},\"ink_rect\":{bounds:?}}}"));
        }
        for (register_id, value) in [
            (REGISTER_SP, STACK),
            (1, STACK + 0x400),
            (REGISTER_X0 + 20, STACK + 168),
            (REGISTER_X0 + 21, SPAN),
            (REGISTER_X0 + 23, FUNCTOR),
            (REGISTER_X0 + 24, 0),
            (REGISTER_X0 + 28, 0),
        ] {
            register(machine.engine, register_id, value);
        }
        check(unsafe {
            uc_emu_start(
                machine.engine,
                TEXT + 0x77324,
                TEXT + 0x77894,
                1_000_000,
                1_000_000,
            )
        });
        assert_eq!(read_register(machine.engine, 260), TEXT + 0x77894);
        assert_eq!(
            glyph_words(machine.engine, cache).len(),
            self.producer_glyph_count * 3
        );
        records
    }

    fn place_object(&self, machine: &Machine, entries: u64, advance: f32, height: f32) -> String {
        for (address, value) in [
            (PARAGRAPH + 96, LAYOUT_MEASURE),
            (PARAGRAPH + 104, PLACED_LINE),
            (PARAGRAPH + 112, PLACED_LINE),
            (PARAGRAPH + 120, PLACED_LINE + 56),
            (PARAGRAPH + 152, MAP),
            (PARAGRAPH + 184, BLOCK_POINTER),
            (LAYOUT_MEASURE, entries),
            (LAYOUT_MEASURE + 8, RICH_PARAGRAPH),
            (LINE, BLOCK_POINTER),
            (LINE + 8, BLOCK_POINTER + 8),
            (BLOCK_POINTER, BLOCK),
        ] {
            pointer(machine.engine, address, value);
        }
        write(machine.engine, LAYOUT_MEASURE + 72, &[1]);
        word(
            machine.engine,
            LAYOUT_MEASURE + 36,
            1_000_000.0_f32.to_bits(),
        );
        word(machine.engine, RICH_PARAGRAPH + 36, 1.35_f32.to_bits());
        word(machine.engine, PARAGRAPH + 132, 1);
        for (address, value) in [(LINE + 24, 17.0), (LINE + 28, height), (LINE + 36, height)] {
            word(machine.engine, address, value.to_bits());
        }
        for (base, bounds) in [
            (BLOCK + 8, [0.0, 0.0, advance, height]),
            (BLOCK + 24, [4.25, 0.0, 104.25, height]),
        ] {
            for (axis, value) in bounds.into_iter().enumerate() {
                word(machine.engine, base + axis as u64 * 4, value.to_bits());
            }
        }
        write(machine.engine, BLOCK + 41, &[1]);
        register(machine.engine, 136, u64::from(3.25_f32.to_bits()));
        machine.call(TEXT + 0x6b4a4, &[PARAGRAPH, LINE]);
        let cursor = f32::from_bits(read_register(machine.engine, 136) as u32);
        let position: [f32; 2] =
            std::array::from_fn(|axis| read_float(machine.engine, entries + 8 + axis as u64 * 4));
        let layout: [f32; 4] =
            std::array::from_fn(|axis| read_float(machine.engine, entries + 16 + axis as u64 * 4));
        let ink: [f32; 4] =
            std::array::from_fn(|axis| read_float(machine.engine, entries + 32 + axis as u64 * 4));
        format!(
            "{{\"cursor\":{cursor:?},\"position\":{position:?},\"layout_rect\":{layout:?},\"ink_rect\":{ink:?}}}"
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
        (0xeef40, TEXT + 0x62344),
        (0xf0560, NOOP),
        (0xf0570, NOOP),
        (0xef900, BASE + 0xb10ec),
        (0xefc60, BASE + 0xb1510),
        (0xefc40, TEXT + 0x779d0),
        (0xef860, TEXT + 0x6cb0c),
        (0xef810, TEXT + 0x6c7ec),
        (0xef940, TEXT + 0x8e100),
        (0xef7e0, TEXT + 0x6c61c),
        (0xef000, BASE + 0xb11a4),
        (0xef4b0, BASE + 0xb1538),
        (0xeed50, BASE + 0xb10e0),
        (0xeee10, BASE + 0xb108c),
        (0xeee20, BASE + 0xb109c),
        (0xf0970, COPY),
        (0xf08e0, COPY),
    ] {
        bind_native(machine.engine, TEXT + plt, target);
    }
    for (address, instruction) in [
        (GET_ID, 0xb9400800_u32),
        (GET_BITMAP, 0x39403000),
        (GET_LANGUAGE, 0xf9400800),
        (NOOP, 0xd65f03c0),
    ] {
        word(machine.engine, address, instruction);
        word(machine.engine, address + 4, 0xd65f03c0);
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
                    producer_glyph_count: 0,
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
        producer_glyph_count: 0,
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
        producer_glyph_count: 0,
    });
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
                for producer_glyph_count in 1..=2 {
                    cases.push(Case {
                        name: format!("producer-window-{name}-{color}-inline-{inline}-glyphs-{producer_glyph_count}"),
                        inline, object: true, dimensions, background, composing,
                        offset: [4.25, 12.5], public_api: true, producer_glyph_count,
                    });
                }
            }
        }
    }
    for inline in [false, true] {
        for (name, background, composing) in [
            ("composing-only", 0, 0x80654321),
            ("alpha-zero-background", 0x00123456, 0),
            ("mapped-zero-sentinel", 0x00ffffff, 0),
        ] {
            cases.push(Case {
                name: format!("producer-window-{name}-inline-{inline}"),
                inline,
                object: true,
                dimensions: [30.0_f32.to_bits(), 40.0_f32.to_bits()],
                background,
                composing,
                offset: [4.25, 12.5],
                public_api: true,
                producer_glyph_count: 1,
            });
        }
    }
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
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"memory_fills\":[0,165,255],\"guest_zero_page\":\"Model ELF header page explicitly unmapped\",\"measurement_context\":{{\"available_width\":120,\"left_reserved\":10,\"right_reserved\":10,\"object_margins\":[0.25,1.5,0.75,2.25]}},\"measure_constructor\":\"0x78090\",\"init_measure_data\":\"0x790d0\",\"measure_object_span\":\"0x779d0\",\"glyph_info\":\"0x78274\",\"get_drawn_text_run\":\"0x66c98\",\"public_get_drawn_text\":\"0x68418\",\"append_text_block\":\"0x67ebc\",\"first_word_load\":\"0x68144\",\"producer_window\":[\"0x77324\",\"0x77894\"],\"set_layout\":\"0x6b4a4\",\"capture_bounds\":\"Native constructor/initMeasureData allocate/default-initialize one entry and its cache. Isolated-helper cases execute measureObjectSpan on the initialized empty cache and demonstrate appendTextBlock missing-glyph precondition, not application crash behavior. Producer-window cases supply one or two shaped records with owner UTF-16 index0, glyph words/positions/ink, advance1000 and cached Font implementation ID7/bitmap0/language en/drawable=true and retained mapped colors, explicitly bypassing font construction. The actual SpanRunFunctor operator window appends these supplied records into native allocated GlyphInfo storage, measures and overrides object dimensions through its actual 0x77844 helper call. Native SetLayout places the entry using supplied one-line/one-block metrics and logical map; those metrics include captured native advance/height plus supplied font metric17, spacing1.35, cursor3.25 and block bounds. Native GetGlyphInfo and complete public GetDrawnText/getDrawnTextRun/appendTextBlock retain glyph words, object flag, placed layout/ink rectangles and ordinary span background. Composing background remains a supplied span input and is not copied into DrawnText. Guest zero page is explicitly unmapped. Host interfaces are allocation/deletion, bounded copy/move, mutex no-ops and supplied Font getters. Theme mapping is not executed; supplied ARGB fields include composing-only and zero-alpha nonzero values. The Composer table background consumer and its alpha gate are source-traced separately; this capture does not apply that policy, and body Composer routes have separate object filtering. Full Widget conversion, full upstream span dispatch, actual font selection/shaping, line/block metric production/wrapping, outer GetDrawnTextData/Composer background painting, clipping and final SVG/PDF/pixels do not execute. Published records are bounded evidence with supplied shaped records, not full native object composition parity.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
