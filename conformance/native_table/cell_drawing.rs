use super::*;
use frames::{BASE, BASE_SHA256, LAYOUT};
use std::collections::BTreeMap;

const DRAW_CELLS: u64 = DRAWING_BASE + 0xa748c;
const DRAW_LINE: u64 = DRAWING_BASE + 0xa8008;
const HOST: u64 = 0x0300_1000;
const CONSTRUCT_PAINT: u64 = HOST;
const DESTROY_PAINT: u64 = HOST + 32;
const SET_STYLE: u64 = HOST + 64;
const SET_COLOR: u64 = HOST + 96;
const SET_WIDTH: u64 = HOST + 128;
const SET_ANTIALIAS: u64 = HOST + 160;
const IDENTITY_COLOR: u64 = HOST + 192;
const DRAW_RECT: u64 = HOST + 224;
const DRAW_ROUND_RECT: u64 = HOST + 256;
const DRAW_CANVAS_LINE: u64 = HOST + 288;
const COPY: u64 = HOST + 320;
const GET_ALPHA: u64 = HOST + 352;
const SET_ALPHA: u64 = HOST + 384;
const SAVE_CANVAS: u64 = HOST + 416;
const RESTORE_CANVAS: u64 = HOST + 448;
const TRANSLATE_CANVAS: u64 = HOST + 480;
const COMMON: u64 = MODEL + 0xa800;
const TABLE_VTABLE: u64 = MODEL + 0xb400;
const TABLE_RECT: u64 = MODEL + 0xb600;
const THEME: u64 = MODEL + 0xb700;
const THEME_VTABLE: u64 = MODEL + 0xb800;

#[derive(Default)]
struct Paint {
    color: u32,
    style: u32,
    width: f32,
}

#[derive(Default)]
struct Observation {
    paints: BTreeMap<u64, Paint>,
    commands: Vec<String>,
    selected: Vec<String>,
    position: [u32; 2],
    constructors: usize,
    destructors: usize,
    theme_colors: Vec<u32>,
    drawing: u64,
    canvas_events: Vec<String>,
    outer_border: bool,
}

fn rect(engine: Engine, address: u64) -> [f32; 4] {
    std::array::from_fn(|axis| read_float(engine, address + axis as u64 * 4))
}

fn write_rect(engine: Engine, address: u64, bounds: [f32; 4]) {
    for (axis, value) in bounds.into_iter().enumerate() {
        assert!(value.is_finite());
        write(engine, address + axis as u64 * 4, &value.to_le_bytes());
    }
}

fn scalar(engine: Engine, index: i32) -> f32 {
    let value = f32::from_bits(read_register(engine, 136 + index) as u32);
    assert!(value.is_finite());
    value
}

