use super::*;
use cell_model_lifecycle::Paths;

const BODY: u64 = 0x0700_0000;
const WIDGET: u64 = 0x0400_0000;
const VIEW: u64 = MODEL + 0x19000;
const OWNER: u64 = MODEL + 0x1a000;
const BUCKETS: u64 = MODEL + 0x1b000;
const OWNER_NODE: u64 = MODEL + 0x1b100;
const GROUP: u64 = MODEL + 0x1d000;
const DOCUMENT: u64 = MODEL + 0x1e000;
const COMPONENT_IMPL: u64 = MODEL + 0x1f000;
const TEXT_COMMON: u64 = MODEL + 0x20000;
const TEXT_IMPL: u64 = MODEL + 0x21000;
const SPAN_VECTOR: u64 = MODEL + 0x22000;
const SPAN: u64 = MODEL + 0x22100;
const TEXT_WRAPPER: u64 = MODEL + 0x23000;
const DOCUMENT_LAYOUT: u64 = MODEL + 0x24000;
const DOCUMENT_VTABLE: u64 = MODEL + 0x25000;
const SETUP_STACK: u64 = MODEL + 0x26000;
const INPUT_RECT: u64 = MODEL + 0x18000;
const DOCUMENT_RECT_GETTER: u64 = 0x0300_0e00;

#[derive(Default)]
struct Trace {
    supplied_rect: [f32; 4],
    span_index: u32,
    addresses: Vec<u64>,
    getters: u32,
}

unsafe extern "C" fn trace(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let trace = unsafe { &mut *data.cast::<Trace>() };
    trace.addresses.push(address);
    if address == DOCUMENT_RECT_GETTER {
        assert_eq!(read_register(engine, REGISTER_X0), DOCUMENT_LAYOUT);
        assert_eq!(
            read_register(engine, REGISTER_X0 + 1) as u32,
            trace.span_index
        );
        trace.getters += 1;
        floats(engine, trace.supplied_rect);
    }
}

fn floats(engine: Engine, values: [f32; 4]) {
    for (index, value) in values.into_iter().enumerate() {
        register(engine, 136 + index as i32, u64::from(value.to_bits()));
    }
}

fn rect(engine: Engine, address: u64) -> [f32; 4] {
    std::array::from_fn(|index| read_float(engine, address + index as u64 * 4))
}

fn put_rect(engine: Engine, address: u64, values: [f32; 4]) {
    for (index, value) in values.into_iter().enumerate() {
        write(engine, address + index as u64 * 4, &value.to_le_bytes());
    }
}

fn put(engine: Engine, address: u64, value: u64) {
    write(engine, address, &value.to_le_bytes());
}

fn run_setup_range(machine: &Machine, start: u64, end: u64) {
    register(machine.engine, REGISTER_SP, SETUP_STACK);
    check(unsafe {
        uc_emu_start(
            machine.engine,
            BODY + start,
            BODY + end,
            machine.call_timeout_micros,
            machine.call_instruction_limit,
        )
    });
    assert_eq!(read_register(machine.engine, 260), BODY + end);
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    document_rect: [f32; 4],
    change: f32,
    index: u32,
    clean: bool,
    divergent: bool,
    match_cell: bool,
    merged: bool,
    owner_present: bool,
    callback_present: bool,
    warm_repeat: bool,
}

