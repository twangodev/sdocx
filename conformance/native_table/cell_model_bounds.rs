use super::*;

const SOURCE: u64 = MODEL + 0x10000;
const BASE_IMPL: u64 = MODEL + 0x11000;
const BASE_DATA: u64 = MODEL + 0x12000;
const SHAPE_BASE_IMPL: u64 = MODEL + 0x13000;
const SHAPE_IMPL: u64 = MODEL + 0x14000;
const CELL: u64 = MODEL + 0x15000;
const CONTENT_VTABLE: u64 = 0x494bf0;
const TEMPLATE_FACTORY: u64 = 0x213748;
const TEMPLATE_SET_RECT: u64 = 0x21236c;

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    cell: [f32; 4],
    content: [f32; 4],
    next: [f32; 4],
}

fn rectangle_arguments(engine: Engine, rectangle: [f32; 4]) {
    for (index, value) in rectangle.into_iter().enumerate() {
        register(engine, 136 + index as i32, u64::from(value.to_bits()));
    }
}

fn write_rectangle(engine: Engine, address: u64, rectangle: [f32; 4]) {
    for (index, value) in rectangle.into_iter().enumerate() {
        write(engine, address + index as u64 * 4, &value.to_le_bytes());
    }
}

fn rectangle(engine: Engine, address: u64) -> [f32; 4] {
    std::array::from_fn(|index| read_float(engine, address + index as u64 * 4))
}

fn initialize(machine: &mut Machine, case: Case, fill: u8) {
    machine.heap.cursor = HEAP;
    machine.heap.allocation_fill = fill;
    write(machine.engine, MODEL, &vec![0; 0x100000]);
    let engine = machine.engine;
    write(engine, SOURCE, &CONTENT_VTABLE.to_le_bytes());
    write(engine, SOURCE + 16, &BASE_IMPL.to_le_bytes());
    write(engine, BASE_IMPL + 8, &SOURCE.to_le_bytes());
    write(engine, BASE_IMPL + 24, &BASE_DATA.to_le_bytes());
    write(engine, BASE_DATA, &2_u32.to_le_bytes());
    write(engine, SOURCE + 32, &SHAPE_BASE_IMPL.to_le_bytes());
    write(engine, SHAPE_BASE_IMPL, &SOURCE.to_le_bytes());
    write(engine, SOURCE + 96, &SHAPE_IMPL.to_le_bytes());
    write(engine, SHAPE_IMPL, &SOURCE.to_le_bytes());
    write(engine, SHAPE_IMPL + 16, &SOURCE.to_le_bytes());
    for offset in [8, 24] {
        machine.call(0x0209c868, &[SHAPE_BASE_IMPL + offset]);
        assert_eq!(machine.call(0x0209c928, &[SHAPE_BASE_IMPL + offset]), 1);
    }
    let template = machine.call(TEMPLATE_FACTORY, &[0]);
    assert!(template >= HEAP && template < TLS);
    assert_eq!(read_u64(engine, template), 0x48f5d8);
    rectangle_arguments(engine, case.content);
    assert_eq!(machine.call(TEMPLATE_SET_RECT, &[template]), 1);
    write(engine, SHAPE_IMPL + 32, &template.to_le_bytes());
    machine.call(0x3b8014, &[SHAPE_IMPL + 128]);
    assert_eq!(machine.call(0x3b8030, &[SHAPE_IMPL + 128]), 1);
    write(engine, SHAPE_IMPL + 364, &u32::MAX.to_le_bytes());
    write(engine, CELL + 88, &SOURCE.to_le_bytes());
    write_rectangle(engine, CELL + 64, case.cell);
    write_rectangle(engine, BASE_DATA + 8, case.content);
    write_rectangle(engine, BASE_DATA + 24, case.content);
}

fn sample(machine: &mut Machine, case: Case, method: &str, fill: u8) -> String {
    initialize(machine, case, fill);
    let (function, arguments): (u64, &[u64]) = match method {
        "data-only" => (0x3c20cc, &[CELL]),
        "set-rect" => (0x3c2008, &[CELL]),
        "set-rect-false" => (0x3c2024, &[CELL, 0]),
        "set-rect-true" => (0x3c2024, &[CELL, 1]),
        _ => unreachable!(),
    };
    rectangle_arguments(machine.engine, case.next);
    let result = machine.call(function, arguments);
    let cell = rectangle(machine.engine, CELL + 64);
    let content = rectangle(machine.engine, BASE_DATA + 8);
    let drawn_content = rectangle(machine.engine, BASE_DATA + 24);
    let cell_changed = read_u32(machine.engine, CELL + 80) >> 8 & 0xffff;
    format!(
        "  {{\"name\":\"{}-{method}\",\"method\":\"{method}\",\"cell_bbox\":{:?},\"content_bbox\":{:?},\"requested_bbox\":{:?},\"result\":{result},\"cell_bbox_after\":{cell:?},\"content_bbox_after\":{content:?},\"drawn_content_bbox_after\":{drawn_content:?},\"cell_changed_flags\":{cell_changed}}}",
        case.name, case.cell, case.content, case.next,
    )
}

fn load(machine: &Machine, base: &Path) {
    frames::load_base(machine, base);
    for (plt, target) in [
        (0x47afa0, 0x020c2a18),
        (0x47b0e0, 0x020c3298),
        (0x47b420, 0x0209d120),
        (0x47e370, 0x020b1a0c),
        (0x47e7c0, 0x020b108c),
        (0x47e7d0, 0x020b109c),
        (0x47e840, 0x020b11a4),
        (0x481850, 0x020b1460),
        (0x485220, 0x020b1438),
        (0x485230, 0x020b144c),
        (0x47ac30, NEW),
        (0x47ac40, NEW),
        (0x47ac50, DELETE),
        (0x020e55f0, NEW),
        (0x020e56f0, NEW),
        (0x020e5780, NEW),
        (0x020e55d0, DELETE),
        (0x020e5650, DELETE),
    ] {
        bind_native(machine.engine, plt, target);
    }
    for plt in [0x47ac10, 0x47ac20, 0x47b060, 0x47b6c0, 0x020e5f30] {
        write(machine.engine, plt, &0xd65f03c0_u32.to_le_bytes());
    }
}

