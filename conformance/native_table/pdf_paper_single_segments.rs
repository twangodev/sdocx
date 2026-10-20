use super::*;

const COMPOSER: u64 = 0x0100_0000;
const BASE: u64 = 0x0500_0000;
const DATA: u64 = 0x0900_0000;
const DATA_SIZE: u64 = 0x1_0000;
const LIST: u64 = DATA + 0x1000;
const RECORD: u64 = DATA + 0x2000;
const BUILDER: u64 = DATA + 0x3000;
const BODY_CONTEXT: u64 = DATA + 0x4000;
const EXPORTER: u64 = DATA + 0x5000;
const NOTE: u64 = DATA + 0x6000;
const PAGE: u64 = DATA + 0x7000;
const STUB: u64 = 0x0800_0000;
const COUNT: u64 = STUB;
const GET: u64 = STUB + 0x20;
const NOTE_HEIGHT: u64 = STUB + 0x40;
const BODY_TEXT: u64 = STUB + 0x60;
const OBJECTS: u64 = STUB + 0x80;
const LOG: u64 = STUB + 0xa0;
const BLANK_RESULT: u64 = COMPOSER + 0x35dfe4;

const NOTE_WIDTH: i32 = 1812;
const DEFAULT_HEIGHT: i32 = 1527;
const NOTE_HEIGHT_VALUE: i32 = 15372;
const SOURCE_RECT: [f32; 4] = [0.0, 0.0, 1812.0, 1485.0];
const STROKE_BOUNDS: [f32; 4] = [
    114.0684814453125,
    1557.342529296875,
    1758.197265625,
    10163.7861328125,
];

fn assert_heap_range(machine: &Machine, start: u64, end: u64, label: &str) {
    assert!(
        start <= end,
        "{label} range is reversed: {start:#x}..{end:#x}"
    );
    assert!(
        start % 8 == 0 && end % 8 == 0,
        "{label} range is not 8-byte aligned"
    );
    assert!(
        end - start <= 0x10000,
        "{label} range exceeds capture bound"
    );
    if start != end {
        assert!(
            start >= machine.heap.base && end <= machine.heap.limit,
            "{label} range escaped native heap arena: {start:#x}..{end:#x}"
        );
    }
}

struct Calls {
    pdf_list_count: usize,
    pdf_list_indices: Vec<i64>,
    note_height: usize,
    body_queries: usize,
    object_rects: Vec<[f32; 4]>,
    blank_results: Vec<u64>,
    log_calls: usize,
}

unsafe extern "C" fn callback(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let calls = unsafe { &mut *data.cast::<Calls>() };
    match address {
        COUNT => {
            calls.pdf_list_count += 1;
            register(engine, REGISTER_X0, 1);
        }
        GET => {
            let index = read_register(engine, REGISTER_X0 + 1) as i64;
            calls.pdf_list_indices.push(index);
            register(engine, REGISTER_X0, if index == 0 { RECORD } else { 0 });
        }
        NOTE_HEIGHT => {
            calls.note_height += 1;
            register(engine, REGISTER_X0, NOTE_HEIGHT_VALUE as u64);
        }
        BODY_TEXT => {
            calls.body_queries += 1;
            register(engine, REGISTER_X0, 0);
        }
        OBJECTS => {
            let rect = std::array::from_fn(|index| {
                f32::from_bits(read_register(engine, 136 + index as i32) as u32)
            });
            calls.object_rects.push(rect);
            let intersects = rect[0] <= STROKE_BOUNDS[2]
                && rect[2] >= STROKE_BOUNDS[0]
                && rect[1] <= STROKE_BOUNDS[3]
                && rect[3] >= STROKE_BOUNDS[1];
            register(engine, REGISTER_X0, u64::from(intersects));
        }
        BLANK_RESULT => calls.blank_results.push(read_register(engine, REGISTER_X0)),
        LOG => {
            calls.log_calls += 1;
            register(engine, REGISTER_X0, 0);
        }
        _ => panic!("unexpected SingleX host call {address:#x}"),
    }
}

fn install(machine: &Machine, address: u64, data: *mut c_void) {
    write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
    let mut hook = 0;
    check(unsafe {
        uc_hook_add(
            machine.engine,
            &mut hook,
            4,
            callback as *mut c_void,
            data,
            address,
            address,
        )
    });
}

fn observe(machine: &Machine, address: u64, data: *mut c_void) {
    let mut hook = 0;
    check(unsafe {
        uc_hook_add(
            machine.engine,
            &mut hook,
            4,
            callback as *mut c_void,
            data,
            address,
            address,
        )
    });
}

