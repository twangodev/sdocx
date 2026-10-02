use super::*;

const LAYOUT: u64 = MODEL + 0x19000;
const SHELL: u64 = MODEL + 0x19100;
const MEASURE: u64 = MODEL + 0x19200;
const ENTRY_VECTOR: u64 = MODEL + 0x19300;
const GLYPH_VECTOR: u64 = MODEL + 0x19400;
const STYLE: u64 = MODEL + 0x19500;
const ENTRIES: u64 = MODEL + 0x1a000;
const GLYPH_ENTRIES: u64 = MODEL + 0x1d000;
const GLYPH_STORAGE: u64 = MODEL + 0x22000;

unsafe extern "C" {
    fn uc_emu_stop(engine: Engine) -> i32;
}
unsafe extern "C" fn stop_window(engine: Engine, _: u64, _: u32, _: *mut c_void) {
    check(unsafe { uc_emu_stop(engine) });
}
fn window(machine: &Machine, begin: u64, end: u64) {
    let mut hook = 0;
    check(unsafe {
        uc_hook_add(
            machine.engine,
            &mut hook,
            4,
            stop_window as *mut c_void,
            ptr::null_mut::<c_void>(),
            TEXT + end,
            TEXT + end,
        )
    });
    let error = unsafe { uc_emu_start(machine.engine, TEXT + begin, TEXT + end, 0, 100_000) };
    check(unsafe { uc_hook_del(machine.engine, hook) });
    assert_eq!(
        error,
        0,
        "entry window {begin:x}..{end:x} failed at PC {:x}, LR {:x}, x0 {:x}",
        read_register(machine.engine, 260),
        read_register(machine.engine, REGISTER_X30),
        read_register(machine.engine, REGISTER_X0),
    );
    assert_eq!(read_register(machine.engine, 260), TEXT + end);
}

fn point(engine: Engine, address: u64) -> [u32; 2] {
    [0, 4].map(|offset| read_u32(engine, address + offset))
}

fn rectangle(engine: Engine, address: u64) -> [u32; 4] {
    [0, 4, 8, 12].map(|offset| read_u32(engine, address + offset))
}

pub(super) fn paint_size(machine: &Machine, size: f32) -> f32 {
    f32::from_bits(paint_profile(machine, size, 0).size_bits)
}

struct PaintProfile {
    source_style: u8,
    source_size_bits: u32,
    size_bits: u32,
    scale_bits: u32,
    skew_bits: u32,
    flags: u64,
    encoding: u64,
    underline: bool,
    bold: bool,
    skia_bitfield: u32,
}

impl PaintProfile {
    fn json(&self) -> String {
        format!(
            "{{\"source_style\":{},\"source_size_bits\":{},\"source_foreground\":4279312947,\"paint_size_bits\":{},\"scale_x_bits\":{},\"skew_x_bits\":{},\"packed_minikin_flags\":{},\"encoding\":{},\"underline\":{},\"fake_bold\":{},\"skia_bitfield\":{}}}",
            self.source_style,
            self.source_size_bits,
            self.size_bits,
            self.scale_bits,
            self.skew_bits,
            self.flags,
            self.encoding,
            self.underline,
            self.bold,
            self.skia_bitfield,
        )
    }
}

fn paint_profile(machine: &Machine, size: f32, style: u8) -> PaintProfile {
    let stack = STACK - 0x2000;
    write(machine.engine, STYLE, &[0; 72]);
    write(machine.engine, STYLE, &size.to_bits().to_le_bytes());
    write(machine.engine, STYLE + 4, &0xff112233_u32.to_le_bytes());
    write(machine.engine, STYLE + 16, &[style]);
    register(machine.engine, REGISTER_SP, stack);
    register(machine.engine, REGISTER_X0, stack + 56);
    register(machine.engine, REGISTER_X0 + 23, STYLE);
    window(machine, 0x76b14, 0x76bb8);
    let paint = read_u64(machine.engine, stack + 64);
    assert_ne!(paint, 0);
    let scalar = |function| {
        machine.call(TEXT + function, &[paint]);
        read_register(machine.engine, 136) as u32
    };
    let size_bits = scalar(0x7c154);
    let scale_bits = scalar(0x7c15c);
    let skew_bits = scalar(0x7c164);
    let flags = machine.call(TEXT + 0x7c1f4, &[paint]);
    let encoding = machine.call(TEXT + 0x7c148, &[paint]);
    let underline = machine.call(TEXT + 0x7c36c, &[paint]);
    let bold = machine.call(TEXT + 0x7c384, &[paint]);
    PaintProfile {
        source_style: style,
        source_size_bits: size.to_bits(),
        size_bits,
        scale_bits,
        skew_bits,
        flags,
        encoding,
        underline: underline != 0,
        bold: bold != 0,
        skia_bitfield: read_u32(machine.engine, paint + 112),
    }
}

