use super::*;

fn rectangle(engine: Engine, address: u64) -> [f32; 4] {
    std::array::from_fn(|axis| read_float(engine, address + axis as u64 * 4))
}

fn bytes(engine: Engine, address: u64, count: usize) -> Vec<u8> {
    assert!(count <= 256);
    let mut result = vec![0; count];
    check(unsafe { uc_mem_read(engine, address, result.as_mut_ptr().cast(), count) });
    result
}

fn byte(engine: Engine, address: u64) -> u8 {
    bytes(engine, address, 1)[0]
}

fn string(engine: Engine, address: u64) -> String {
    let marker = byte(engine, address);
    let (data, length) = if marker & 1 == 0 {
        (address + 1, usize::from(marker >> 1))
    } else {
        (
            read_u64(engine, address + 16),
            usize::try_from(read_u64(engine, address + 8)).unwrap(),
        )
    };
    String::from_utf8(bytes(engine, data, length)).unwrap()
}

fn glyphs(machine: &Machine, cache: u64) -> String {
    let begin = read_u64(machine.engine, cache);
    let end = read_u64(machine.engine, cache + 8);
    assert!(end >= begin && end - begin <= 72 && (end - begin).is_multiple_of(12));
    (begin..end)
        .step_by(12)
        .map(|address| {
            format!(
                "[{}, {:?}, {:?}]",
                read_u32(machine.engine, address),
                read_float(machine.engine, address + 4),
                read_float(machine.engine, address + 8)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn font(machine: &Machine, cache: u64) -> String {
    let wrapper = read_u64(machine.engine, cache + 24);
    if wrapper == 0 {
        return "null".into();
    }
    let implementation = read_u64(machine.engine, wrapper + 8);
    assert_ne!(implementation, 0);
    let language = string(
        machine.engine,
        read_u64(machine.engine, implementation + 16),
    );
    format!(
        "{{\"wrapper\":{wrapper},\"id\":{},\"bitmap\":{},\"language\":{language:?}}}",
        read_u32(machine.engine, implementation + 8) as i32,
        byte(machine.engine, implementation + 12) != 0,
    )
}

fn span(machine: &Machine, address: u64) -> String {
    let name = read_u64(machine.engine, address + 24);
    assert_eq!(name, 0, "these generators supply null native font names");
    format!(
        "{{\"font_size\":{:?},\"foreground\":{},\"background\":{},\"composing_background\":{},\"style_bits\":{},\"family\":null,\"underline\":{},\"correction_foreground\":{},\"flags\":{},\"correction_foreground_enabled\":{}}}",
        read_float(machine.engine, address),
        read_u32(machine.engine, address + 4),
        read_u32(machine.engine, address + 8),
        read_u32(machine.engine, address + 12),
        byte(machine.engine, address + 16),
        read_u32(machine.engine, address + 32),
        read_u32(machine.engine, address + 36),
        byte(machine.engine, address + 40),
        byte(machine.engine, address + 66) != 0,
    )
}

pub(super) struct CachedInput {
    pub entries: u64,
    pub caches: u64,
    pub spans: u64,
    pub source: u64,
    pub logical_map: u64,
    pub count: usize,
    pub gravity: f32,
    pub paragraph_flag65: bool,
}

impl CachedInput {
    pub fn snapshot(&self, machine: &Machine) -> String {
        assert!(self.count <= 12);
        let source: Vec<_> = bytes(machine.engine, self.source, self.count * 2)
            .chunks_exact(2)
            .map(|unit| u16::from_le_bytes(unit.try_into().unwrap()))
            .collect();
        let logical_map: Vec<_> = (0..self.count)
            .map(|index| read_u32(machine.engine, self.logical_map + index as u64 * 4))
            .collect();
        let entries: Vec<_> = (0..self.count)
            .map(|index| {
                let entry = self.entries + index as u64 * 80;
                let cache = self.caches + index as u64 * 40;
                let style = self.spans + index as u64 * 72;
                format!(
                    "{{\"advance\":{:?},\"position\":{:?},\"layout_rect\":{:?},\"ink_rect\":{:?},\"kind\":{},\"direction\":{},\"cache_flags\":{:?},\"drawable\":{},\"font\":{},\"span\":{},\"glyphs\":[{}]}}",
                    read_float(machine.engine, entry),
                    [read_float(machine.engine, entry + 8), read_float(machine.engine, entry + 12)],
                    rectangle(machine.engine, entry + 16),
                    rectangle(machine.engine, entry + 32),
                    read_u32(machine.engine, entry + 48),
                    read_u32(machine.engine, entry + 52) as i32,
                    [byte(machine.engine, cache + 32), byte(machine.engine, cache + 33)],
                    byte(machine.engine, cache + 34) != 0,
                    font(machine, cache),
                    span(machine, style),
                    glyphs(machine, cache),
                )
            })
            .collect();
        format!(
            "{{\"source_utf16\":{source:?},\"logical_map\":{logical_map:?},\"gravity\":{:?},\"paragraph_flag65\":{},\"entries\":[{}]}}",
            self.gravity,
            self.paragraph_flag65,
            entries.join(","),
        )
    }
}
