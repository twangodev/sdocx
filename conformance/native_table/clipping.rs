use super::*;
use frames::{BASE, BASE_SHA256, LAYOUT};

const CLIP: u64 = DRAWING_BASE + 0xa72d4;
const VISIBLE: u64 = DRAWING_BASE + 0xaba50;
const LAST_ROW: u64 = DRAWING_BASE + 0xabba4;
const EXACT_VISIBLE: u64 = DRAWING_BASE + 0xab8e8;
const CLIP_RECORDER: u64 = 0x0300_0280;
const MEMMOVE: u64 = 0x0300_02a0;
const FOCUS: u64 = MODEL + 0xe300;
const COMMON_IMPL: u64 = MODEL + 0xa800;
const LAYOUT_VTABLE: u64 = MODEL + 0xb000;

unsafe extern "C" fn record_clip(engine: Engine, _: u64, _: u32, data: *mut c_void) {
    assert_eq!(read_register(engine, REGISTER_X0), CANVAS);
    let calls = unsafe { &mut *data.cast::<Vec<[i32; 5]>>() };
    calls.push(std::array::from_fn(|i| {
        read_register(engine, REGISTER_X0 + i as i32 + 1) as u32 as i32
    }));
}

unsafe extern "C" fn copy_memory(engine: Engine, _: u64, _: u32, _: *mut c_void) {
    let destination = read_register(engine, REGISTER_X0);
    let source = read_register(engine, REGISTER_X0 + 1);
    let count = read_register(engine, REGISTER_X0 + 2) as usize;
    assert!(count <= 128);
    let mut bytes = vec![0; count];
    check(unsafe { uc_mem_read(engine, source, bytes.as_mut_ptr().cast(), count) });
    write(engine, destination, &bytes);
}

struct Recorder {
    engine: Engine,
    hook: usize,
    copy_hook: usize,
    calls: Box<Vec<[i32; 5]>>,
}

impl Recorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hook: 0,
            copy_hook: 0,
            calls: Box::default(),
        };
        write(machine.engine, CLIP_RECORDER, &0xd65f03c0_u32.to_le_bytes());
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut recorder.hook,
                4,
                record_clip as *mut c_void,
                ptr::from_mut(recorder.calls.as_mut()).cast(),
                CLIP_RECORDER,
                CLIP_RECORDER,
            )
        });
        write(machine.engine, MEMMOVE, &0xd65f03c0_u32.to_le_bytes());
        bind_native(machine.engine, DRAWING_BASE + 0xbdc20, MEMMOVE);
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut recorder.copy_hook,
                4,
                copy_memory as *mut c_void,
                ptr::null_mut(),
                MEMMOVE,
                MEMMOVE,
            )
        });
        recorder
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        check(unsafe { uc_hook_del(self.engine, self.hook) });
        check(unsafe { uc_hook_del(self.engine, self.copy_hook) });
    }
}

fn write_rect(machine: &Machine, address: u64, rect: [f32; 4]) {
    for (axis, value) in rect.into_iter().enumerate() {
        assert!(value.is_finite());
        write(
            machine.engine,
            address + axis as u64 * 4,
            &value.to_le_bytes(),
        );
    }
}

fn read_rect(machine: &Machine, address: u64) -> [f32; 4] {
    std::array::from_fn(|axis| read_float(machine.engine, address + axis as u64 * 4))
}

fn rectangle_result(machine: &Machine) -> [f32; 4] {
    std::array::from_fn(|i| {
        let value = f32::from_bits(read_register(machine.engine, 136 + i as i32) as u32);
        assert!(value.is_finite());
        value
    })
}

fn query_rect(machine: &Machine, function: u64, focus: [f32; 4]) -> [f32; 4] {
    for (axis, value) in focus.into_iter().enumerate() {
        register(
            machine.engine,
            136 + axis as i32,
            u64::from(value.to_bits()),
        );
    }
    machine.call(function, &[LAYOUT]);
    rectangle_result(machine)
}

struct Case {
    name: String,
    grid: BorderCase,
    constraint: u32,
    row_offset: f32,
    bands: Vec<[f32; 4]>,
    outline_width: f32,
}

impl Case {
    fn new(name: &str, spans: &[(usize, [u32; 2])]) -> Self {
        let mut grid = BorderCase::new("clipping-grid", 3, 2);
        grid.heights = vec![20.0, 30.0, 40.0];
        grid.widths = vec![40.0, 60.0];
        grid.borders.fill(None);
        grid.native_defaults = true;
        grid.outer_border = Some(
            [BorderStyle {
                color: 0xff000000,
                width: 1.0,
                start_radius: 0.0,
                end_radius: 0.0,
            }; 4],
        );
        for &(slot, span) in spans {
            grid.spans[slot] = span;
        }
        Self {
            name: name.to_owned(),
            grid,
            constraint: 2,
            row_offset: 0.0,
            bands: vec![[0.0, 20.0, 1080.0, 30.0]],
            outline_width: 1.0,
        }
    }

