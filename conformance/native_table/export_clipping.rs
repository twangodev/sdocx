use super::*;
use frames::{BASE, BASE_SHA256, LAYOUT};

const COMPOSER: u64 = 0x0600_0000;
const COMPOSER_SHA256: &str = "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f";
const CROP_START: u64 = COMPOSER + 0x37e534;
const CROP_END: u64 = COMPOSER + 0x37e56c;
const WRITER: u64 = MODEL + 0xf000;
const PAGE_SIZE: u64 = MODEL + 0xf100;
const COMMON_IMPL: u64 = MODEL + 0xa800;

struct Case {
    name: String,
    page_size: [i32; 2],
    margins: [f32; 2],
    measured: [f32; 4],
}

impl Case {
    fn new(name: &str, measured: [f32; 4]) -> Self {
        Self {
            name: name.into(),
            page_size: [100, 200],
            margins: [10.0, 20.0],
            measured,
        }
    }

    fn fixture(&self, machine: &mut Machine, recorder: &mut clipping::Recorder) -> String {
        write(machine.engine, MODEL, &vec![0; 0x100000]);
        machine.heap.cursor = HEAP;
        write(
            machine.engine,
            DRAWING_OBJECT,
            &[machine.heap.allocation_fill; 240],
        );
        machine.call(DRAWING_BASE + 0xa5634, &[DRAWING_OBJECT, 0, TABLE_OBJECT]);
        let default_focus: [f32; 4] = std::array::from_fn(|axis| {
            read_float(machine.engine, DRAWING_OBJECT + 120 + axis as u64 * 4)
        });
        assert_eq!(default_focus, [0.0; 4]);
        write(
            machine.engine,
            TABLE_OBJECT + 16,
            &COMMON_IMPL.to_le_bytes(),
        );
        write(machine.engine, CANVAS, &CANVAS_VTABLE.to_le_bytes());
        write(
            machine.engine,
            CANVAS_VTABLE + 64,
            &0x0300_0280_u64.to_le_bytes(),
        );
        let mut default_clips = Vec::new();
        for constraint in [0_u32, 1, 2] {
            write(machine.engine, COMMON_IMPL + 124, &constraint.to_le_bytes());
            recorder.calls.clear();
            machine.call(
                DRAWING_BASE + 0xa72d4,
                &[DRAWING_OBJECT, CANVAS, LAYOUT, RETURN_STYLE],
            );
            assert!(recorder.calls.is_empty());
            default_clips.push(recorder.calls.len());
        }
        for (axis, dimension) in self.page_size.into_iter().enumerate() {
            write(
                machine.engine,
                PAGE_SIZE + axis as u64 * 4,
                &dimension.to_le_bytes(),
            );
        }
        for (axis, value) in self.measured.into_iter().enumerate() {
            write(
                machine.engine,
                WRITER + 96 + axis as u64 * 4,
                &value.to_le_bytes(),
            );
        }
        register(machine.engine, REGISTER_X0 + 19, WRITER);
        register(machine.engine, REGISTER_X0 + 22, PAGE_SIZE);
        register(machine.engine, REGISTER_X0 + 23, WRITER + 96);
        register(machine.engine, 136, u64::from(self.margins[1].to_bits()));
        register(machine.engine, 145, u64::from(self.margins[0].to_bits()));
        register(machine.engine, REGISTER_SP, STACK);
        register(machine.engine, REGISTER_X30, STOP);
        check(unsafe { uc_emu_start(machine.engine, CROP_START, CROP_END, 1_000_000, 1000) });
        assert_eq!(read_register(machine.engine, 260), CROP_END);
        let crop: [f32; 4] =
            std::array::from_fn(|axis| read_float(machine.engine, WRITER + 80 + axis as u64 * 4));
        format!(
            "{{\"name\":{:?},\"page_size\":{:?},\"scaled_margins\":{:?},\"measured_bbox\":{:?},\"background_crop\":{crop:?},\"default_focus\":{default_focus:?},\"default_clip_counts\":{default_clips:?}}}",
            self.name, self.page_size, self.margins, self.measured,
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base: &Path, composer: &Path) {
    frames::load_base(machine, base);
    map_library(machine.engine, composer, COMPOSER, COMPOSER_SHA256);
    for (plt, target) in [
        (COMPOSER + 0x54f2d0, BASE + 0xb1350),
        (COMPOSER + 0x553570, BASE + 0xb16e0),
        (DRAWING_BASE + 0xbcd70, 0x2d2208),
        (DRAWING_BASE + 0xb92a0, BASE + 0xb11bc),
    ] {
        bind_native(machine.engine, plt, target);
    }
    write(
        machine.engine,
        DRAWING_BASE + 0xb8a30,
        &0xd65f03c0_u32.to_le_bytes(),
    );
    let mut recorder = clipping::Recorder::new(machine);
    let mut cases = Vec::new();
    for (name, measured) in [
        ("inside-page", [20.25, 30.75, 80.125, 100.875]),
        ("touch-page", [0.0, 10.0, 100.0, 180.0]),
        ("cross-page", [-20.25, -30.75, 120.125, 220.875]),
        ("left-of-page", [-40.0, 20.0, -10.0, 50.0]),
        ("right-of-page", [110.0, 20.0, 140.0, 50.0]),
        ("above-page", [20.0, -40.0, 50.0, -10.0]),
        ("below-page", [20.0, 210.0, 50.0, 240.0]),
        ("touch-left", [-40.0, 20.0, 0.0, 50.0]),
        ("touch-top", [20.0, -20.0, 50.0, 10.0]),
        ("touch-right", [100.0, 20.0, 140.0, 50.0]),
        ("touch-bottom", [20.0, 180.0, 50.0, 240.0]),
        ("zero-width", [20.0, 30.0, 20.0, 100.0]),
        ("zero-height", [20.0, 30.0, 80.0, 30.0]),
        ("inverted-width", [80.0, 30.0, 20.0, 100.0]),
        ("inverted-height", [20.0, 100.0, 80.0, 30.0]),
    ] {
        for margins in [[0.0, 0.0], [10.0, 20.0], [0.25, 0.75], [-10.5, -20.5]] {
            let mut case = Case::new(&format!("{name}-{margins:?}"), measured);
            case.margins = margins;
            cases.push(case);
        }
    }
    for boundary in [10.0_f32, 180.0] {
        for edge in [
            f32::from_bits(boundary.to_bits() - 1),
            boundary,
            f32::from_bits(boundary.to_bits() + 1),
        ] {
            for (name, measured) in [
                ("above", [20.0, -20.0, 80.0, edge]),
                ("below", [20.0, edge, 80.0, 240.0]),
            ] {
                cases.push(Case::new(&format!("boundary-{name}-{edge}"), measured));
            }
        }
    }
    for size in [
        [0, 200],
        [100, 0],
        [-100, 200],
        [100, -200],
        [1080, 1527],
        [16_777_217, 16_777_217],
    ] {
        let mut case = Case::new(
            &format!("page-size-{size:?}"),
            [0.0, 0.0, 20_000_000.0, 20_000_000.0],
        );
        case.page_size = size;
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
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"composer_library_sha256\":\"{COMPOSER_SHA256}\",\"constructor_address\":\"0xa5634\",\"crop_start\":\"0x37e534\",\"crop_end\":\"0x37e56c\",\"measurement_inputs\":\"Supplied world measured bounds (including fractional helper-boundary probes), page dimensions and density-scaled body margins. The native writer crop instruction block, Base rectangle helpers and Drawing constructor execute unchanged; constructor logging and canvas command recording are isolated. No bitmap or graphics factory executes.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n"),
    );
}