struct GlyphGeometry {
    glyph_id: u32,
    owner_utf16: u32,
    full_position_bits: [u32; 2],
    owner_position_bits: [u32; 2],
    ink_bits: [u32; 4],
    entry_position_bits: [u32; 2],
    translated_ink_bits: [u32; 4],
    entry_ink_bits: [u32; 4],
}

impl GlyphGeometry {
    fn json(&self) -> String {
        format!(
            "{{\"glyph_id\":{},\"owner_utf16\":{},\"full_position_bits\":{:?},\"owner_position_bits\":{:?},\"layout_ink_bits\":{:?},\"entry_position_bits\":{:?},\"translated_ink_bits\":{:?},\"entry_ink_bits\":{:?}}}",
            self.glyph_id,
            self.owner_utf16,
            self.full_position_bits,
            self.owner_position_bits,
            self.ink_bits,
            self.entry_position_bits,
            self.translated_ink_bits,
            self.entry_ink_bits,
        )
    }
}

pub(super) fn capture(machine: &Machine, case: &Case, source: &[u16], range: [u32; 2]) -> String {
    assert!(source.len() <= 128);
    write(machine.engine, LAYOUT, &[0; 256]);
    machine.call(TEXT + 0x97c98, &[LAYOUT, source.len() as u64]);
    register(machine.engine, 136, 0);
    machine.call(TEXT + 0x9dbf0, &[LAYOUT, PIECE, u64::from(range[0])]);
    let records = read_u64(machine.engine, LAYOUT);
    let end = read_u64(machine.engine, LAYOUT + 8);
    assert!(end >= records && (end - records) % 64 == 0);
    let glyph_count = (end - records) as usize / 64;
    assert!(glyph_count <= 128);
    let owners = (0..glyph_count)
        .map(|index| {
            let owner = read_u64(machine.engine, records + index as u64 * 64 + 40);
            assert!(owner < source.len() as u64);
            u32::try_from(owner).unwrap()
        })
        .collect::<Vec<_>>();
    let mut owner_counts = vec![0; source.len()];
    for owner in &owners {
        owner_counts[*owner as usize] += 1;
    }
    write(machine.engine, SHELL, &[0; 0x500]);
    write(machine.engine, ENTRIES, &vec![0; source.len() * 80]);
    write(machine.engine, GLYPH_ENTRIES, &vec![0; source.len() * 40]);
    write(machine.engine, SHELL + 16, &MEASURE.to_le_bytes());
    write(machine.engine, MEASURE, &ENTRY_VECTOR.to_le_bytes());
    write(machine.engine, MEASURE + 8, &GLYPH_VECTOR.to_le_bytes());
    vector(machine.engine, ENTRY_VECTOR, ENTRIES, source.len() * 80);
    vector(
        machine.engine,
        GLYPH_VECTOR,
        GLYPH_ENTRIES,
        source.len() * 40,
    );
    let mut cursor = GLYPH_STORAGE;
    for (owner, count) in owner_counts.iter().enumerate() {
        let address = GLYPH_ENTRIES + owner as u64 * 40;
        write(machine.engine, address, &cursor.to_le_bytes());
        write(machine.engine, address + 8, &cursor.to_le_bytes());
        cursor += count * 12;
        write(machine.engine, address + 16, &cursor.to_le_bytes());
        machine.call(TEXT + 0xef900, &[ENTRIES + owner as u64 * 80 + 32]);
    }
    write(
        machine.engine,
        STYLE,
        &case.font_size.to_bits().to_le_bytes(),
    );
    let mut geometry = Vec::new();
    for (index, owner) in owners.iter().copied().enumerate() {
        let record = records + index as u64 * 64;
        let entry = GLYPH_ENTRIES + u64::from(owner) * 40;
        let destination = read_u64(machine.engine, entry + 8);
        let divisor = [100_f32.to_bits(), 100_f32.to_bits(), 0, 0];
        check(unsafe { uc_reg_write(machine.engine, 114, divisor.as_ptr().cast()) });
        register(machine.engine, REGISTER_X0 + 8, record);
        register(machine.engine, REGISTER_X0 + 9, GLYPH_ENTRIES);
        register(machine.engine, REGISTER_X0 + 27, u64::from(owner));
        register(machine.engine, REGISTER_X0 + 28, 0);
        window(machine, 0x773e0, 0x7741c);
        window(machine, 0x774e4, 0x774e8);
        assert_eq!(
            read_u32(machine.engine, destination),
            read_u32(machine.engine, record + 16)
        );

        register(machine.engine, REGISTER_SP, STACK - 0x1000);
        register(machine.engine, REGISTER_X0 + 8, record);
        window(machine, 0x775fc, 0x77610);
        let translated_ink = rectangle(machine.engine, STACK - 0x1000 + 48);
        register(machine.engine, 144, u64::from(0.01_f32.to_bits()));
        window(machine, 0x77610, 0x7761c);
        let entry_ink = rectangle(machine.engine, STACK - 0x1000 + 48);
        register(machine.engine, REGISTER_X0 + 23, SHELL);
        register(machine.engine, REGISTER_X0 + 28, u64::from(owner));
        window(machine, 0x7761c, 0x77640);

        register(machine.engine, REGISTER_X0 + 20, LAYOUT);
        register(machine.engine, REGISTER_X0 + 19, u64::from(owner));
        register(machine.engine, REGISTER_X0 + 21, STYLE);
        register(machine.engine, REGISTER_X0 + 23, SHELL);
        register(machine.engine, REGISTER_X0 + 28, u64::from(owner));
        window(machine, 0x77640, 0x77678);
        geometry.push(GlyphGeometry {
            glyph_id: read_u32(machine.engine, record + 16),
            owner_utf16: owner,
            full_position_bits: point(machine.engine, record + 20),
            owner_position_bits: point(machine.engine, record + 28),
            ink_bits: rectangle(machine.engine, record + 48),
            entry_position_bits: point(machine.engine, destination + 4),
            translated_ink_bits: translated_ink,
            entry_ink_bits: entry_ink,
        });
    }
    let advance_start = read_u64(machine.engine, LAYOUT + 24);
    let advances = (0..source.len())
        .map(|index| read_u32(machine.engine, advance_start + index as u64 * 4))
        .collect::<Vec<_>>();
    let entry_widths = (0..source.len())
        .map(|index| read_u32(machine.engine, ENTRIES + index as u64 * 80))
        .collect::<Vec<_>>();
    let entry_sizes = (0..source.len())
        .map(|index| point(machine.engine, ENTRIES + index as u64 * 80 + 4)[0])
        .collect::<Vec<_>>();
    let entry_secondary_sizes = (0..source.len())
        .map(|index| read_u32(machine.engine, ENTRIES + index as u64 * 80 + 60))
        .collect::<Vec<_>>();
    let entry_ink = (0..source.len())
        .map(|index| rectangle(machine.engine, ENTRIES + index as u64 * 80 + 32))
        .collect::<Vec<_>>();
    let profiles = (0..8)
        .map(|style| paint_profile(machine, case.font_size, style).json())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"source_utf16\":{:?},\"append_owner_origin_utf16\":{},\"append_extra_advance_bits\":0,\"layout_total_advance_bits\":{},\"layout_character_advances_bits\":{:?},\"glyphs\":[{}],\"entry_widths_bits\":{:?},\"entry_sizes_bits\":{:?},\"entry_secondary_sizes_bits\":{:?},\"entry_ink_bits\":{:?},\"paint_profiles\":[{profiles}]}}",
        source,
        range[0],
        read_u32(machine.engine, LAYOUT + 48),
        advances,
        geometry
            .iter()
            .map(GlyphGeometry::json)
            .collect::<Vec<_>>()
            .join(","),
        entry_widths,
        entry_sizes,
        entry_secondary_sizes,
        entry_ink,
    )
}

