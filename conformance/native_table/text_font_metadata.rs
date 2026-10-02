use super::*;
use geometry::TEXT_SHA256;

const TEXT: u64 = 0x0500_0000;
const FONT: u64 = MODEL + 0x18000;
const IMPLEMENTATION: u64 = FONT + 0x100;
const SOURCE: u64 = FONT + 0x200;
const TYPEFACE: u64 = FONT + 0x300;
const IMPLEMENTATION_VTABLE: u64 = FONT + 0x400;
const SOURCE_VTABLE: u64 = FONT + 0x500;
const LANGUAGE_BYTES: u64 = FONT + 0x600;
const GET_TABLE_TAGS: u64 = TEXT + 0xf04c0;

#[derive(Default)]
struct TagResults {
    tags: Vec<u32>,
    calls: usize,
}

unsafe extern "C" fn table_tags(engine: Engine, _: u64, _: u32, data: *mut c_void) {
    let results = unsafe { &mut *data.cast::<TagResults>() };
    assert_eq!(read_register(engine, REGISTER_X0), TYPEFACE);
    let destination = read_register(engine, REGISTER_X0 + 1);
    assert!(results.tags.len() <= 32);
    for (index, tag) in results.tags.iter().enumerate() {
        write(engine, destination + index as u64 * 4, &tag.to_le_bytes());
    }
    results.calls += 1;
    register(engine, REGISTER_X0, results.tags.len() as u64);
}

struct TagHook {
    engine: Engine,
    hook: usize,
    results: Box<TagResults>,
}

impl TagHook {
    fn new(machine: &Machine) -> Self {
        let mut hook = Self {
            engine: machine.engine,
            hook: 0,
            results: Box::default(),
        };
        write(
            machine.engine,
            GET_TABLE_TAGS,
            &0xd65f03c0_u32.to_le_bytes(),
        );
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut hook.hook,
                4,
                table_tags as *mut c_void,
                ptr::from_mut(hook.results.as_mut()).cast(),
                GET_TABLE_TAGS,
                GET_TABLE_TAGS,
            )
        });
        hook
    }
}

impl Drop for TagHook {
    fn drop(&mut self) {
        check(unsafe { uc_hook_del(self.engine, self.hook) });
    }
}

struct Case {
    name: String,
    tags: Option<Vec<u32>>,
    source_present: bool,
    implementation_present: bool,
    source_id: i32,
    language: &'static str,
    allocated_language: bool,
}

fn read_string(engine: Engine, address: u64) -> String {
    let mut first = 0_u8;
    check(unsafe { uc_mem_read(engine, address, ptr::from_mut(&mut first).cast(), 1) });
    let (length, bytes) = if first & 1 == 0 {
        (usize::from(first >> 1), address + 1)
    } else {
        (
            read_u64(engine, address + 8) as usize,
            read_u64(engine, address + 16),
        )
    };
    assert!(length <= 64);
    let mut content = vec![0; length];
    check(unsafe { uc_mem_read(engine, bytes, content.as_mut_ptr().cast(), length) });
    String::from_utf8(content).unwrap()
}

impl Case {
    fn fixture(&self, machine: &mut Machine, hook: &mut TagHook, fill: u8) -> String {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        for (address, size) in [
            (FONT, 24),
            (IMPLEMENTATION, 64),
            (SOURCE, 128),
            (IMPLEMENTATION_VTABLE, 88),
            (SOURCE_VTABLE, 88),
        ] {
            write(machine.engine, address, &vec![0; size]);
        }
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        machine.heap.allocations = 0;
        machine.heap.deletes = 0;
        hook.results.tags = self.tags.clone().unwrap_or_default();
        hook.results.calls = 0;
        for (address, value) in [
            (
                FONT + 8,
                if self.implementation_present {
                    IMPLEMENTATION
                } else {
                    0
                },
            ),
            (IMPLEMENTATION, IMPLEMENTATION_VTABLE),
            (
                IMPLEMENTATION + 8,
                if self.source_present { SOURCE } else { 0 },
            ),
            (SOURCE, SOURCE_VTABLE),
            (SOURCE + 112, if self.tags.is_some() { TYPEFACE } else { 0 }),
            (IMPLEMENTATION_VTABLE + 48, TEXT + 0x877f8),
            (IMPLEMENTATION_VTABLE + 56, TEXT + 0x87814),
            (IMPLEMENTATION_VTABLE + 80, TEXT + 0x87834),
            (SOURCE_VTABLE + 80, TEXT + 0x88ad4),
        ] {
            write(machine.engine, address, &value.to_le_bytes());
        }
        write(machine.engine, SOURCE + 120, &self.source_id.to_le_bytes());
        let language = IMPLEMENTATION + 32;
        assert!(self.language.len() <= 64);
        if self.allocated_language {
            for (address, value) in [
                (language, 81_u64),
                (language + 8, self.language.len() as u64),
                (language + 16, LANGUAGE_BYTES),
            ] {
                write(machine.engine, address, &value.to_le_bytes());
            }
            write(machine.engine, LANGUAGE_BYTES, self.language.as_bytes());
            write(
                machine.engine,
                LANGUAGE_BYTES + self.language.len() as u64,
                &[0],
            );
        } else {
            assert!(self.language.len() < 23);
            write(machine.engine, language, &[(self.language.len() * 2) as u8]);
            write(machine.engine, language + 1, self.language.as_bytes());
        }
        let source_bitmap = machine.call(TEXT + 0x88ae4, &[SOURCE]) != 0;
        let implementation_bitmap = machine.call(TEXT + 0x87814, &[IMPLEMENTATION]) != 0;
        let font_bitmap = machine.call(TEXT + 0x85d98, &[FONT]) != 0;
        let source_id = machine.call(TEXT + 0x88ad4, &[SOURCE]) as u32 as i32;
        let implementation_id = machine.call(TEXT + 0x877f8, &[IMPLEMENTATION]) as u32 as i32;
        let font_id = machine.call(TEXT + 0x85d7c, &[FONT]) as u32 as i32;
        let implementation_language = read_string(
            machine.engine,
            machine.call(TEXT + 0x87834, &[IMPLEMENTATION]),
        );
        let font_language = read_string(machine.engine, machine.call(TEXT + 0x85db0, &[FONT]));
        let scans = usize::from(self.tags.is_some())
            * (1 + usize::from(self.source_present)
                + usize::from(self.source_present && self.implementation_present));
        assert_eq!(hook.results.calls, scans);
        assert_eq!(machine.heap.allocations, scans);
        assert_eq!(machine.heap.deletes, scans);
        assert_eq!(machine.heap.cursor, HEAP + scans as u64 * 128);
        format!(
            "{{\"name\":{:?},\"table_tags\":{},\"source_present\":{},\"implementation_present\":{},\"source_id_input\":{},\"language_input\":{:?},\"allocated_language\":{},\"bitmap\":[{source_bitmap},{implementation_bitmap},{font_bitmap}],\"source_ids\":[{source_id},{implementation_id},{font_id}],\"languages\":[{implementation_language:?},{font_language:?}],\"table_tag_calls\":{scans}}}",
            self.name,
            self.tags
                .as_ref()
                .map_or_else(|| "null".into(), |tags| format!("{tags:?}")),
            self.source_present,
            self.implementation_present,
            self.source_id,
            self.language,
            self.allocated_language
        )
    }
}