pub(super) fn capture(machine: &mut Machine, composer: &Path, base: &Path) {
    map_library(
        machine.engine,
        base,
        BASE,
        "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    );
    map_library(
        machine.engine,
        composer,
        COMPOSER,
        "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    );
    assert!(DATA >= 0x0900_0000 && DATA + DATA_SIZE <= 0x0901_0000);
    let host_objects = [
        ("PDF list", LIST, 0x40_u64),
        ("PDF record", RECORD, 0x40),
        ("segment builder", BUILDER, 0x100),
        ("body context", BODY_CONTEXT, 0x80),
        ("exporter", EXPORTER, 0x180),
        ("note", NOTE, 0x100),
        ("page", PAGE, 0x100),
    ];
    for &(name, start, size) in &host_objects {
        assert!(
            start >= DATA && start + size <= DATA + DATA_SIZE,
            "{name} escaped host data arena"
        );
    }
    for left in 0..host_objects.len() {
        let (left_name, left_start, left_size) = host_objects[left];
        for &(right_name, right_start, right_size) in &host_objects[left + 1..] {
            assert!(
                left_start + left_size <= right_start || right_start + right_size <= left_start,
                "host object ranges overlap: {left_name} and {right_name}"
            );
        }
    }
    check(unsafe { uc_mem_map(machine.engine, DATA, DATA_SIZE, 7) });
    check(unsafe { uc_mem_map(machine.engine, STUB, 0x1000, 7) });

    let mut calls = Box::new(Calls {
        pdf_list_count: 0,
        pdf_list_indices: Vec::new(),
        note_height: 0,
        body_queries: 0,
        object_rects: Vec::new(),
        blank_results: Vec::new(),
        log_calls: 0,
    });
    let data = ptr::from_mut(calls.as_mut()).cast();
    for address in [COUNT, GET, NOTE_HEIGHT, BODY_TEXT, OBJECTS, LOG] {
        install(machine, address, data);
    }
    observe(machine, BLANK_RESULT, data);

    // Executed native call edges are explicitly wired; a reached, unbound import
    // remains an emulator failure instead of silently returning a default value.
    for (plt, target) in [
        (COMPOSER + 0x54f290, COUNT),
        (COMPOSER + 0x54f2f0, GET),
        (COMPOSER + 0x54f630, NOTE_HEIGHT),
        (COMPOSER + 0x5519d0, BODY_TEXT),
        (COMPOSER + 0x5519c0, BODY_TEXT),
        (COMPOSER + 0x551ab0, OBJECTS),
        (COMPOSER + 0x551a70, COMPOSER + 0x35dee8),
        (COMPOSER + 0x5519b0, COMPOSER + 0x35fa0c),
        (COMPOSER + 0x551c90, BASE + 0xb127c),
        (COMPOSER + 0x54d840, LOG),
        (COMPOSER + 0x54d850, NEW),
        (COMPOSER + 0x54d870, DELETE),
    ] {
        bind_native(machine.engine, plt, target);
    }

    // Frozen retained quiz input: source page 0, PDFData rect [0,0,1812,1485].
    // The single-record branch never compares a source-identity String.
    write(machine.engine, LIST, &[0; 64]);
    write(machine.engine, RECORD, &[0; 64]);
    write(machine.engine, RECORD + 20, &0_i32.to_le_bytes());
    for (index, value) in SOURCE_RECT.iter().enumerate() {
        write(
            machine.engine,
            RECORD + 24 + index as u64 * 4,
            &value.to_bits().to_le_bytes(),
        );
    }
    write(machine.engine, BODY_CONTEXT, &[0; 0x80]);
    write(machine.engine, NOTE, &[0; 0x100]);
    write(machine.engine, NOTE, &NOTE_HEIGHT_VALUE.to_le_bytes());
    write(machine.engine, PAGE, &[0; 0x100]);
    write(machine.engine, EXPORTER, &[0; 0x180]);
    write(machine.engine, EXPORTER + 80, &NOTE.to_le_bytes());
    write(machine.engine, EXPORTER + 88, &[0]);
    write(machine.engine, EXPORTER + 224, &PAGE.to_le_bytes());
    write(machine.engine, EXPORTER + 232, &NOTE_WIDTH.to_le_bytes());
    write(machine.engine, EXPORTER + 256, &BODY_CONTEXT.to_le_bytes());
    write(machine.engine, EXPORTER + 312, &BUILDER.to_le_bytes());

    // The builder receives the actual note width, retained default page height,
    // one retained PDFData row, and a bounded BodyTextContext adapter.
    machine.call(
        COMPOSER + 0x35f940,
        &[
            BUILDER,
            NOTE_WIDTH as u64,
            DEFAULT_HEIGHT as u64,
            LIST,
            BODY_CONTEXT,
        ],
    );
    machine.call(COMPOSER + 0x357678, &[EXPORTER, DEFAULT_HEIGHT as u64]);
    machine.call(COMPOSER + 0x35db00, &[EXPORTER]);

    let count = machine.call(COMPOSER + 0x3601dc, &[BUILDER]) as usize;
    let start = read_u64(machine.engine, BUILDER + 24);
    let end = read_u64(machine.engine, BUILDER + 32);
    assert!(end >= start && (end - start) % 8 == 0);
    assert_heap_range(machine, start, end, "segment pointer vector");
    assert_eq!((end - start) / 8, count as u64);
    assert!(count <= 16, "native segment count exceeds capture bound");
    let mut segments = Vec::new();
    for index in 0..count {
        let segment = read_u64(machine.engine, start + index as u64 * 8);
        assert_heap_range(machine, segment, segment + 48, "segment object");
        let record_begin = read_u64(machine.engine, segment + 8);
        // libc++ vector stores begin/end/capacity at +8/+16/+24.
        // Capacity can exceed the number of initialized record pointers.
        let record_end = read_u64(machine.engine, segment + 16);
        let record_capacity = read_u64(machine.engine, segment + 24);
        assert!(record_begin <= record_end && record_end <= record_capacity);
        assert_eq!((record_end - record_begin) % 8, 0);
        assert_heap_range(
            machine,
            record_begin,
            record_end,
            "segment PDF-record vector",
        );
        let record_count = (record_end - record_begin) / 8;
        assert!(
            record_count <= 1,
            "native segment has unexpected PDF-record count"
        );
        let records = (record_begin..record_end)
            .step_by(8)
            .map(|slot| {
                let pointer = read_u64(machine.engine, slot);
                assert_eq!(
                    pointer, RECORD,
                    "native PDF-record pointer differs from retained quiz input"
                );
                "\"retained_pdf_record_0\"".to_owned()
            })
            .collect::<Vec<_>>();
        let slice_begin = read_u64(machine.engine, segment + 32);
        let slice_end = read_u64(machine.engine, segment + 40);
        assert_heap_range(
            machine,
            slice_begin,
            slice_end,
            "segment offset/height vector",
        );
        assert!(
            (slice_end - slice_begin) / 8 <= 32,
            "native slice count exceeds capture bound"
        );
        let slices = (slice_begin..slice_end)
            .step_by(8)
            .map(|slot| read_u64(machine.engine, slot))
            .map(|pair| [pair as u32 as i32, (pair >> 32) as u32 as i32])
            .collect::<Vec<_>>();
        segments.push(format!(
            "{{\"output_page_index\":{},\"source_record_pointers\":[{}],\"offset_height_pairs\":{:?}}}",
            read_u32(machine.engine, segment) as i32,
            records.join(","),
            slices
        ));
    }
    assert_eq!(count, 2);
    assert_eq!(calls.pdf_list_indices, [0]);
    assert_eq!(calls.blank_results.len(), 11);
    assert_eq!(calls.blank_results[..10], [0; 10]);
    assert_eq!(calls.blank_results[10], 1);
    assert_eq!(segments[0].matches("retained_pdf_record_0").count(), 1);
    assert_eq!(segments[1].matches("retained_pdf_record_0").count(), 0);

    let last_height = machine.call(COMPOSER + 0x3601a0, &[BUILDER]);
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"source_note_sha256\":\"38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a\",\"source_page_id\":\"91c795fe-3d67-11ee-8ec8-e76e383a352a\",\"source_pdf_sha256\":\"f01243e2d712d47ad08735ec4e47da2a94974818f2cd7c28da2db3ee326c4279\",\"composer_sha256\":\"52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f\",\"base_sha256\":\"e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb\",\"native_calls\":{{\"SetDefaultHeight\":\"0x357678\",\"buildPageSegments\":\"0x35db00\",\"AccumulatePage\":\"0x35fa0c\",\"isLastPageBlank\":\"0x35dee8\",\"RectF_IsIntersect\":\"Base+0xb127c\"}},\"retained_quiz_input\":{{\"default_page_dimensions\":[1080,1527],\"note_width\":{NOTE_WIDTH},\"note_height\":{NOTE_HEIGHT_VALUE},\"pdf_record\":{{\"source_page_index\":0,\"note_rect\":{:?}}},\"stroke_bounds\":[114.0684814453125,1557.342529296875,1758.197265625,10163.7861328125]}},\"native_set_default_height\":{DEFAULT_HEIGHT},\"native_segment_count\":{count},\"last_native_segment_height\":{last_height},\"host_calls\":{{\"pdf_list_count\":{},\"pdf_list_get_indices\":{:?},\"note_height\":{},\"body_text_queries\":{},\"final_page_object_rects\":{:?},\"native_is_last_blank_results\":{:?},\"android_log\":{}}},\"segments\":[{}],\"host_boundaries\":[\"ArrayList::GetCount/Get return the one frozen retained PDFData record\",\"WNote::GetHeight returns retained quiz canvas height 15372\",\"BodyTextContext::HasVisibleText and HasText return false; no BodyText measurement runs\",\"final-page hasObjectDrawingByRect adapter intersects the retained full-note aggregate stroke bounds, not per-object geometry\",\"Android log is a no-op diagnostic callback; any unbound reached import fails the emulator run\"],\"boundary\":\"Pinned native PdfPageSegmentBuilder and NotePDFExporterRasterSingleX::buildPageSegments/isLastPageBlank execute over quiz retained record and dimensions. No native exporter construction/initialization, PDF loading/import, capturePages, page writer, emitted PDF, or pixels execute. The final blank-page object query uses aggregate note stroke bounds.\"}}",
        SOURCE_RECT,
        calls.pdf_list_count,
        calls.pdf_list_indices,
        calls.note_height,
        calls.body_queries,
        calls.object_rects,
        calls.blank_results,
        calls.log_calls,
        segments.join(",")
    );
}