impl Case {
    fn snapshot(self, machine: &Machine) -> String {
        let mut cells = Vec::new();
        for row in 0..2 {
            for column in 0..2 {
                let cell = machine.call(0x3d2be0, &[TABLE_OBJECT, row, column]);
                let source = read_u64(machine.engine, cell + 88);
                let implementation = read_u64(machine.engine, source + 16);
                let data = read_u64(machine.engine, implementation + 24);
                machine.call(DRAWING_BASE + 0xab4bc, &[frames::LAYOUT, cell]);
                let frame: [f32; 4] = std::array::from_fn(|i| {
                    f32::from_bits(read_register(machine.engine, 136 + i as i32) as u32)
                });
                let flags = read_u32(machine.engine, cell + 80);
                let source_flags = read_u32(machine.engine, implementation + 240);
                cells.push(format!("{{\"cell_rect\":{:?},\"content_model_rect\":{:?},\"content_drawn_rect\":{:?},\"drawing_frame\":{frame:?},\"cell_changed\":{},\"cell_unsaved\":{},\"content_changed\":{},\"content_unsaved\":{}}}",rect(machine.engine,cell+64),rect(machine.engine,data+8),rect(machine.engine,data+24),(flags>>8)&255,(flags>>16)&255,source_flags&255,(source_flags>>8)&255));
            }
        }
        let table_impl = read_u64(machine.engine, TABLE_OBJECT + 104);
        let base_impl = read_u64(machine.engine, TABLE_OBJECT + 16);
        let table_data = read_u64(machine.engine, base_impl + 24);
        let table_flags = read_u32(machine.engine, base_impl + 240);
        format!(
            "{{\"table_model_rect\":{:?},\"table_drawn_rect\":{:?},\"table_changed\":{},\"table_unsaved\":{},\"table_content_rect\":{:?},\"drawing_content_rect\":{:?},\"cells\":[{}]}}",
            rect(machine.engine, table_data + 8),
            rect(machine.engine, table_data + 24),
            table_flags & 255,
            (table_flags >> 8) & 255,
            rect(machine.engine, table_impl + 16),
            rect(machine.engine, frames::LAYOUT + 652),
            cells.join(",")
        )
    }

