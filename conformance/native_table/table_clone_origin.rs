use super::*;

const WIDGET: u64 = 0x0800_0000;
const COMPOSER: u64 = 0x0900_0000;
const WIDGET_SHA256: &str = "cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9";
const COMPOSER_SHA256: &str = "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f";
const DRAWN_TEXT: u64 = MODEL + 0x29000;

pub(super) struct Paths<'a> {
    pub lifecycle: cell_model_lifecycle::Paths<'a>,
    pub widget: &'a Path,
    pub composer: &'a Path,
}
const INPUT: u64 = MODEL + 0x28000;

fn rect(machine: &Machine, address: u64) -> [f32; 4] {
    std::array::from_fn(|i| read_float(machine.engine, address + i as u64 * 4))
}
fn snapshot(machine: &Machine, table: u64, stage: &str, prepared: bool) -> String {
    let implementation = read_u64(machine.engine, table + 16);
    let data = read_u64(machine.engine, implementation + 24);
    let table_impl = read_u64(machine.engine, table + 104);
    let vtable = read_u64(machine.engine, table);
    machine.call(read_u64(machine.engine, vtable + 160), &[table]);
    let drawn: [f32; 4] = std::array::from_fn(|i| {
        f32::from_bits(read_register(machine.engine, 136 + i as i32) as u32)
    });
    let cells = (0..4).map(|i| {
        let cell = machine.call(0x3d2be0, &[table, i / 2, i % 2]);
        let content = read_u64(machine.engine, cell + 88);
        let content_data = read_u64(machine.engine, read_u64(machine.engine, content + 16) + 24);
        let frame = if prepared {
            machine.call(DRAWING_BASE + 0xab4bc, &[frames::LAYOUT, cell]);
            format!("{:?}", std::array::from_fn::<_, 4, _>(|axis| f32::from_bits(read_register(machine.engine, 136 + axis as i32) as u32)))
        } else { "null".into() };
        format!("{{\"drawing_frame\":{frame},\"position\":[{},{}],\"saved_rect\":{:?},\"content_model_rect\":{:?},\"content_drawn_rect\":{:?}}}", i / 2, i % 2, rect(machine, cell + 64), rect(machine, content_data + 8), rect(machine, content_data + 24))
    }).collect::<Vec<_>>();
    format!(
        "{{\"stage\":{stage:?},\"spannable_over_pages\":{},\"model_rect\":{:?},\"base_data_drawn_rect\":{:?},\"virtual_drawn_rect\":{drawn:?},\"content_rect\":{:?},\"vtable_slots\":[{},{},{}],\"cells\":[{}]}}",
        machine.call(0x2d2208, &[table]) != 0,
        rect(machine, data + 8),
        rect(machine, data + 24),
        rect(machine, table_impl + 16),
        read_u64(machine.engine, read_u64(machine.engine, table) + 40),
        read_u64(machine.engine, read_u64(machine.engine, table) + 184),
        read_u64(machine.engine, read_u64(machine.engine, table) + 480),
        cells.join(",")
    )
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    source: [f32; 4],
    drawn_run: [f32; 4],
    position: [f32; 2],
    writer_origin: [f32; 2],
    spannable: bool,
    merged: bool,
}

impl Case {
    fn standard(name: &'static str) -> Self {
        Self {
            name,
            source: [13.25, -19.5, 173.25, 180.5],
            drawn_run: [0.0, 0.0, 161.0, 201.0],
            position: [0.0; 2],
            writer_origin: [123.25, 169.375],
            spannable: false,
            merged: false,
        }
    }
}

