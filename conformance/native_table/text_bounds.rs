use super::*;
use frames::{BASE, BASE_SHA256};
use geometry::TEXT_SHA256;

#[path = "text_bounds_grouping.rs"]
mod grouping;

#[path = "text_runs.rs"]
mod runs;

#[path = "text_cached_snapshot.rs"]
mod cached_snapshot;
#[path = "text_ownership.rs"]
mod ownership;

#[path = "text_wrap_numeric.rs"]
mod wrap_numeric;

const TEXT: u64 = 0x0500_0000;
const PARAGRAPH: u64 = MODEL + 0x9000;
const MEASURE: u64 = MODEL + 0x9200;
const RICH_PARAGRAPH: u64 = MODEL + 0x9300;
const LINES: u64 = MODEL + 0xa000;
const BLOCKS: u64 = MODEL + 0xb000;
const BLOCK_POINTERS: u64 = MODEL + 0xc000;
const ENTRIES: u64 = MODEL + 0xd000;
const LOGICAL_MAP: u64 = MODEL + 0xe000;
const PLACED_LINES: u64 = MODEL + 0xf000;
const SPANS: u64 = MODEL + 0x10000;
const FONT: u64 = SPANS + 160;
const DRAWN: u64 = MODEL + 0x11000;
const COPY: u64 = 0x0300_0400;
const SET_LAYOUT: u64 = TEXT + 0x6b4a4;
const SAME_DRAW: u64 = TEXT + 0x65998;
const UNION_START: u64 = TEXT + 0x67274;
const UNION_END: u64 = TEXT + 0x672f0;
const STORE_START: u64 = TEXT + 0x680e0;
const STORE_END: u64 = TEXT + 0x68110;

fn float(engine: Engine, address: u64, value: f32) {
    assert!(value.is_finite());
    write(engine, address, &value.to_le_bytes());
}

fn rectangle(engine: Engine, address: u64) -> [f32; 4] {
    std::array::from_fn(|axis| read_float(engine, address + axis as u64 * 4))
}

fn set_rectangle(engine: Engine, address: u64, bounds: [f32; 4]) {
    for (axis, value) in bounds.into_iter().enumerate() {
        float(engine, address + axis as u64 * 4, value);
    }
}

fn scalar(engine: Engine, index: i32, value: f32) {
    register(engine, 136 + index, u64::from(value.to_bits()));
}

fn execute_window(machine: &Machine, begin: u64, end: u64) {
    check(unsafe { uc_emu_start(machine.engine, begin, end, 1_000_000, 1000) });
    assert_eq!(read_register(machine.engine, 260), end);
}

unsafe extern "C" fn copy(engine: Engine, _: u64, _: u32, _: *mut c_void) {
    let destination = read_register(engine, REGISTER_X0);
    let source = read_register(engine, REGISTER_X0 + 1);
    let count = usize::try_from(read_register(engine, REGISTER_X0 + 2)).unwrap();
    assert!(count <= 80);
    let mut bytes = vec![0; count];
    check(unsafe { uc_mem_read(engine, source, bytes.as_mut_ptr().cast(), count) });
    write(engine, destination, &bytes);
}

struct CopyHook {
    engine: Engine,
    hook: usize,
}

impl CopyHook {
    fn new(machine: &Machine) -> Self {
        write(machine.engine, COPY, &0xd65f03c0_u32.to_le_bytes());
        let mut hook = 0;
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut hook,
                4,
                copy as *mut c_void,
                ptr::null_mut::<c_void>(),
                COPY,
                COPY,
            )
        });
        Self {
            engine: machine.engine,
            hook,
        }
    }
}

impl Drop for CopyHook {
    fn drop(&mut self) {
        check(unsafe { uc_hook_del(self.engine, self.hook) });
    }
}

struct Case {
    name: String,
    cursor: f32,
    margin: f32,
    font_size: f32,
    height: f32,
    pixels: f32,
    multiplier: f32,
    alignment: u32,
    object_metric: bool,
    offset: [f32; 2],
    line_count: usize,
    style_change: bool,
}

