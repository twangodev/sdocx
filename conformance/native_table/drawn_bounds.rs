use super::*;
use cell_model_lifecycle::Paths;

const INPUT: u64 = MODEL + 0x18000;
const STYLE: u64 = MODEL + 0x18100;

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    bounds: [f32; 4],
    outer: Option<[f32; 4]>,
    default: Option<[f32; 4]>,
    own: [Option<[f32; 4]>; 9],
    color: u32,
    merge: bool,
    null_outer: bool,
    null_default: bool,
}

fn rectangle(engine: Engine, address: u64) -> [f32; 4] {
    std::array::from_fn(|index| read_float(engine, address + index as u64 * 4))
}

fn returned_rect(machine: &Machine, function: u64, object: u64) -> [f32; 4] {
    machine.call(function, &[object]);
    std::array::from_fn(|index| {
        f32::from_bits(read_register(machine.engine, 136 + index as i32) as u32)
    })
}

fn set_border(machine: &Machine, object: u64, cell: bool, widths: [f32; 4], color: u32) {
    for (edge, width) in widths.into_iter().enumerate() {
        machine.call(0x3dc280, &[STYLE]);
        write(machine.engine, STYLE, &color.to_le_bytes());
        write(machine.engine, STYLE + 4, &width.to_le_bytes());
        machine.call(
            if cell { 0x3c24cc } else { 0x3dc770 },
            &[object, STYLE, 1 << edge],
        );
    }
}