pub(super) const CAPTURE_BOUNDARY: &str = "Actual initialized file Font/FontFamily/FontCollection and LayoutPiece execute on supplied pinned Roboto; native layout initializer 0x97c98 and complete append 0x9dbf0 execute, producing glyph/full/owner positions, ink and UTF16 advances. Bounded SpanRunFunctor instruction windows execute f32 size*100, owner-position division by100, owner-position addition to already-offset layout ink followed by actual RectF::Scale(0.01f32), actual RectF::SetEmpty/Union, and per-owner width division by100 into entry records. Supplied layout owner origin, zero extra advance, source/entry/vector storage and preallocated glyph capacity are caller boundaries. Paint profile window executes actual TextPaint constructor and encoding/size/color/underline/fake-bold/base-skew setters for source style0..7, then actual getters; these profiles do not substitute for captured shaping paint or establish typeface selection. Whole SpanRunFunctor, Typeface/manager initialization, editing, entry classification, wrapping, fallback fonts and document composition do not execute.";

pub(super) fn cases() -> Vec<Case> {
    let mut cases = vec![
        Case::regular("av_default", "AV", 17.0),
        Case::regular("fractional_size", "To", 17.125),
        Case::regular("large_size", "AV", 23.0),
        Case::regular("marks", "x\u{327}\u{301}y", 17.125),
        Case::regular("supplementary", "A😀V", 17.125),
    ];
    let mut partial = Case::regular("partial_marks", "😀x\u{327}\u{301}y😀", 17.125);
    partial.range = Some([2, 6]);
    cases.push(partial);
    let mut italic = Case::regular("italic_marks", "x\u{327}\u{301}y", 17.125);
    italic.skew = -0.25;
    cases.push(italic);
    let mut rtl = Case::regular("rtl_marks", "x\u{327}\u{301}y", 17.125);
    rtl.rtl = true;
    cases.push(rtl);
    for case in &mut cases {
        case.entry_geometry = true;
    }
    cases
}