fn initialize_layout_shell(machine: &Machine, fill: u8) {
    write(machine.engine, MODEL, &vec![fill; 0x100000]);
    for (address, size) in [
        (PARAGRAPH, 216),
        (MEASURE, 80),
        (RICH_PARAGRAPH, 88),
        (LINES, 128),
        (BLOCKS, 160),
        (SPANS, 144),
        (FONT, 16),
    ] {
        write(machine.engine, address, &vec![0; size]);
    }
    for (address, value) in [
        (PARAGRAPH + 96, MEASURE),
        (PARAGRAPH + 104, PLACED_LINES),
        (PARAGRAPH + 112, PLACED_LINES),
        (PARAGRAPH + 120, PLACED_LINES + 112),
        (PARAGRAPH + 152, LOGICAL_MAP),
        (PARAGRAPH + 184, BLOCK_POINTERS),
        (MEASURE, ENTRIES),
        (MEASURE + 8, RICH_PARAGRAPH),
    ] {
        write(machine.engine, address, &value.to_le_bytes());
    }
    write(machine.engine, MEASURE + 72, &[1]);
    float(machine.engine, MEASURE + 36, 1_000_000.0);
    write(machine.engine, PARAGRAPH + 132, &1_u32.to_le_bytes());
}

impl Case {
    fn place_entries(&self, machine: &Machine, fill: u8) -> Vec<f32> {
        initialize_layout_shell(machine, fill);
        float(machine.engine, RICH_PARAGRAPH + 32, self.pixels);
        float(machine.engine, RICH_PARAGRAPH + 36, self.multiplier);
        float(machine.engine, SPANS, 20.0);
        float(
            machine.engine,
            SPANS + 72,
            if self.style_change { 24.0 } else { 20.0 },
        );
        let mut cursor = self.cursor;
        let mut placements = Vec::new();
        for line in 0..self.line_count {
            let info = LINES + line as u64 * 64;
            let block = BLOCKS + line as u64 * 80;
            let pointer = BLOCK_POINTERS + line as u64 * 8;
            write(machine.engine, pointer, &block.to_le_bytes());
            write(machine.engine, info, &pointer.to_le_bytes());
            write(machine.engine, info + 8, &(pointer + 8).to_le_bytes());
            float(machine.engine, info + 24, self.font_size);
            float(machine.engine, info + 28, self.height);
            float(machine.engine, info + 32, self.margin);
            float(machine.engine, info + 36, self.height);
            write(machine.engine, block, &(line as u32 * 3).to_le_bytes());
            write(
                machine.engine,
                block + 4,
                &(line as u32 * 3 + 2).to_le_bytes(),
            );
            set_rectangle(machine.engine, block + 8, [0.0, 0.0, 30.0, self.height]);
            set_rectangle(machine.engine, block + 24, [4.25, 0.0, 84.25, self.height]);
            write(machine.engine, block + 41, &[u8::from(self.object_metric)]);
            for (within, advance) in [10.0, 12.0, 8.0].into_iter().enumerate() {
                let index = line * 3 + within;
                let entry = ENTRIES + index as u64 * 80;
                write(machine.engine, entry, &[0; 80]);
                float(machine.engine, entry, advance);
                float(machine.engine, entry + 8, 2.0);
                float(machine.engine, entry + 12, 3.0);
                set_rectangle(machine.engine, entry + 32, [3.0, -12.0, 11.0, 6.0]);
                write(
                    machine.engine,
                    LOGICAL_MAP + index as u64 * 4,
                    &(index as u32).to_le_bytes(),
                );
            }
            write(
                machine.engine,
                RICH_PARAGRAPH + 28,
                &self.alignment.to_le_bytes(),
            );
            scalar(machine.engine, 0, cursor);
            machine.call(SET_LAYOUT, &[PARAGRAPH, info]);
            cursor = f32::from_bits(read_register(machine.engine, 136) as u32);
            placements.push(cursor);
        }
        assert_eq!(
            read_u64(machine.engine, PARAGRAPH + 112),
            PLACED_LINES + self.line_count as u64 * 56
        );
        placements
    }