fn cases() -> Vec<Case> {
    let original: [f32; 4] = [2.25, 3.5, 82.25, 43.5];
    let moved = [13.25, -19.5, 93.25, 20.5];
    let different = [-10.0, -20.0, 10.0, 5.0];
    let large = [16_777_216.0, 16_777_216.0, 16_777_296.0, 16_777_256.0];
    let ulp = f32::from_bits(original[2].to_bits() + 1);
    vec![
        Case {
            name: "same",
            cell: original,
            content: original,
            next: original,
        },
        Case {
            name: "same-cell-divergent-content",
            cell: original,
            content: different,
            next: original,
        },
        Case {
            name: "different-cell-current-content",
            cell: different,
            content: original,
            next: original,
        },
        Case {
            name: "translated",
            cell: original,
            content: original,
            next: moved,
        },
        Case {
            name: "translated-divergent-content",
            cell: original,
            content: different,
            next: moved,
        },
        Case {
            name: "width-increased",
            cell: original,
            content: original,
            next: [2.25, 3.5, 122.25, 43.5],
        },
        Case {
            name: "height-increased",
            cell: original,
            content: original,
            next: [2.25, 3.5, 82.25, 83.5],
        },
        Case {
            name: "width-one-ulp",
            cell: original,
            content: original,
            next: [2.25, 3.5, ulp, 43.5],
        },
        Case {
            name: "zero-width",
            cell: original,
            content: original,
            next: [2.25, 3.5, 2.25, 43.5],
        },
        Case {
            name: "zero-height",
            cell: original,
            content: original,
            next: [2.25, 3.5, 82.25, 3.5],
        },
        Case {
            name: "inverted-x",
            cell: original,
            content: original,
            next: [82.25, 3.5, 2.25, 43.5],
        },
        Case {
            name: "inverted-y",
            cell: original,
            content: original,
            next: [2.25, 43.5, 82.25, 3.5],
        },
        Case {
            name: "inverted-both",
            cell: original,
            content: original,
            next: [82.25, 43.5, 2.25, 3.5],
        },
        Case {
            name: "large-translated",
            cell: original,
            content: original,
            next: large,
        },
        Case {
            name: "signed-zero",
            cell: [0.0, 0.0, 80.0, 40.0],
            content: [0.0, 0.0, 80.0, 40.0],
            next: [-0.0, -0.0, 80.0, 40.0],
        },
    ]
}

pub(super) fn capture(machine: &mut Machine, base: &Path) {
    load(machine, base);
    let mut output = Vec::new();
    for case in cases() {
        for method in ["data-only", "set-rect", "set-rect-false", "set-rect-true"] {
            let expected = sample(machine, case, method, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(sample(machine, case, method, fill), expected);
            }
            output.push(expected);
        }
    }
    println!(
        concat!(
            "{{\n\"mode\":\"native-cell-model-bounds\",\n",
            "\"apk_version\":\"4.4.45.37\",\n",
            "\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\n",
            "\"model_sha256\":\"{}\",\n\"base_sha256\":\"{}\",\n",
            "\"allocation_fills\":[0,165,255],\n",
            "\"allocation_fill_scope\":\"native template, fill and list heap allocations; supplied fixture object storage is zero-initialized for every sample\",\n",
            "\"supplied_object_state\":\"manually seeded TableCell, TableCellContentObject, ObjectBaseImpl, ObjectBaseData, ObjectShapeBaseImpl and ObjectShapeImpl; actual native content vtable; cell rectangle seeded separately from identical initial Model and drawn rectangles\",\n",
            "\"native_setter_addresses\":{{\"cell_set_rect\":\"0x3c2008\",\"cell_set_rect_bool\":\"0x3c2024\",\"cell_set_rect_data_only\":\"0x3c20cc\",\"shape_set_rect\":\"0x397708\",\"shape_set_rect_data_only\":\"0x399954\",\"shape_impl_set_rect\":\"0x3a6a60\",\"base_set_rect_only_data\":\"0x2d2b18\",\"base_impl_set_rect\":\"0x2d7324\",\"base_impl_set_drawn_rect\":\"0x2d740c\"}},\n",
            "\"native_constructor_addresses\":{{\"template_factory\":\"0x213748\",\"template_initial_rect\":\"0x21236c\",\"list_constructor_base\":\"0x9c868\",\"list_construct_base\":\"0x9c928\",\"fill_image_effect_constructor\":\"0x3b8014\",\"fill_image_effect_construct\":\"0x3b8030\"}},\n",
            "\"host_boundaries\":[\"bounded allocation and free\",\"memory operations\",\"Android logging\",\"Error::SetError\",\"recursive mutex construction\"],\n",
            "\"execution_conditions\":[\"nil model context\",\"nil observer\",\"empty native magnetic and connected-object lists\",\"disabled follower offset and resize flags\",\"absent image IDs\",\"native type-0 template\",\"native empty FillImageEffect\"],\n",
            "\"excluded_lifecycle\":[\"document history and coedit context\",\"observer callbacks\",\"image transforms\",\"follower movement\",\"Drawing text shaping and layout\"],\n",
            "\"cases\":[\n{}\n]\n}}",
        ),
        LIBRARY_SHA256,
        frames::BASE_SHA256,
        output.join(",\n"),
    );
}