fn border_json(machine: &Machine, pointer: u64) -> String {
    if pointer == 0 {
        return "null".to_owned();
    }
    let edges = (0..4)
        .map(|edge| {
            format!(
                "{{\"color\":{},\"width\":{:?}}}",
                read_u32(machine.engine, pointer + edge * 20),
                read_float(machine.engine, pointer + edge * 20 + 4)
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", edges.join(","))
}

impl Case {
    fn sample(self, paths: &Paths, fill: u8) -> String {
        let machine = cell_model_lifecycle::initialize(paths, fill);
        for (index, value) in self.bounds.into_iter().enumerate() {
            write(
                machine.engine,
                INPUT + index as u64 * 4,
                &value.to_le_bytes(),
            );
        }
        machine.call(0x3d2690, &[TABLE_OBJECT]);
        assert_eq!(machine.call(0x3d27d8, &[TABLE_OBJECT, INPUT, 3, 3]), 1);
        let implementation = read_u64(machine.engine, TABLE_OBJECT + 104);
        let outer = read_u64(machine.engine, implementation + 152);
        let default = read_u64(machine.engine, implementation + 160);
        assert_ne!(outer, 0);
        assert_ne!(default, 0);
        let constructor_outer = border_json(&machine, outer);
        let constructor_default = border_json(&machine, default);
        if let Some(widths) = self.outer {
            set_border(&machine, outer, false, widths, self.color);
        }
        if let Some(widths) = self.default {
            set_border(&machine, default, false, widths, self.color);
        }
        for (slot, widths) in self.own.into_iter().enumerate() {
            if let Some(widths) = widths {
                let cell =
                    machine.call(0x3d2be0, &[TABLE_OBJECT, slot as u64 / 3, slot as u64 % 3]);
                set_border(&machine, cell, true, widths, self.color);
            }
        }
        if self.merge {
            assert_eq!(machine.call(0x3d6038, &[TABLE_OBJECT, 0, 0, 2, 2]), 1);
        }
        if self.null_outer {
            write(machine.engine, implementation + 152, &0_u64.to_le_bytes());
        }
        if self.null_default {
            write(machine.engine, implementation + 160, &0_u64.to_le_bytes());
        }
        let physical_cells=(0..9).map(|slot| {
            let cell=machine.call(0x3d2be0,&[TABLE_OBJECT,slot/3,slot%3]);
            let pointer=read_u64(machine.engine,cell+96);
            let selected=machine.call(0x3c7510,&[implementation,slot/3,slot%3]);
            let owner=machine.call(GET_FRAME_CELL,&[implementation,slot/3,slot%3]);
            let owner_slot=(0..9).position(|index|machine.call(0x3d2be0,&[TABLE_OBJECT,index/3,index%3])==owner).unwrap();
            format!("{{\"slot\":{slot},\"owner_slot\":{owner_slot},\"own_border\":{},\"selected_border\":{}}}",border_json(&machine,pointer),border_json(&machine,selected))
        }).collect::<Vec<_>>();
        let widths = if self.null_outer {
            "null".to_owned()
        } else {
            let values: [f32; 4] = std::array::from_fn(|index| {
                machine.call(
                    [0x3dbba4, 0x3dbc30, 0x3dbcbc, 0x3dbd48][index],
                    &[TABLE_OBJECT],
                );
                f32::from_bits(read_register(machine.engine, 136) as u32)
            });
            format!("{values:?}")
        };
        let raw = returned_rect(&machine, 0x2caa60, TABLE_OBJECT);
        let drawn = returned_rect(&machine, 0x3d48c4, TABLE_OBJECT);
        let stored_rect = rectangle(
            machine.engine,
            read_u64(
                machine.engine,
                read_u64(machine.engine, TABLE_OBJECT + 16) + 24,
            ) + 8,
        );
        assert_eq!(raw, stored_rect);
        format!(
            "{{\"name\":{:?},\"bounds\":{:?},\"color_input\":{},\"merged\":{},\"supplied_null_outer_pointer\":{},\"supplied_null_default_pointer\":{},\"constructor_outer_border\":{},\"constructor_default_border\":{},\"outer_border\":{},\"default_border\":{},\"physical_cells\":[{}],\"native_drawn_widths\":{},\"native_raw_rect\":{:?},\"native_drawn_rect\":{:?}}}",
            self.name,
            self.bounds,
            self.color,
            self.merge,
            self.null_outer,
            self.null_default,
            constructor_outer,
            constructor_default,
            border_json(&machine, read_u64(machine.engine, implementation + 152)),
            border_json(&machine, read_u64(machine.engine, implementation + 160)),
            physical_cells.join(","),
            widths,
            raw,
            drawn
        )
    }
}

pub(super) fn capture(paths: Paths) {
    let base = Case {
        name: "native-constructor-defaults",
        bounds: [13.25, -19.5, 173.25, 180.5],
        outer: None,
        default: None,
        own: [None; 9],
        color: 0xff000000,
        merge: false,
        null_outer: false,
        null_default: false,
    };
    let boundary = [
        Some([2., 0., 0., 0.]),
        Some([0., 4., 0., 0.]),
        Some([0., 0., 6., 0.]),
        None,
        Some([1000.; 4]),
        None,
        None,
        Some([0., 0., 0., 8.]),
        None,
    ];
    let mut merged_border = [None; 9];
    merged_border[8] = Some([10., 12., 14., 16.]);
    let mut interior_border = [None; 9];
    interior_border[4] = Some([1000.; 4]);
    let cases = [
        base,
        Case {
            name: "asymmetric-opaque",
            outer: Some([2., 4., 6., 8.]),
            ..base
        },
        Case {
            name: "asymmetric-transparent",
            outer: Some([2., 4., 6., 8.]),
            color: 0,
            ..base
        },
        Case {
            name: "asymmetric-partial-alpha",
            outer: Some([2., 4., 6., 8.]),
            color: 0x01000000,
            ..base
        },
        Case {
            name: "default-cell-edges",
            outer: Some([0.; 4]),
            default: Some([8., 6., 4., 2.]),
            ..base
        },
        Case {
            name: "physical-boundary-own-edges",
            outer: Some([0.; 4]),
            default: Some([0.; 4]),
            own: boundary,
            color: 0,
            ..base
        },
        Case {
            name: "own-zero-overrides-default",
            outer: Some([0.; 4]),
            default: Some([10.; 4]),
            own: [Some([0.; 4]); 9],
            ..base
        },
        Case {
            name: "merged-physical-last-slot",
            outer: Some([0.; 4]),
            default: Some([0.; 4]),
            own: merged_border,
            merge: true,
            ..base
        },
        Case {
            name: "merged-interior-slot-ignored",
            outer: Some([0.; 4]),
            default: Some([0.; 4]),
            own: interior_border,
            merge: true,
            ..base
        },
        Case {
            name: "supplied-null-outer-with-wide-cells",
            own: [Some([100.; 4]); 9],
            null_outer: true,
            ..base
        },
        Case {
            name: "supplied-null-default",
            outer: Some([0.; 4]),
            null_default: true,
            ..base
        },
        Case {
            name: "fractional-f32-arithmetic",
            outer: Some([1.1, 2.3, 3.7, 4.9]),
            ..base
        },
        Case {
            name: "large-f32-origin",
            bounds: [16777216., 16777216., 16777536., 16777516.],
            outer: Some([1., 3., 5., 7.]),
            ..base
        },
    ];
    let samples = cases
        .into_iter()
        .map(|case| {
            let expected = case.sample(&paths, 0);
            for fill in [85, 165, 255, 0] {
                assert_eq!(
                    case.sample(&paths, fill),
                    expected,
                    "{} allocation fill {fill}",
                    case.name
                );
            }
            expected
        })
        .collect::<Vec<_>>();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,85,165,255],\"repeat_fill\":0,\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"bodytext_library_sha256\":\"27324ca3807f07e0c1d0647b23eb9af1296762a8c9d892ee486f37b1eb9543f0\",\"cpp_library_sha256\":\"4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4\",\"native_entrypoints\":{{\"table_constructor\":\"0x3d2690\",\"table_construct\":\"0x3d27d8\",\"border_style_constructor\":\"0x3dc280\",\"border_style_setter\":\"0x3dc770\",\"cell_border_setter\":\"0x3c24cc\",\"merge_cells\":\"0x3d6038\",\"drawn_rect\":\"0x3d48c4\",\"drawn_rect_implementation\":\"0x3c6cac\",\"drawn_widths\":[\"0x3dbba4\",\"0x3dbc30\",\"0x3dbcbc\",\"0x3dbd48\"],\"physical_cell_border_getter\":\"0x3c7510\"}},\"capture_boundary\":\"complete native Model table/row/cell/content and default border constructors, native BorderStyle constructor and setters, native merge, complete GetDrawnRect and width getters; supplied input rectangle and BorderStyle color/width fields; explicitly named nil outer/default cases overwrite pointers after successful construction and do not represent missing saved style fields; direct width getters are excluded for nil outer because native GetDrawnRect skips them\",\"host_boundaries\":[\"bounded allocation and free\",\"memory and byte string operations\",\"deterministic UUID service\",\"single-thread mutex and C++ guard services\",\"destructor registration\",\"Android logging and Error::SetError\"],\"allocation_boundary\":\"bounded table and input storage zero-initialized; allocation fills vary native owned allocations\",\"excluded_producers\":\"saved document parser, allocation failure behavior, Drawing and text layout, Composer clone and final export\",\"cases\":[\n{}\n]}}",
        frames::BASE_SHA256,
        samples.join(",\n")
    );
}