    fn fixture(&self, machine: &Machine, fill: u8) -> String {
        let placements = self.place_entries(machine, fill);
        let mut entries = Vec::new();
        let mut groups = Vec::new();
        let mut first = 0;
        for index in 0..self.line_count * 3 {
            let entry = ENTRIES + index as u64 * 80;
            entries.push(format!(
                "{{\"position\":{:?},\"layout_rect\":{:?},\"glyph_rect\":{:?}}}",
                [
                    read_float(machine.engine, entry + 8),
                    read_float(machine.engine, entry + 12)
                ],
                rectangle(machine.engine, entry + 16),
                rectangle(machine.engine, entry + 32)
            ));
            if index > 0 {
                let previous_style = if self.style_change && index - 1 == 1 {
                    SPANS + 72
                } else {
                    SPANS
                };
                let style = if self.style_change && index == 1 {
                    SPANS + 72
                } else {
                    SPANS
                };
                let same = machine.call(
                    SAME_DRAW,
                    &[0, entry - 80, entry, previous_style, style, FONT, FONT],
                );
                assert!(same <= 1);
                if same == 0 {
                    groups.push(self.drawn_rectangle(machine, first, index));
                    first = index;
                }
            }
        }
        groups.push(self.drawn_rectangle(machine, first, self.line_count * 3));
        format!(
            "{{\"name\":{:?},\"cursor\":{:?},\"margin\":{:?},\"font_size\":{:?},\"base_height\":{:?},\"pixels\":{:?},\"multiplier\":{:?},\"alignment\":{},\"object_metric\":{},\"offset\":{:?},\"line_count\":{},\"style_change\":{},\"post_cursors\":{placements:?},\"entries\":[{}],\"runs\":[{}]}}",
            self.name,
            self.cursor,
            self.margin,
            self.font_size,
            self.height,
            self.pixels,
            self.multiplier,
            self.alignment,
            self.object_metric,
            self.offset,
            self.line_count,
            self.style_change,
            entries.join(","),
            groups.join(",")
        )
    }

    fn drawn_rectangle(&self, machine: &Machine, first: usize, end: usize) -> String {
        for index in first..end {
            let entry = ENTRIES + index as u64 * 80;
            set_rectangle(
                machine.engine,
                STACK + 384,
                rectangle(machine.engine, entry + 32),
            );
            set_rectangle(
                machine.engine,
                STACK + 400,
                rectangle(machine.engine, entry + 16),
            );
            write(machine.engine, STACK + 156, &(index as u32).to_le_bytes());
            write(machine.engine, STACK + 60, &(first as u32).to_le_bytes());
            write(machine.engine, STACK + 416, &0_u64.to_le_bytes());
            write(machine.engine, STACK + 440, &0_u64.to_le_bytes());
            scalar(machine.engine, 10, self.offset[0]);
            scalar(machine.engine, 11, self.offset[1]);
            scalar(machine.engine, 13, read_float(machine.engine, entry + 8));
            scalar(machine.engine, 12, read_float(machine.engine, entry + 12));
            register(machine.engine, REGISTER_SP, STACK);
            execute_window(machine, UNION_START, UNION_END);
        }
        let layout = rectangle(machine.engine, STACK + 560);
        let glyphs = rectangle(machine.engine, STACK + 576);
        set_rectangle(machine.engine, STACK + 96, layout);
        register(machine.engine, 1, STACK);
        register(machine.engine, REGISTER_X0 + 20, DRAWN);
        register(machine.engine, REGISTER_X0 + 23, SPANS);
        for (index, value) in [
            (13, 0.0),
            (12, 0.0),
            (11, glyphs[0]),
            (10, glyphs[1]),
            (9, glyphs[2]),
            (8, glyphs[3]),
        ] {
            scalar(machine.engine, index, value);
        }
        execute_window(machine, STORE_START, STORE_END);
        assert_eq!(rectangle(machine.engine, DRAWN + 104), layout);
        assert_eq!(rectangle(machine.engine, DRAWN + 88), glyphs);
        format!(
            "{{\"range\":[{first},{end}],\"layout_rect\":{layout:?},\"glyph_rect\":{glyphs:?}}}"
        )
    }
}

fn load(machine: &Machine, base: &Path, text: &Path) {
    frames::load_base(machine, base);
    map_library(machine.engine, text, TEXT, TEXT_SHA256);
    for (plt, target) in [
        (0xef860, TEXT + 0x6cb0c),
        (0xef810, TEXT + 0x6c7ec),
        (0xef940, TEXT + 0x8e100),
        (0xef7e0, TEXT + 0x6c61c),
        (0xeed50, BASE + 0xb10e0),
        (0xeee10, BASE + 0xb108c),
        (0xef000, BASE + 0xb11a4),
        (0xef4b0, BASE + 0xb1538),
        (0xef4d0, TEXT + 0x8db40),
        (0xf0970, COPY),
    ] {
        bind_native(machine.engine, TEXT + plt, target);
    }
}