    fn sample(self, paths: &Paths, widget: &Path, fill: u8) -> String {
        let machine = cell_model_lifecycle::initialize(paths, fill);
        map_library(machine.engine, widget, WIDGET, geometry::WIDGET_SHA256);
        bind_native(machine.engine, BODY + 0x1042b0, WIDGET + 0xd39ac);
        bind_native(machine.engine, DRAWING_BASE + 0xb8a30, 0x0300_0c00);
        put_rect(machine.engine, INPUT_RECT, [13.25, -19.5, 173.25, 180.5]);
        machine.call(0x3d2690, &[TABLE_OBJECT]);
        assert_eq!(machine.call(0x3d27d8, &[TABLE_OBJECT, INPUT_RECT, 2, 2]), 1);
        if self.merged {
            assert_eq!(machine.call(0x3d6038, &[TABLE_OBJECT, 0, 0, 1, 1]), 1);
        }
        let table_impl = read_u64(machine.engine, TABLE_OBJECT + 104);
        frames::configure_layout(&machine);
        put(machine.engine, TABLE_OBJECT + 104, table_impl);
        put(machine.engine, frames::LAYOUT, DRAWING_BASE + 0xc4b30);
        machine.call(DRAWING_BASE + 0xaa6b4, &[frames::LAYOUT]);
        machine.call(DRAWING_BASE + 0xab168, &[frames::LAYOUT]);
        floats(machine.engine, [self.change, 0.0, 0.0, 0.0]);
        machine.call(DRAWING_BASE + 0xaff74, &[frames::LAYOUT, 0]);
        floats(machine.engine, [self.change, 0.0, 0.0, 0.0]);
        machine.call(DRAWING_BASE + 0xade0c, &[frames::LAYOUT, 1]);
        machine.call(DRAWING_BASE + 0xab168, &[frames::LAYOUT]);
        let handle = machine.call(0x2cb0d0, &[TABLE_OBJECT]);
        let map = OWNER + 496;
        for (address, value) in [
            (VIEW + 1048, OWNER),
            (VIEW + 1056, GROUP),
            (map, BUCKETS),
            (map + 8, 1),
            (map + 16, OWNER_NODE),
            (map + 24, 1),
            (BUCKETS, map + 16),
            (OWNER_NODE + 8, handle),
            (OWNER_NODE + 24, frames::LAYOUT),
        ] {
            put(machine.engine, address, value);
        }
        write(
            machine.engine,
            OWNER_NODE + 16,
            &(handle as u32).to_le_bytes(),
        );
        write(machine.engine, map + 32, &1_f32.to_le_bytes());
        machine.call(0x417440, &[SPAN]);
        assert_eq!(machine.call(0x417fc4, &[SPAN, TABLE_OBJECT]), 1);
        machine.call(0x417598, &[SPAN, u64::from(self.index)]);
        machine.call(0x2d1f90, &[TABLE_OBJECT, 1]);
        for (address, value) in [
            (VIEW + 1008, DOCUMENT),
            (DOCUMENT + 40 + 16, COMPONENT_IMPL),
            (COMPONENT_IMPL + 8, TEXT_COMMON),
            (TEXT_COMMON + 8, TEXT_IMPL),
            (TEXT_IMPL + 384, SPAN_VECTOR),
            (TEXT_IMPL + 392, SPAN_VECTOR + 8),
            (TEXT_IMPL + 400, SPAN_VECTOR + 8),
            (SPAN_VECTOR, SPAN),
            (OWNER + 488, TEXT_WRAPPER),
            (TEXT_WRAPPER + 368, DOCUMENT_LAYOUT),
            (DOCUMENT_LAYOUT, DOCUMENT_VTABLE),
            (DOCUMENT_VTABLE + 112, DOCUMENT_RECT_GETTER),
        ] {
            put(machine.engine, address, value);
        }
        assert_eq!(machine.call(BODY + 0xd6d78, &[VIEW, TABLE_OBJECT]), SPAN);
        for (vtable, invoke) in [(0x10cea8, 0xb59d0), (0x10dcd8, 0xdce8c)] {
            assert_eq!(read_u64(machine.engine, BODY + vtable + 48), BODY + invoke);
        }
        bind_native(machine.engine, BODY + 0x104130, NEW);
        put(machine.engine, frames::LAYOUT + 496, frames::LAYOUT + 504);
        put(machine.engine, SETUP_STACK, frames::LAYOUT);
        register(machine.engine, REGISTER_X0 + 20, OWNER);
        run_setup_range(&machine, 0xb3228, 0xb3314);
        assert_eq!(read_u64(machine.engine, frames::LAYOUT + 512), 1);
        register(machine.engine, REGISTER_X0 + 19, VIEW);
        run_setup_range(&machine, 0xd080c, 0xd085c);
        assert_eq!(read_u64(machine.engine, OWNER + 272), OWNER + 240);
        if self.match_cell {
            for row in 0..2 {
                for column in 0..2 {
                    let cell = machine.call(0x3d2be0, &[TABLE_OBJECT, row, column]);
                    machine.call(DRAWING_BASE + 0xab4bc, &[frames::LAYOUT, cell]);
                    let frame: [f32; 4] = std::array::from_fn(|i| {
                        f32::from_bits(read_register(machine.engine, 136 + i as i32) as u32)
                    });
                    put_rect(machine.engine, INPUT_RECT, frame);
                    floats(machine.engine, self.document_rect);
                    machine.call(frames::BASE + 0xb11a4, &[INPUT_RECT]);
                    floats(machine.engine, rect(machine.engine, INPUT_RECT));
                    assert_eq!(machine.call(0x3c20cc, &[cell]), 1);
                }
            }
        }
        if self.divergent {
            let cell = machine.call(0x3d2be0, &[TABLE_OBJECT, 0, 0]);
            let source = read_u64(machine.engine, cell + 88);
            floats(machine.engine, [-40.5, -41.25, -20.5, -11.25]);
            assert_eq!(machine.call(0x399954, &[source]), 1);
        }
        if self.clean {
            for row in 0..2 {
                for column in 0..2 {
                    let cell = machine.call(0x3d2be0, &[TABLE_OBJECT, row, column]);
                    machine.call(0x3c28e4, &[cell]);
                }
            }
        }
        let size_gate = machine.call(BODY + 0xd76e0, &[VIEW, TABLE_OBJECT]);
        if !self.owner_present {
            put(machine.engine, map + 24, 0);
            put(machine.engine, map + 16, 0);
            put(machine.engine, BUCKETS, 0);
        }
        if !self.callback_present {
            put(machine.engine, OWNER + 272, 0);
        }
        let before = self.snapshot(&machine);
        write(
            machine.engine,
            DOCUMENT_RECT_GETTER,
            &0xd65f03c0_u32.to_le_bytes(),
        );
        let mut observation = Trace {
            supplied_rect: self.document_rect,
            span_index: self.index,
            ..Trace::default()
        };
        for address in [
            DOCUMENT_RECT_GETTER,
            BODY + 0xb59b4,
            BODY + 0xb59c8,
            BODY + 0xb59d0,
            BODY + 0xdce8c,
            BODY + 0xf1938,
            BODY + 0xd6d78,
            WIDGET + 0xd39ac,
            0x41765c,
            BODY + 0xd76e0,
            BODY + 0xd78ec,
        ] {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    trace as *mut c_void,
                    (&mut observation as *mut Trace).cast(),
                    address,
                    address,
                )
            });
        }
        machine.call(DRAWING_BASE + 0xb1bf0, &[frames::LAYOUT]);
        let after = self.snapshot(&machine);
        let mut repeated = "null".to_owned();
        if self.warm_repeat {
            machine.call(DRAWING_BASE + 0xb1bf0, &[frames::LAYOUT]);
            repeated = self.snapshot(&machine);
        }
        let route = observation
            .addresses
            .iter()
            .map(|address| format!("\"0x{address:x}\""))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"name\":{:?},\"document_rect\":{:?},\"span_index\":{},\"row_change\":{:?},\"initially_clean\":{},\"divergent_source\":{},\"cell_matches_target\":{},\"merged\":{},\"owner_present\":{},\"view_callback_present\":{},\"repeat_notification\":{},\"prepared_size_gate\":{},\"document_getter_calls\":{},\"executed_route\":[{}],\"before\":{},\"after\":{},\"repeated\":{}}}",
            self.name,
            self.document_rect,
            self.index,
            self.change,
            self.clean,
            self.divergent,
            self.match_cell,
            self.merged,
            self.owner_present,
            self.callback_present,
            self.warm_repeat,
            size_gate,
            observation.getters,
            route,
            before,
            after,
            repeated
        )
    }
}