    fn fixture(&self, machine: &mut Machine, recorder: &mut Recorder) -> String {
        frames::initialize(machine, &self.grid);
        write(machine.engine, LAYOUT, &LAYOUT_VTABLE.to_le_bytes());
        write(
            machine.engine,
            LAYOUT_VTABLE + 88,
            &(DRAWING_BASE + 0xab494).to_le_bytes(),
        );
        write(
            machine.engine,
            TABLE_OBJECT + 16,
            &COMMON_IMPL.to_le_bytes(),
        );
        write(
            machine.engine,
            COMMON_IMPL + 124,
            &self.constraint.to_le_bytes(),
        );
        write(
            machine.engine,
            DRAWING_OBJECT + 104,
            &TABLE_OBJECT.to_le_bytes(),
        );
        write(machine.engine, CANVAS, &CANVAS_VTABLE.to_le_bytes());
        write(
            machine.engine,
            CANVAS_VTABLE + 64,
            &CLIP_RECORDER.to_le_bytes(),
        );
        write(
            machine.engine,
            RETURN_STYLE + 4,
            &self.outline_width.to_le_bytes(),
        );
        register(machine.engine, 136, u64::from(self.row_offset.to_bits()));
        machine.call(DRAWING_BASE + 0xade0c, &[LAYOUT, 0]);
        for slot in 0..self.grid.spans.len() {
            geometry::TextMetrics {
                has_text_layout: false,
                has_text: false,
                first_line_height: 0.0,
                top_margin: 0.0,
                measured_height: 0.0,
            }
            .supply(machine, slot);
        }
        machine.call(DRAWING_BASE + 0xab168, &[LAYOUT]);
        splits::supply_bands(machine, &self.bands);
        let frames = frames::snapshot(machine, 3, 2).grid;
        let measured = read_rect(machine, LAYOUT + 668);
        let mut queries = Vec::new();
        for focus in focus_rectangles(&self.bands) {
            let visible = query_rect(machine, VISIBLE, focus);
            for (axis, value) in focus.into_iter().enumerate() {
                register(
                    machine.engine,
                    136 + axis as i32,
                    u64::from(value.to_bits()),
                );
            }
            let last_row = machine.call(LAST_ROW, &[LAYOUT]) as u32 as i32;
            write_rect(machine, FOCUS, focus);
            machine.call(EXACT_VISIBLE, &[LAYOUT, FOCUS, 0]);
            assert_eq!(
                rectangle_result(machine).map(f32::to_bits),
                visible.map(f32::to_bits),
                "public and vertical-only visibility"
            );
            machine.call(EXACT_VISIBLE, &[LAYOUT, FOCUS, 1]);
            let intersected = rectangle_result(machine);
            recorder.calls.clear();
            for (axis, value) in focus.into_iter().enumerate() {
                register(
                    machine.engine,
                    136 + axis as i32,
                    u64::from(value.to_bits()),
                );
            }
            machine.call(DRAWING_BASE + 0xa84f4, &[DRAWING_OBJECT]);
            assert_eq!(read_rect(machine, DRAWING_OBJECT + 120), focus);
            machine.call(CLIP, &[DRAWING_OBJECT, CANVAS, LAYOUT, RETURN_STYLE]);
            assert!(recorder.calls.len() <= 1);
            let adjusted_focus = read_rect(machine, DRAWING_OBJECT + 120);
            queries.push(format!(
                "{{\"focus\":{focus:?},\"last_row\":{last_row},\"visible\":{visible:?},\"intersected\":{intersected:?},\"adjusted_focus\":{adjusted_focus:?},\"canvas_clips\":{:?}}}",
                recorder.calls,
            ));
        }
        format!(
            "{{\"name\":{:?},\"constraint\":{},\"heights\":{:?},\"widths\":{:?},\"spans\":{:?},\"row_offset\":{:?},\"bands\":{:?},\"outline_width\":{:?},\"frames\":{frames:?},\"measured_bbox\":{measured:?},\"queries\":[{}]}}",
            self.name,
            self.constraint,
            self.grid.heights,
            self.grid.widths,
            self.grid.spans,
            self.row_offset,
            self.bands,
            self.outline_width,
            queries.join(","),
        )
    }
}