pub(super) fn capture_runs(machine: &mut Machine, base: &Path, text: &Path) {
    load(machine, base, text);
    runs::capture(machine);
}

pub(super) fn capture_ownership(machine: &mut Machine, base: &Path, text: &Path) {
    load(machine, base, text);
    ownership::capture(machine);
}

pub(super) fn capture_cached_runs(machine: &mut Machine, base: &Path, text: &Path) {
    load(machine, base, text);
    runs::capture_cached(machine);
}

pub(super) fn capture_cached_ownership(machine: &mut Machine, base: &Path, text: &Path) {
    load(machine, base, text);
    ownership::capture_cached(machine);
}

pub(super) fn capture_owner_bases(machine: &mut Machine, base: &Path, text: &Path) {
    load(machine, base, text);
    ownership::capture_owner_bases(machine);
}

pub(super) fn capture(machine: &mut Machine, base: &Path, text: &Path) {
    load(machine, base, text);
    let _copy = CopyHook::new(machine);
    let mut captures = Vec::new();
    for (name, font_size, height, pixels, multiplier, object_metric) in [
        ("ordinary", 20.0, 20.0, 0.0, 1.35, false),
        ("pixel-spacing", 20.0, 20.0, 7.0, 1.6, false),
        ("fractional", 17.25, 17.25, 0.0, 1.6, false),
        ("mixed-metrics", 44.0, 60.0, 0.0, 1.35, false),
        ("object-leading", 20.0, 100.0, 0.0, 1.35, true),
        ("large-object-font", 900.0, 100.0, 0.0, 1.35, true),
    ] {
        for alignment in 0..3 {
            for (cursor, margin, offset) in [
                (0.0, 0.0, [0.0, 0.0]),
                (3.25, 4.75, [100.25, 200.75]),
                (-20.5, 1.125, [-30.25, -40.5]),
            ] {
                for (line_count, style_change) in [(1, false), (2, false), (2, true)] {
                    let case = Case {
                        name: format!("{name}-{}", captures.len()),
                        cursor,
                        margin,
                        font_size,
                        height,
                        pixels,
                        multiplier,
                        alignment,
                        object_metric,
                        offset,
                        line_count,
                        style_change,
                    };
                    let expected = case.fixture(machine, 0);
                    for fill in [0xa5, 0xff] {
                        assert_eq!(
                            case.fixture(machine, fill),
                            expected,
                            "{} memory fill",
                            case.name
                        );
                    }
                    captures.push(expected);
                }
            }
        }
    }
    let grouping = grouping::capture(machine);
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"memory_fills\":[0,165,255],\"text_library_sha256\":\"{TEXT_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"set_layout\":\"0x6b4a4\",\"same_draw\":\"0x65998\",\"union_window\":[\"0x67274\",\"0x672f0\"],\"store_window\":[\"0x680e0\",\"0x68110\"],\"entry_advances\":[10,12,8],\"entry_old_position\":[2,3],\"entry_old_glyph_rect\":[3,-12,11,6],\"block_x_bounds\":[4.25,84.25],\"measurement_inputs\":\"Supplied ordinary entry advances/ink bounds, identity logical maps, block bounds/metric flags, line metrics, spacing and drawing offsets. Native SetLayout/GetBaseline, inSameDraw/RichTextSpan comparison and retained rectangle union/storage instructions execute unchanged. Memory copy is host supplied; nonnull Font wrappers have null FontImpl pointers and compare native -1 IDs. No shaping, wrap selection, object placement, bullets, justification, emoji, font-specific draw gates, complete getDrawnTextRun loop, Composer clip or final PDF paths/pixels execute.\",\"cases\":[\n{}\n],\"grouping_cases\":[\n{}\n]}}",
        captures.join(",\n"),
        grouping.join(",\n")
    );
}

pub(super) fn capture_wrap_numeric(machine: &mut Machine, base: &Path, text: &Path) {
    load(machine, base, text);
    wrap_numeric::capture(machine);
}