pub(super) fn capture(paths: Paths, widget: &Path) {
    let base = Case {
        name: "unchanged",
        document_rect: [13.25, -19.5, 173.25, 180.5],
        change: 0.0,
        index: 7,
        clean: false,
        divergent: false,
        match_cell: false,
        merged: false,
        owner_present: true,
        callback_present: true,
        warm_repeat: false,
    };
    let cases = [
        base,
        Case {
            name: "origin-only",
            document_rect: [123.75, -31.125, 283.75, 168.875],
            ..base
        },
        Case {
            name: "zero-document-rect",
            document_rect: [0.0; 4],
            change: 0.5,
            ..base
        },
        Case {
            name: "below-size-gate",
            change: 0.0005,
            ..base
        },
        Case {
            name: "above-size-gate",
            change: 0.002,
            ..base
        },
        Case {
            name: "grow",
            change: 0.5,
            ..base
        },
        Case {
            name: "shrink",
            change: -0.5,
            ..base
        },
        Case {
            name: "translated-origin",
            document_rect: [123.75, -31.125, 283.75, 168.875],
            change: 0.5,
            ..base
        },
        Case {
            name: "document-extent-independent",
            document_rect: [13.25, -19.5, 999.0, 777.0],
            change: 0.5,
            ..base
        },
        Case {
            name: "negative-span-index",
            index: u32::MAX,
            change: 0.5,
            ..base
        },
        Case {
            name: "clean-grow",
            clean: true,
            change: 0.5,
            ..base
        },
        Case {
            name: "merged-grow",
            merged: true,
            change: 0.5,
            ..base
        },
        Case {
            name: "equal-cell-divergent-content",
            change: 0.5,
            match_cell: true,
            divergent: true,
            ..base
        },
        Case {
            name: "clean-equal-cell-divergent-content",
            clean: true,
            change: 0.5,
            match_cell: true,
            divergent: true,
            ..base
        },
        Case {
            name: "changed-cell-divergent-content",
            change: 0.5,
            divergent: true,
            ..base
        },
        Case {
            name: "missing-owner-layout",
            change: 0.5,
            owner_present: false,
            ..base
        },
        Case {
            name: "missing-view-callback",
            change: 0.5,
            callback_present: false,
            ..base
        },
        Case {
            name: "warm-repeat",
            change: 0.5,
            warm_repeat: true,
            ..base
        },
    ];
    let fixtures = cases
        .into_iter()
        .map(|case| {
            let expected = case.sample(&paths, widget, 0);
            for fill in [85, 165, 255, 0] {
                assert_eq!(
                    case.sample(&paths, widget, fill),
                    expected,
                    "allocation fill {fill} changed {}",
                    case.name
                );
            }
            expected
        })
        .collect::<Vec<_>>();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,85,165,255],\"repeat_fill\":0,\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"bodytext_library_sha256\":\"27324ca3807f07e0c1d0647b23eb9af1296762a8c9d892ee486f37b1eb9543f0\",\"cpp_library_sha256\":\"4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4\",\"widget_library_sha256\":\"{}\",\"native_entrypoints\":{{\"table_constructor\":\"Model:0x3d2690\",\"table_construct\":\"Model:0x3d27d8\",\"notification\":\"Drawing:0xb1bf0\",\"listener_registration_range\":\"Bodytext:[0xb3228,0xb3314)\",\"view_installation_range\":\"Bodytext:[0xd080c,0xd085c)\",\"listener_clone_assignment\":\"Bodytext:0xb5714\",\"layout_forwarder\":\"Bodytext:0xb59d0\",\"view_callback\":\"Bodytext:0xdce8c\",\"span_lookup\":\"Bodytext:0xd6d78\",\"size_gate\":\"Bodytext:0xd76e0\",\"model_bridge\":\"Bodytext:0xd78ec\",\"span_constructor\":\"Model:0x417440\",\"span_set_object\":\"Model:0x417fc4\",\"span_set_index\":\"Model:0x417598\",\"document_text_layout_getter\":\"Widget:0xd39ac\"}},\"native_scope\":\"complete native notification, std::function clone/invoke/destroy, Bodytext forwarding and listener callback, runtime owner lookup and dynamic cast, native ObjectSpan construction, native span list copy/traversal and index getter, dimension gate and full Model rectangle bridge\",\"supplied_boundaries\":\"bounded zero-initialized table storage followed by actual native construction; supplied zero-initialized Drawing and Bodytext storage, owner association, empty listener tree, document ComponentText/TextCommon pointer chain and one-element span vector; native instruction ranges within newObjectLayout and initBodyTextLayout install the listener tree node and both closures, including allocation, balancing and std::function clone assignment; these ranges exclude the rest of both setup functions; document ITextLayout virtual slot112 returns the explicitly supplied RectF and verifies the native ObjectSpan index; nil page cache, shared controls, context, observers and followers; ObjectSpan index inputs are transported as native 32-bit values including signed -1; existing Drawing ObjectTextLayout allocation adapter omits SetObject/SetTextScale and native shaping; allocation fills vary native heap allocations, not supplied fixture storage\",\"host_boundaries\":[\"bounded allocation and free\",\"memory and byte string operations\",\"deterministic UUID service\",\"single-thread mutex and C++ guard services\",\"destructor registration\",\"Android logging and Error::SetError\",\"zeroed Drawing ObjectTextLayout storage adapter; SetObject and SetTextScale omitted\",\"document ITextLayout virtual slot112 RectF result\"],\"excluded_producers\":\"document parser, document text shaping and placement that produce slot112 rectangles, complete newObjectLayout/initBodyTextLayout setup outside captured instruction ranges, first cell text measurement, Composer clone and final export clip\",\"cases\":[\n{}\n]}}",
        frames::BASE_SHA256,
        geometry::WIDGET_SHA256,
        fixtures.join(",\n")
    );
}