fn fixture(paths: &Paths<'_>, case: Case, fill: u8) -> String {
    let machine = cell_model_lifecycle::initialize(&paths.lifecycle, fill);
    map_library(machine.engine, paths.widget, WIDGET, WIDGET_SHA256);
    map_library(machine.engine, paths.composer, COMPOSER, COMPOSER_SHA256);
    for (plt, target) in [
        (0xea330, WIDGET + 0xe230c),
        (0xe6780, frames::BASE + 0xbe518),
        (0xe72a0, frames::BASE + 0xb108c),
        (0xe6950, frames::BASE + 0xb109c),
        (0xea4e0, 0x2c9fec),
        (0xe66c0, frames::BASE + 0xbe63c),
        (0xe6840, frames::BASE + 0xb10e0),
        (0xe6810, frames::BASE + 0xb1658),
    ] {
        bind_native(machine.engine, WIDGET + plt, target);
    }
    bind_native(machine.engine, COMPOSER + 0x552ec0, WIDGET + 0xe1f88);
    bind_native(machine.engine, COMPOSER + 0x54f2e0, frames::BASE + 0xb11a4);
    bind_native(machine.engine, COMPOSER + 0x376478, STOP);
    for (plt, target) in [(0x552ee0, 0x2d2208_u64), (0x552ed0, 0x2d21f4)] {
        write(
            machine.engine,
            COMPOSER + plt,
            &0x58000050_u32.to_le_bytes(),
        );
        write(
            machine.engine,
            COMPOSER + plt + 4,
            &0xd61f0200_u32.to_le_bytes(),
        );
        write(machine.engine, COMPOSER + plt + 8, &target.to_le_bytes());
    }
    bind_native(machine.engine, COMPOSER + 0x3764a8, STOP);
    let bounds = case.source;
    for (i, value) in bounds.into_iter().enumerate() {
        write(machine.engine, INPUT + i as u64 * 4, &value.to_le_bytes());
    }
    machine.call(0x3d2690, &[TABLE_OBJECT]);
    assert_eq!(machine.call(0x3d27d8, &[TABLE_OBJECT, INPUT, 2, 2]), 1);
    if case.merged {
        assert_eq!(machine.call(0x3d6038, &[TABLE_OBJECT, 0, 0, 1, 1]), 1);
    }
    machine.call(0x2d21f4, &[TABLE_OBJECT, u64::from(case.spannable)]);
    let source = snapshot(&machine, TABLE_OBJECT, "source", false);
    let clone = machine.call(0x36d6cc, &[22, 0]);
    assert_ne!(clone, 0);
    let vtable = read_u64(machine.engine, clone);
    let copy = read_u64(machine.engine, vtable + 184);
    assert_eq!(copy, 0x3d9af4);
    assert_eq!(machine.call(copy, &[clone, TABLE_OBJECT]), 1);
    let copied = snapshot(&machine, clone, "copied", false);
    for (i, value) in case.drawn_run.into_iter().enumerate() {
        write(
            machine.engine,
            DRAWN_TEXT + 88 + i as u64 * 4,
            &value.to_le_bytes(),
        );
    }
    for (offset, value) in [(80, case.position[0]), (84, case.position[1])] {
        write(machine.engine, DRAWN_TEXT + offset, &value.to_le_bytes());
    }
    machine.call(WIDGET + 0xe1f88, &[TABLE_OBJECT, DRAWN_TEXT + 88]);
    let local_raw: [f32; 4] = std::array::from_fn(|i| {
        f32::from_bits(read_register(machine.engine, 136 + i as i32) as u32)
    });
    register(machine.engine, REGISTER_X0 + 19, DRAWN_TEXT);
    register(machine.engine, REGISTER_X0 + 22, clone);
    register(machine.engine, REGISTER_X0 + 23, TABLE_OBJECT);
    register(
        machine.engine,
        144,
        u64::from(case.writer_origin[1].to_bits()),
    );
    register(
        machine.engine,
        145,
        u64::from(case.writer_origin[0].to_bits()),
    );
    machine.call(COMPOSER + 0x376418, &[]);
    register(machine.engine, REGISTER_X0 + 23, TABLE_OBJECT);
    register(machine.engine, REGISTER_X0 + 22, clone);
    machine.call(COMPOSER + 0x376494, &[]);
    let placed = snapshot(&machine, clone, "placed", false);
    machine.call(read_u64(machine.engine, vtable + 160), &[clone]);
    let clone_drawn: [f32; 4] = std::array::from_fn(|i| {
        f32::from_bits(read_register(machine.engine, 136 + i as i32) as u32)
    });
    let table_impl = read_u64(machine.engine, TABLE_OBJECT + 104);
    frames::configure_layout(&machine);
    write(
        machine.engine,
        TABLE_OBJECT + 104,
        &table_impl.to_le_bytes(),
    );
    write(machine.engine, frames::LAYOUT + 584, &clone.to_le_bytes());
    write(
        machine.engine,
        frames::LAYOUT,
        &(DRAWING_BASE + 0xc4b30).to_le_bytes(),
    );
    machine.call(DRAWING_BASE + 0xaa6b4, &[frames::LAYOUT]);
    machine.call(DRAWING_BASE + 0xab168, &[frames::LAYOUT]);
    let prepared = snapshot(&machine, clone, "cold_frames", true);
    let drawing = cell_drawing::draw_prepared(&machine, clone, frames::LAYOUT, clone_drawn[0]);
    format!(
        "{{\"name\":{:?},\"spannable\":{},\"merged\":{},\"source_bounds\":{:?},\"drawn_run\":{:?},\"run_position\":{:?},\"writer_origin\":{:?},\"local_raw_rect\":{local_raw:?},\"stages\":[{source},{copied},{placed},{prepared}],\"drawing\":{drawing}}}",
        case.name,
        case.spannable,
        case.merged,
        case.source,
        case.drawn_run,
        case.position,
        case.writer_origin
    )
}