fn focus_rectangles(bands: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let mut focuses = vec![
        [0.0; 4],
        [5.0, 10.0, 5.0, 40.0],
        [5.0, 10.0, 20.0, 10.0],
        [20.0, 10.0, 5.0, 40.0],
        [5.0, 40.0, 20.0, 10.0],
        [-100.0, -100.0, 200.0, 200.0],
        [5.0, 10.0, 80.0, 40.0],
        [200.0, 10.0, 300.0, 40.0],
        [5.0, -100.0, 80.0, -10.0],
        [5.0, 100.0, 80.0, 200.0],
    ];
    for y in [0.5_f32, 20.5, 50.5, 90.5] {
        for edge in [
            f32::from_bits(y.to_bits() - 1),
            y,
            f32::from_bits(y.to_bits() + 1),
        ] {
            focuses.push([-100.0, -100.0, 200.0, edge]);
        }
    }
    for &band in bands {
        for y in [band[1], band[1] + 0.25, band[3] - 0.25, band[3]] {
            focuses.push([-100.0, y, 200.0, 200.0]);
            focuses.push([-100.0, -100.0, 200.0, y]);
        }
    }
    focuses
}

pub(super) fn capture(machine: &mut Machine, base: &Path, widget: &Path, text: &Path) {
    frames::load_base(machine, base);
    geometry::load_measurements(machine, widget, text);
    for (plt, target) in [
        (0xbcd70, 0x2d2208),
        (0xbcd80, VISIBLE),
        (0xbd020, LAST_ROW),
        (0xb92a0, BASE + 0xb11bc),
        (0xb9290, BASE + 0xb1350),
        (0xbd350, BASE + 0xb10ec),
        (0xbbe30, BASE + 0xb16e0),
        (0xb8b10, BASE + 0xb108c),
    ] {
        bind_native(machine.engine, DRAWING_BASE + plt, target);
    }
    let mut recorder = Recorder::new(machine);
    let mut cases = Vec::new();
    for (name, spans) in [
        ("unmerged", vec![]),
        ("last-column-owner", vec![(4, [1, 2])]),
        ("last-row-owner", vec![(1, [3, 1])]),
        ("whole-grid-owner", vec![(0, [3, 2])]),
        ("covered-owner-chain", vec![(0, [2, 1]), (2, [2, 1])]),
    ] {
        for (kind, bands) in [
            ("no-bands", vec![]),
            ("ordinary-band", vec![[0.0, 20.0, 1080.0, 30.0]]),
            ("x-disjoint-band", vec![[2000.0, 20.0, 3000.0, 30.0]]),
            ("empty-band", vec![[0.0, 20.0, 1080.0, 20.0]]),
            (
                "unsorted-bands",
                vec![[0.0, 60.0, 1080.0, 80.0], [0.0, 10.0, 1080.0, 40.0]],
            ),
            (
                "overlapping-bands",
                vec![[0.0, 10.0, 1080.0, 40.0], [0.0, 30.0, 1080.0, 60.0]],
            ),
        ] {
            let mut case = Case::new(&format!("{name}-{kind}"), &spans);
            case.bands = bands;
            cases.push(case);
        }
    }
    for constraint in [0, 1, 2] {
        for width in [
            0.0,
            0.25,
            1.0,
            2.5,
            5.0,
            f32::from_bits(1_f32.to_bits() - 2),
            f32::from_bits(1_f32.to_bits() + 2),
        ] {
            let mut case = Case::new(&format!("constraint-{constraint}-outline-{width}"), &[]);
            case.constraint = constraint;
            case.outline_width = width;
            cases.push(case);
        }
    }
    for offset in [-90.25, -10.25, 0.25, 20.25] {
        let mut case = Case::new(&format!("offset-{offset}"), &[(0, [3, 2])]);
        case.row_offset = offset;
        cases.push(case);
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
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"widget_library_sha256\":\"{}\",\"text_library_sha256\":\"{}\",\"clip_address\":\"0xa72d4\",\"display_rect_address\":\"0xa84f4\",\"visible_address\":\"0xaba50\",\"exact_visible_address\":\"0xab8e8\",\"last_row_address\":\"0xabba4\",\"measurement_inputs\":\"Native cold initialization and measured-bounds update with empty text caches; no cold/warm text sizing. The actual display-rectangle setter and clip routine execute. Canvas clip arguments are recorded; allocation, deletion, fill and memmove are host supplied.\",\"cases\":[\n{}\n]}}",
        geometry::WIDGET_SHA256,
        geometry::TEXT_SHA256,
        captures.join(",\n"),
    );
}