unsafe extern "C" fn interface(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    let object = read_register(engine, REGISTER_X0);
    let argument = read_register(engine, REGISTER_X0 + 1);
    match address {
        CONSTRUCT_PAINT => {
            assert!(
                observation
                    .paints
                    .insert(object, Paint::default())
                    .is_none()
            );
            observation.constructors += 1;
        }
        DESTROY_PAINT => {
            assert!(observation.paints.remove(&object).is_some());
            observation.destructors += 1;
        }
        SET_STYLE => observation.paints.get_mut(&object).unwrap().style = argument as u32,
        SET_COLOR => observation.paints.get_mut(&object).unwrap().color = argument as u32,
        SET_WIDTH => observation.paints.get_mut(&object).unwrap().width = scalar(engine, 0),
        SET_ANTIALIAS => assert_eq!(argument, 1),
        GET_ALPHA | SET_ALPHA => panic!("global-alpha branch is outside this capture"),
        IDENTITY_COLOR => {
            assert_eq!(object, THEME);
            assert_eq!(read_register(engine, REGISTER_X0 + 2), 3);
            observation.theme_colors.push(argument as u32);
            register(engine, REGISTER_X0, argument);
        }
        DRAW_RECT | DRAW_ROUND_RECT => {
            assert_eq!(object, CANVAS);
            let paint = observation
                .paints
                .get(&read_register(engine, REGISTER_X0 + 2))
                .unwrap();
            assert_eq!(paint.style, 0);
            let bounds = rect(engine, argument);
            let radii = if address == DRAW_ROUND_RECT {
                [scalar(engine, 0), scalar(engine, 1)]
            } else {
                [0.0, 0.0]
            };
            observation.commands.push(format!(
                "{{\"kind\":{:?},\"position\":{:?},\"bounds\":{bounds:?},\"bounds_bits\":{:?},\"radii\":{radii:?},\"color\":{}}}",
                if address == DRAW_ROUND_RECT { "round_rect" } else { "rect" },
                observation.position,
                bounds.map(f32::to_bits),
                paint.color,
            ));
        }
        DRAW_CANVAS_LINE => {
            assert_eq!(object, CANVAS);
            let paint = observation
                .paints
                .get(&read_register(engine, REGISTER_X0 + 3))
                .unwrap();
            assert_eq!(paint.style, 2);
            let second = read_register(engine, REGISTER_X0 + 2);
            let endpoints = [
                read_float(engine, argument),
                read_float(engine, argument + 4),
                read_float(engine, second),
                read_float(engine, second + 4),
            ];
            observation.commands.push(format!(
                "{{\"kind\":\"line\",\"position\":{:?},\"endpoints\":{endpoints:?},\"endpoint_bits\":{:?},\"color\":{},\"width\":{:?},\"width_bits\":{}{}}}",
                observation.position,
                endpoints.map(f32::to_bits),
                paint.color,
                paint.width,
                paint.width.to_bits(),
                if observation.drawing == 0 { String::new() } else { format!(",\"outer_border\":{}", observation.outer_border) },
            ));
        }
        SAVE_CANVAS | RESTORE_CANVAS | TRANSLATE_CANVAS => {
            assert_eq!(object, CANVAS);
            let event = if address == TRANSLATE_CANVAS {
                let offset = [scalar(engine, 0), scalar(engine, 1)];
                format!(
                    "{{\"kind\":\"translate\",\"offset\":{offset:?},\"offset_bits\":{:?}}}",
                    offset.map(f32::to_bits)
                )
            } else {
                format!(
                    "{{\"kind\":{:?},\"argument\":{}}}",
                    if address == SAVE_CANVAS {
                        "save"
                    } else {
                        "restore"
                    },
                    argument as i32
                )
            };
            observation.canvas_events.push(event);
        }
        COPY => {
            let size = usize::try_from(read_register(engine, REGISTER_X0 + 2)).unwrap();
            assert!(size <= 0x10000);
            let mut bytes = vec![0; size];
            check(unsafe { uc_mem_read(engine, argument, bytes.as_mut_ptr().cast(), size) });
            write(engine, object, &bytes);
        }
        DRAW_LINE => {
            assert_eq!(
                object,
                if observation.drawing == 0 {
                    DRAWING_OBJECT
                } else {
                    observation.drawing
                }
            );
            assert_eq!(argument, CANVAS);
            assert!(read_register(engine, REGISTER_X0 + 5) <= u64::from(observation.drawing != 0));
            observation.outer_border = read_register(engine, REGISTER_X0 + 5) != 0;
            let path = read_register(engine, REGISTER_X0 + 2);
            let endpoints = [20, 24, 28, 32].map(|offset| read_float(engine, path + offset));
            let equation = [36, 40, 44].map(|offset| read_float(engine, path + offset));
            observation.selected.push(format!(
                "{{\"position\":{:?},\"edge\":{},\"color\":{},\"width\":{:?},\"radii\":{:?},\"endpoints\":{endpoints:?},\"endpoint_bits\":{:?},\"equation\":{equation:?},\"equation_bits\":{:?}{}}}",
                observation.position,
                read_u32(engine, path + 48),
                read_u32(engine, path),
                read_float(engine, path + 4),
                [read_float(engine, path + 8), read_float(engine, path + 12)],
                endpoints.map(f32::to_bits),
                equation.map(f32::to_bits),
                if observation.drawing == 0 { String::new() } else { format!(",\"outer_border\":{}", observation.outer_border) },
            ));
        }
        address if address == DRAWING_BASE + 0xbcdb0 => {
            observation.position = [
                argument as u32,
                read_register(engine, REGISTER_X0 + 2) as u32,
            ];
        }
        _ => unreachable!("host interface {address:x}"),
    }
}

struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    observation: Box<Observation>,
}

impl Recorder {
    fn new(machine: &Machine) -> Self {
        check(unsafe { uc_mem_map(machine.engine, HOST, 0x1000, 7) });
        for (plt, target) in [
            (0xbcd70, 0x2d2208),
            (0xbcdc0, 0x3c22f0),
            (0xbcdd0, 0x3c2318),
            (0xbcdf0, 0x3c2154),
            (0xbcde0, DRAWING_BASE + 0xab4bc),
            (0xbce20, 0x3d8028),
            (0xbcce0, DRAW_LINE),
            (0xb9280, BASE + 0xb11a4),
            (0xb92a0, BASE + 0xb11bc),
            (0xbce00, BASE + 0xb127c),
            (0xbce10, GET_CELL_BACKGROUND),
            (0xbdb10, MEMSET),
            (0xbdc20, COPY),
            (0xb8ab0, CONSTRUCT_PAINT),
            (0xb8ad0, DESTROY_PAINT),
            (0xb8f40, SET_STYLE),
            (0xb8f50, SET_COLOR),
            (0xb8f80, SET_WIDTH),
            (0xb92f0, SET_ANTIALIAS),
            (0xbb950, GET_ALPHA),
            (0xb9850, SET_ALPHA),
        ] {
            bind_native(machine.engine, DRAWING_BASE + plt, target);
        }
        bind_native(machine.engine, 0x486ec0, GET_CELL_BORDER_PATH);
        bind_native(machine.engine, 0x47d920, 0x2caa60);
        let mut recorder = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            observation: Box::default(),
        };
        for address in [
            CONSTRUCT_PAINT,
            DESTROY_PAINT,
            SET_STYLE,
            SET_COLOR,
            SET_WIDTH,
            SET_ANTIALIAS,
            IDENTITY_COLOR,
            DRAW_RECT,
            DRAW_ROUND_RECT,
            DRAW_CANVAS_LINE,
            COPY,
            GET_ALPHA,
            SET_ALPHA,
            SAVE_CANVAS,
            RESTORE_CANVAS,
            TRANSLATE_CANVAS,
            DRAW_LINE,
            DRAWING_BASE + 0xbcdb0,
        ] {
            if (HOST..HOST + 0x1000).contains(&address) {
                write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
            }
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    interface as *mut c_void,
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
        for &hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, hook) });
        }
    }
}

fn configure_canvas(
    machine: &Machine,
    drawing_x: f32,
    display: [f32; 4],
    canvas_scale: f32,
    outline: [f32; 3],
    outline_color: u32,
) {
    write(machine.engine, DRAWING_OBJECT + 96, &THEME.to_le_bytes());
    write(
        machine.engine,
        DRAWING_OBJECT + 112,
        &drawing_x.to_le_bytes(),
    );
    write_rect(machine.engine, DRAWING_OBJECT + 120, display);
    write(machine.engine, THEME, &THEME_VTABLE.to_le_bytes());
    write(
        machine.engine,
        THEME_VTABLE + 80,
        &IDENTITY_COLOR.to_le_bytes(),
    );
    write(machine.engine, CANVAS, &CANVAS_VTABLE.to_le_bytes());
    for (offset, target) in [
        (232, DRAW_CANVAS_LINE),
        (272, DRAW_RECT),
        (288, DRAW_ROUND_RECT),
        (80, MATRIX_GETTER),
        (48, SAVE_CANVAS),
        (56, RESTORE_CANVAS),
        (152, TRANSLATE_CANVAS),
    ] {
        write(
            machine.engine,
            CANVAS_VTABLE + offset,
            &target.to_le_bytes(),
        );
    }
    write(machine.engine, CANVAS_MATRIX, &canvas_scale.to_le_bytes());
    write(machine.engine, MATRIX_GETTER, &0x58000040_u32.to_le_bytes());
    write(
        machine.engine,
        MATRIX_GETTER + 4,
        &0xd65f03c0_u32.to_le_bytes(),
    );
    write(
        machine.engine,
        MATRIX_GETTER + 8,
        &CANVAS_MATRIX.to_le_bytes(),
    );
    write(machine.engine, RETURN_STYLE, &outline_color.to_le_bytes());
    for (axis, value) in outline.into_iter().enumerate() {
        write(
            machine.engine,
            RETURN_STYLE + 4 + axis as u64 * 4,
            &value.to_le_bytes(),
        );
    }
}