pub(super) fn capture(machine: &mut Machine, text: &Path) {
    map_library(machine.engine, text, TEXT, TEXT_SHA256);
    for (plt, target) in [(0xeebc0, NEW), (0xeebd0, DELETE), (0xf0420, TEXT + 0x88ae4)] {
        bind_native(machine.engine, TEXT + plt, target);
    }
    let mut hook = TagHook::new(machine);
    let cbdt = u32::from_be_bytes(*b"CBDT");
    let colr = u32::from_be_bytes(*b"COLR");
    let svg = u32::from_be_bytes(*b"SVG ");
    let sbix = u32::from_be_bytes(*b"sbix");
    let head = u32::from_be_bytes(*b"head");
    let variants = [
        ("null-typeface", None),
        ("empty", Some(vec![])),
        ("cbdt-only", Some(vec![cbdt])),
        ("cbdt-first", Some(vec![cbdt, head, colr])),
        ("cbdt-middle", Some(vec![head, cbdt, svg])),
        ("cbdt-last", Some(vec![head, sbix, cbdt])),
        ("colr-only", Some(vec![colr])),
        ("svg-only", Some(vec![svg])),
        ("sbix-only", Some(vec![sbix])),
        ("other-color-tables", Some(vec![colr, svg, sbix])),
        (
            "cbdt-at-capacity",
            Some(
                (0..32)
                    .map(|index| if index == 31 { cbdt } else { head })
                    .collect(),
            ),
        ),
        ("no-cbdt-at-capacity", Some(vec![head; 32])),
    ];
    let mut cases = Vec::new();
    for (name, tags) in variants {
        for (
            suffix,
            source_present,
            implementation_present,
            source_id,
            language,
            allocated_language,
        ) in [
            ("empty-language", true, true, 7, "", false),
            ("deva-language", true, true, 0, "und-Deva", false),
            (
                "allocated-language",
                true,
                true,
                i32::MAX,
                "und-Deva-extra-long-language",
                true,
            ),
            ("null-source", false, true, -1, "und-Deva", true),
            ("null-implementation", true, false, -1, "und-Deva", false),
        ] {
            let case = Case {
                name: format!("{name}-{suffix}"),
                tags: tags.clone(),
                source_present,
                implementation_present,
                source_id,
                language,
                allocated_language,
            };
            let expected = case.fixture(machine, &mut hook, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(
                    case.fixture(machine, &mut hook, fill),
                    expected,
                    "allocation fill changed {}",
                    case.name
                );
            }
            cases.push(expected);
        }
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"text_library_sha256\":\"{TEXT_SHA256}\",\"bitmap_font_address\":\"0x88ae4\",\"font_bitmap_address\":\"0x85d98\",\"implementation_bitmap_address\":\"0x87814\",\"font_source_id_address\":\"0x85d7c\",\"implementation_source_id_address\":\"0x877f8\",\"source_id_address\":\"0x88ad4\",\"font_language_address\":\"0x85db0\",\"implementation_language_address\":\"0x87834\",\"result_order\":{{\"bitmap\":[\"source\",\"implementation\",\"font\"],\"source_ids\":[\"source\",\"implementation\",\"font\"],\"languages\":[\"implementation\",\"font\"]}},\"supplied_inputs\":\"cached source metadata, language strings, interface vtables and typeface table-tag results; no font factory or shaping\",\"cases\":[\n{}\n]}}",
        cases.join(",\n")
    );
}