pub(super) fn capture(paths: Paths<'_>) {
    let standard = Case::standard;
    let cases = [
        standard("translated-normal"),
        Case {
            spannable: true,
            name: "translated-over-pages",
            ..standard("")
        },
        Case {
            merged: true,
            name: "translated-merged-normal",
            ..standard("")
        },
        Case {
            merged: true,
            spannable: true,
            name: "translated-merged-over-pages",
            ..standard("")
        },
        Case {
            source: [0.0, 0.0, 160.0, 200.0],
            name: "zero-source-origin",
            ..standard("")
        },
        Case {
            writer_origin: [-150.25, -37.5],
            name: "negative-drawing-origin",
            ..standard("")
        },
        Case {
            writer_origin: [16777216.0, 16777418.0],
            name: "large-drawing-origin",
            ..standard("")
        },
        Case {
            drawn_run: [0.0, 0.0, 322.0, 402.0],
            writer_origin: [123.25, 370.375],
            name: "scaled-double",
            ..standard("")
        },
        Case {
            drawn_run: [0.0, 0.0, 80.5, 100.5],
            writer_origin: [123.25, 68.875],
            name: "scaled-half",
            ..standard("")
        },
        Case {
            drawn_run: [-10.25, 4.125, 150.75, 205.125],
            position: [5.5, -2.25],
            name: "retained-run-offsets",
            ..standard("")
        },
        Case {
            source: [1000000.0, -1000000.0, 1000160.0, -999800.0],
            name: "large-source-origin",
            ..standard("")
        },
    ];
    let results: Vec<_> = cases
        .into_iter()
        .map(|case| {
            let zero = fixture(&paths, case, 0);
            assert_eq!(
                zero,
                fixture(&paths, case, 0xa5),
                "allocation fill a5: {}",
                case.name
            );
            assert_eq!(
                zero,
                fixture(&paths, case, 0xff),
                "allocation fill ff: {}",
                case.name
            );
            zero
        })
        .collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"widget_library_sha256\":\"{WIDGET_SHA256}\",\"composer_library_sha256\":\"{COMPOSER_SHA256}\",\"bodytext_library_sha256\":\"27324ca3807f07e0c1d0647b23eb9af1296762a8c9d892ee486f37b1eb9543f0\",\"cpp_library_sha256\":\"4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4\",\"allocation_fills\":[0,165,255],\"native_addresses\":{{\"create_object\":\"0x36d6cc\",\"table_copy\":\"0x3d9af4\",\"shape_set_rect\":\"0x397708\",\"get_rect_by_drawn_rect\":\"0xe1f88\",\"composer_placement_window\":[\"0x376418\",\"0x376478\"],\"composer_span_flag_window\":[\"0x376494\",\"0x3764a8\"],\"cold_initialize\":\"0xaa6b4\",\"cold_frames\":\"0xab168\",\"draw_cell_artwork\":\"0xa748c\"}},\"capture_boundary\":\"Native Model constructor/factory/copy, Widget affine producer, Composer clone-placement instruction window, cold Drawing frames, native cell artwork; supplied retained DrawnText rectangle/position and writer origin, zeroed source/table/retained-run/layout storage, zeroed text-wrapper adapter omitting SetObject/SetTextScale, deterministic UUIDs, allocator/free/memory/mutex/C++ guard/logging services, Paint/Canvas recording interfaces, identity theme, empty display clip, canvas-local commands with native literal zero Y offset; no shaping, parent measurement, full Composer writer, Bodytext callbacks, or final PDF transport\",\"cases\":[{}]}}",
        frames::BASE_SHA256,
        results.join(",\n")
    );
}