pub(super) fn draw_prepared(machine: &Machine, table: u64, layout: u64, drawing_x: f32) -> String {
    let recorder = Recorder::new(machine);
    write(machine.engine, DRAWING_OBJECT + 104, &table.to_le_bytes());
    configure_canvas(machine, drawing_x, [0.0; 4], 1.0, [0.0; 3], 0);
    machine.call(DRAW_CELLS, &[DRAWING_OBJECT, CANVAS, layout, RETURN_STYLE]);
    let observation = &recorder.observation;
    assert_eq!(observation.constructors, 2);
    assert_eq!(observation.destructors, 2);
    assert!(observation.paints.is_empty());
    format!(
        "{{\"source_is_clone\":{},\"drawing_x\":{drawing_x:?},\"selected_paths\":[{}],\"commands\":[{}]}}",
        read_u64(machine.engine, DRAWING_OBJECT + 104) == table,
        observation.selected.join(","),
        observation.commands.join(",")
    )
}

pub(super) fn draw_background_window(machine: &Machine, composer: u64, writer: u64) -> String {
    let mut recorder = Recorder::new(machine);
    configure_canvas(machine, 0.0, [0.0; 4], 1.0, [0.0; 3], 0);
    write(machine.engine, writer, &THEME.to_le_bytes());
    recorder.observation.drawing = STACK + 112;
    write(
        machine.engine,
        DRAWING_BASE + 0xb8a30,
        &0xd65f03c0_u32.to_le_bytes(),
    );
    for (plt, target) in [
        (0x5535b0, DRAWING_BASE + 0xa5634),
        (0x5535c0, DRAWING_BASE + 0xa84ec),
        (0x5535d0, DRAWING_BASE + 0xa6738),
    ] {
        write(
            machine.engine,
            composer + plt,
            &0x58000050_u32.to_le_bytes(),
        );
        write(
            machine.engine,
            composer + plt + 4,
            &0xd61f0200_u32.to_le_bytes(),
        );
        write(machine.engine, composer + plt + 8, &target.to_le_bytes());
    }
    register(machine.engine, REGISTER_X0 + 19, writer);
    register(machine.engine, REGISTER_X0 + 23, CANVAS);
    register(machine.engine, REGISTER_SP, STACK);
    register(machine.engine, REGISTER_X30, STOP);
    let error = unsafe {
        uc_emu_start(
            machine.engine,
            composer + 0x380004,
            composer + 0x380040,
            1_000_000,
            1000,
        )
    };
    assert_eq!(
        error,
        0,
        "background window PC {:x}, LR {:x}",
        read_register(machine.engine, 260),
        read_register(machine.engine, REGISTER_X30)
    );
    assert_eq!(read_register(machine.engine, 260), composer + 0x380040);
    register(machine.engine, 144, 0);
    let error = unsafe {
        uc_emu_start(
            machine.engine,
            composer + 0x380098,
            composer + 0x3800d8,
            1_000_000,
            100000,
        )
    };
    assert_eq!(
        error,
        0,
        "background window PC {:x}, LR {:x}",
        read_register(machine.engine, 260),
        read_register(machine.engine, REGISTER_X30)
    );
    assert_eq!(read_register(machine.engine, 260), composer + 0x3800d8);
    let drawing = recorder.observation.drawing;
    let observation = &recorder.observation;
    assert_eq!(observation.constructors, observation.destructors);
    assert!(observation.paints.is_empty());
    assert_eq!(read_float(machine.engine, drawing + 112), 0.0);
    format!(
        "{{\"source_is_clone\":{},\"scroll_x\":{:?},\"text_size_delta\":{:?},\"display_rect\":{:?},\"apply_scale\":{},\"paint_count\":{},\"canvas_events\":[{}],\"selected_paths\":[{}],\"commands\":[{}]}}",
        read_u64(machine.engine, drawing + 104) == read_u64(machine.engine, writer + 40),
        read_float(machine.engine, drawing + 112),
        read_float(machine.engine, drawing + 116),
        rect(machine.engine, drawing + 120),
        read_u32(machine.engine, drawing + 156) & 255 != 0,
        observation.constructors,
        observation.canvas_events.join(","),
        observation.selected.join(","),
        observation.commands.join(",")
    )
}

struct Case {
    name: String,
    grid: BorderCase,
    constraint: u32,
    saved_offset: [f32; 2],
    drawing_x: f32,
    layout_offset: f32,
    pending_gap: f32,
    outline: [f32; 3],
    outline_color: u32,
    display: [f32; 4],
    canvas_scale: f32,
    cache_frame_override: Option<[f32; 4]>,
    saved_frame_override: Option<[f32; 4]>,
}

impl Case {
    fn new(name: &str, constraint: u32) -> Self {
        let mut grid = BorderCase::new("cell-drawing", 2, 2);
        grid.origin = [31.25, 57.5];
        grid.heights = vec![20.0, 30.0];
        grid.widths = vec![40.0, 60.0];
        Self {
            name: name.to_owned(),
            grid,
            constraint,
            saved_offset: [0.0, 0.0],
            drawing_x: 0.0,
            layout_offset: 0.0,
            pending_gap: 0.0,
            outline: [0.0; 3],
            outline_color: 0,
            display: [0.0; 4],
            canvas_scale: 1.0,
            cache_frame_override: None,
            saved_frame_override: None,
        }
    }

    fn fixture(&self, machine: &mut Machine, recorder: &mut Recorder) -> String {
        frames::initialize(machine, &self.grid);
        if self.layout_offset != 0.0 {
            register(machine.engine, 136, u64::from(self.layout_offset.to_bits()));
            machine.call(DRAWING_BASE + 0xade0c, &[LAYOUT, 0]);
        }
        let gap_pointer = read_u64(machine.engine, LAYOUT + 856);
        write(
            machine.engine,
            gap_pointer + 4,
            &self.pending_gap.to_le_bytes(),
        );
        let snapshot = frames::snapshot(machine, 2, 2);
        let mut prepared = snapshot.cached.clone();
        if let Some(bounds) = self.cache_frame_override {
            write(machine.engine, MODEL + 0xe000, &CELL_BASE.to_le_bytes());
            let node = machine.call(DRAWING_BASE + 0xb4d70, &[LAYOUT + 752, MODEL + 0xe000]);
            assert!((HEAP..machine.heap.cursor).contains(&node));
            write_rect(machine.engine, node + 24, bounds);
            prepared[0] = bounds;
        }
        let bounds = rect(machine.engine, MODEL + 16);
        let mut saved = Vec::new();
        let mut owners = Vec::new();
        for position in 0..4 {
            let row = position / 2;
            let column = position % 2;
            let cell = CELL_BASE + position as u64 * CELL_STRIDE;
            let frame = snapshot.grid[position];
            let mut source = [
                frame[0] + self.grid.origin[0] + self.saved_offset[0],
                frame[1] - self.layout_offset + self.grid.origin[1] + self.saved_offset[1],
                frame[2] + self.grid.origin[0] + self.saved_offset[0],
                frame[3] - self.layout_offset + self.grid.origin[1] + self.saved_offset[1],
            ];
            if position == 0 {
                source = self.saved_frame_override.unwrap_or(source);
            }
            write_rect(machine.engine, cell + 64, source);
            saved.push(source);
            let color = 0xff20_3040_u32 + position as u32;
            write(machine.engine, cell + 60, &color.to_le_bytes());
            write(machine.engine, cell + 128, &[1]);
            assert_eq!(
                machine.call(GET_CELL_BACKGROUND, &[cell, 1]),
                u64::from(color)
            );
            let owner = machine.call(GET_FRAME_CELL, &[MODEL, row as u64, column as u64]);
            owners.push(machine.position(owner, 4));
        }
        write(machine.engine, TABLE_OBJECT, &TABLE_VTABLE.to_le_bytes());
        write(
            machine.engine,
            TABLE_VTABLE + 160,
            &0x3d48c0_u64.to_le_bytes(),
        );
        write(machine.engine, TABLE_OBJECT + 16, &COMMON.to_le_bytes());
        write(machine.engine, COMMON + 24, &TABLE_RECT.to_le_bytes());
        write(machine.engine, COMMON + 124, &self.constraint.to_le_bytes());
        write_rect(machine.engine, TABLE_RECT + 8, bounds);
        write(
            machine.engine,
            DRAWING_OBJECT + 104,
            &TABLE_OBJECT.to_le_bytes(),
        );
        configure_canvas(
            machine,
            self.drawing_x,
            self.display,
            self.canvas_scale,
            self.outline,
            self.outline_color,
        );
        *recorder.observation = Observation::default();
        machine.call(DRAW_CELLS, &[DRAWING_OBJECT, CANVAS, LAYOUT, RETURN_STYLE]);
        let observation = &recorder.observation;
        assert_eq!(observation.constructors, 2);
        assert_eq!(observation.destructors, 2);
        assert!(observation.paints.is_empty());
        format!(
            "{{\"name\":{:?},\"constraint\":{},\"source_table_rect\":{bounds:?},\"saved_cell_rects\":{saved:?},\"drawing_x\":{:?},\"display_rect\":{:?},\"cold_frames\":{:?},\"prepared_frames\":{:?},\"pending_gaps\":{:?},\"spans\":{:?},\"frame_owners\":{owners:?},\"heights\":{:?},\"widths\":{:?},\"borders\":[{}],\"outline_color\":{},\"outline\":{:?},\"canvas_scale\":{:?},\"selected_paths\":[{}],\"commands\":[{}],\"theme_colors\":{:?}}}",
            self.name,
            self.constraint,
            self.drawing_x,
            self.display,
            snapshot.grid,
            prepared,
            snapshot.pending_gaps,
            self.grid.spans,
            self.grid.heights,
            self.grid.widths,
            self.grid
                .borders
                .iter()
                .map(|border| border_json(*border))
                .collect::<Vec<_>>()
                .join(","),
            self.outline_color,
            self.outline,
            self.canvas_scale,
            observation.selected.join(","),
            observation.commands.join(","),
            observation.theme_colors,
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base: &Path) {
    frames::load_base(machine, base);
    let mut recorder = Recorder::new(machine);
    let mut cases = Vec::new();
    for constraint in [0, 1, 2] {
        for (name, span) in [
            ("unmerged", [1, 1]),
            ("column-merged", [1, 2]),
            ("whole-grid-merged", [2, 2]),
        ] {
            let mut case = Case::new(&format!("{name}-constraint-{constraint}"), constraint);
            case.grid.spans[0] = span;
            cases.push(case);
        }
        for (name, changed_spans) in [
            ("row-merged-last-column", vec![(1, [2, 1])]),
            ("column-merged-last-row", vec![(2, [1, 2])]),
            ("retained-covered-span", vec![(0, [1, 2]), (1, [2, 1])]),
        ] {
            let mut case = Case::new(&format!("{name}-constraint-{constraint}"), constraint);
            for (position, span) in changed_spans {
                case.grid.spans[position] = span;
            }
            cases.push(case);
        }
        for (name, span, cache, saved) in [
            (
                "supplied-column-merged-frame",
                [1, 2],
                [0.0, 0.0, 100.0, 20.0],
                [31.25, 57.5, 131.25, 77.5],
            ),
            (
                "supplied-whole-grid-merged-frame",
                [2, 2],
                [0.0, 0.0, 100.0, 50.0],
                [31.25, 57.5, 131.25, 107.5],
            ),
        ] {
            let mut case = Case::new(&format!("{name}-constraint-{constraint}"), constraint);
            case.grid.spans[0] = span;
            case.cache_frame_override = Some(cache);
            case.saved_frame_override = Some(saved);
            cases.push(case);
        }
        let mut saved = Case::new(&format!("saved-offset-constraint-{constraint}"), constraint);
        saved.saved_offset = [5.25, 8.5];
        saved.drawing_x = 3.5;
        cases.push(saved);
        let mut moved = Case::new(
            &format!("prepared-offset-constraint-{constraint}"),
            constraint,
        );
        moved.layout_offset = 17.25;
        moved.drawing_x = 3.5;
        cases.push(moved);
        for gap in [0.0, 0.001, 0.00101, -0.00101, 9.0] {
            let mut case = Case::new(
                &format!("active-outer-gap-{gap}-constraint-{constraint}"),
                constraint,
            );
            case.pending_gap = gap;
            case.outline_color = 0xff10_2030;
            case.outline = [2.0, 0.0, 0.0];
            cases.push(case);
        }
        let mut rounded = Case::new(
            &format!("rounded-corners-constraint-{constraint}"),
            constraint,
        );
        rounded.outline = [2.0, 3.0, 4.0];
        rounded.outline_color = 0xff10_2030;
        cases.push(rounded);
        let mut clipped = Case::new(
            &format!("display-row-two-constraint-{constraint}"),
            constraint,
        );
        clipped.display = [0.0, 21.0, 100.0, 50.0];
        cases.push(clipped);
        for origin in [[16777216.0, -16777216.0], [0.0625, -0.125]] {
            let mut case = Case::new(
                &format!("origin-{:?}-constraint-{constraint}", origin),
                constraint,
            );
            case.grid.origin = origin;
            case.drawing_x = 3.5;
            case.grid.heights = vec![20.125, 30.25];
            case.grid.widths = vec![40.125, 60.003906];
            cases.push(case);
        }
        let mut minimum = Case::new(
            &format!("width-color-filter-constraint-{constraint}"),
            constraint,
        );
        minimum.canvas_scale = 0.5;
        minimum.grid.borders[0].as_mut().unwrap()[0].width = -3.0;
        minimum.grid.borders[0].as_mut().unwrap()[1].width = 0.0;
        minimum.grid.borders[1].as_mut().unwrap()[0].color = 0;
        minimum.grid.borders[2].as_mut().unwrap()[0].color = 0x0012_3456;
        cases.push(minimum);
    }
    let captures = cases
        .iter()
        .map(|case| {
            machine.heap.allocation_fill = 0;
            let expected = case.fixture(machine, &mut recorder);
            for fill in [0xa5, 0xff] {
                machine.heap.allocation_fill = fill;
                assert_eq!(
                    case.fixture(machine, &mut recorder),
                    expected,
                    "{} allocation fill",
                    case.name
                );
            }
            expected
        })
        .collect::<Vec<_>>();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"draw_cells_address\":\"0xa748c\",\"draw_line_address\":\"0xa8008\",\"inputs\":\"Synthetic Model grids, distinct saved cell rectangles and source table bounds. Cold cache frames and offsets are produced by native cold layout initialization and native offsetRows. Named supplied-merged-frame cases overwrite only the owner cached rectangle with the literal prepared_frame input, separately from the native cold grid and literal saved owner frame; no text measurement or pagination feedback is executed. Complete drawTableCellWithoutText and drawLinePath execute unchanged. Canvas line, rectangle and rounded-rectangle interface commands are recorded. Allocation, deletion, memset and memmove are host supplied. Paint construction/setters use host state, global alpha is disabled, and the supplied theme converter is identity. The outline color/width/radii argument is supplied independently; upstream getTableBorderStyle aggregation is not executed. Recorded command positions come from the raw Drawing GetCell call and can differ from Model frame_owners. There is no device paint, rasterization, canvas clipping or color-theme parity claim.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n"),
    );
}
